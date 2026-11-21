//! Servidor HTTP minimo da Fase C (RFC 0002): `http_serve` std-only.
//!
//! Modelo: thread-por-conexao (`std::thread`), nada compartilhado entre
//! requests. Cada request compila um programa sintetico (todas as `fn` de
//! topo + chamada do handler) **na propria worker thread** a partir de
//! `Arc<Program>` (so dados owned, `Send+Sync`); `Chunk`/`Rc` nunca cruzam
//! threads. Sem `unsafe`, sem `Send` manual, sem dependencias.
//!
//! Limites: `max_conexoes` (acima = 503 imediato), `max_requisicoes`
//! (0 = para sempre; N = serve N e retorna — ancora de testes),
//! corpo de request ate 1 MiB, I/O com timeout de 30 s. Sem keep-alive.

use rhp_bytecode::{compile, Value};
use rhp_parser::Program as AstProgram;
use rhp_parser::{AssignTarget, CallArg, CallData, Expr, Program, Stmt, StmtNode};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{type_name, Vm, VmError};

/// Timeout de I/O por conexao (s).
const IO_TIMEOUT_SECS: u64 = 30;
/// Teto de corpo de request (bytes; mesmo teto do cliente).
const MAX_BODY: usize = 1024 * 1024;
/// Teto de cabecalhos de request (bytes).
const MAX_HEADERS: usize = 64 * 1024;
/// Teto de conexoes simultaneas permitido em `max_conexoes`.
const MAX_CONNS_HARD: u64 = 1024;
/// Teto de IPs rastreados no rate limit (eviccao grosseira: limpa tudo).
const MAX_TRACKED_IPS: usize = 4096;
/// Orcamento padrao de instrucoes por request no handler (0.6.0: antes
/// ilimitado — handler em loop eterno ocupava a worker para sempre).
const DEFAULT_MAX_PASSOS: u64 = 10_000_000;

/// Opcoes de `http_serve` (tudo opcional).
#[derive(Debug, Clone)]
pub struct ServeOpts {
    /// Interface de escuta ("127.0.0.1" padrao; "0.0.0.0" = todas IPv4;
    /// IP literal v4/v6 especifico; "localhost" = 127.0.0.1). Outros nomes
    /// sao rejeitados (sem DNS no caminho do bind).
    pub host: String,
    pub max_conexoes: u64,
    /// 0 = servir para sempre; N = servir N requests e retornar.
    pub max_requisicoes: u64,
    /// Teto de requests/segundo por IP (0 = sem limite).
    pub limite_taxa: u64,
    /// Teto de instrucoes da VM por request (0 = sem limite).
    pub max_passos: u64,
    /// Teto de heap da VM por request em bytes (0 = sem limite).
    pub max_heap_bytes: u64,
}

impl Default for ServeOpts {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            max_conexoes: 32,
            max_requisicoes: 0,
            limite_taxa: 0,
            max_passos: DEFAULT_MAX_PASSOS,
            max_heap_bytes: 0,
        }
    }
}

// `true` dentro da worker thread de um request: `http_serve` aninhado
// (no handler) seria um bloqueio eterno — vira erro claro em vez de hang.
std::thread_local! {
    static EM_HANDLER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Decisao de rate limit (janela fixa de 1 s por IP; `agora` em segundos).
/// Pura para teste deterministico; o servidor passa o relogio real.
fn permite(
    contadores: &mut HashMap<std::net::IpAddr, (u64, u32)>,
    ip: std::net::IpAddr,
    agora: u64,
    limite: u64,
) -> bool {
    if limite == 0 {
        return true;
    }
    if contadores.len() >= MAX_TRACKED_IPS {
        contadores.clear();
    }
    let slot = contadores.entry(ip).or_insert((agora, 0));
    if slot.0 != agora {
        *slot = (agora, 0);
    }
    if u64::from(slot.1) >= limite {
        return false;
    }
    slot.1 += 1;
    true
}

fn agora_segundos() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Controle compartilhado entre accept-loop e workers.
struct ServerCtl {
    /// Pedido de parada (limite atingido ou `exit` no handler).
    done: AtomicBool,
    /// `exit(codigo)` dentro de handler (termina o servidor).
    exit_code: Mutex<Option<i32>>,
    /// Conexoes ativas agora.
    ativas: AtomicUsize,
    /// Requests concluidos.
    concluidos: AtomicU64,
}

/// Ponto de entrada do builtin: valida args e serve.
pub(super) fn serve_builtin(
    args: &[Value],
    program: Option<Arc<AstProgram>>,
) -> Result<Value, crate::VmError> {
    use crate::VmError;
    let port = match &args[0] {
        Value::Int(n) => *n,
        other => {
            return Err(VmError::new(format!(
                "`http_serve`: porta exige `Int`, encontrado {}",
                type_name(other)
            )));
        }
    };
    let handler = match &args[1] {
        Value::Str(name) => name.clone(),
        Value::Func(func) => {
            // Valor de funcao resolve pelo nome no topo (equivale a string).
            // Exige `fn <nome>` no nivel superior: closures/aninhadas nao
            // tem entrada no programa sintetico (limitacao documentada).
            let name = func.def.name.clone();
            let no_topo = program.as_ref().is_some_and(|p| {
                p.statements
                    .iter()
                    .any(|node| matches!(&node.stmt, Stmt::FnDef { name: n, .. } if n == &name))
            });
            if name.is_empty() || !no_topo {
                return Err(VmError::new(format!(
                    "`http_serve`: handler como valor so aceita `fn {name}` no nivel superior (closures nao suportadas; use o nome como string)"
                )));
            }
            name
        }
        other => {
            return Err(VmError::new(format!(
                "`http_serve`: handler exige string (nome de `fn`) ou valor de funcao, encontrado {}",
                type_name(other)
            )));
        }
    };
    let opts = match &args[2] {
        Value::Map(opts) => parse_serve_opts(opts)?,
        other => {
            return Err(VmError::new(format!(
                "`http_serve`: opts exige mapa, encontrado {}",
                type_name(other)
            )));
        }
    };
    let program = program
        .ok_or_else(|| VmError::new("`http_serve`: sem programa (uso interno)".to_string()))?;
    serve(port, &handler, &opts, &program)
}

/// Resolve `host` de escuta: IP literal ou `localhost` (= 127.0.0.1).
/// Nomes arbitrarios sao rejeitados (sem DNS no caminho do bind).
fn resolve_host(host: &str) -> Result<std::net::IpAddr, String> {
    if host.eq_ignore_ascii_case("localhost") {
        return Ok(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
    }
    host.parse::<std::net::IpAddr>().map_err(|_| {
        format!("use IP literal ou \"localhost\" (ex.: \"127.0.0.1\", \"0.0.0.0\"), encontrado `{host}`")
    })
}

/// Valida o mapa de `opts` (`host`, `max_conexoes`, `max_requisicoes`,
/// `limite_taxa`, `max_passos`, `max_heap_bytes`).
fn parse_serve_opts(opts: &HashMap<String, Value>) -> Result<ServeOpts, VmError> {
    let mut parsed = ServeOpts::default();
    for (key, value) in opts {
        match key.as_str() {
            "host" => match value {
                Value::Str(h)
                    if h.eq_ignore_ascii_case("localhost")
                        || h.parse::<std::net::IpAddr>().is_ok() =>
                {
                    parsed.host = if h.eq_ignore_ascii_case("localhost") {
                        "127.0.0.1".to_string()
                    } else {
                        h.clone()
                    };
                }
                other => {
                    return Err(VmError::new(format!(
                        "`http_serve`: `host` invalido (use IP literal ou \"localhost\"), encontrado {}",
                        type_name(other)
                    )));
                }
            },
            "max_conexoes" => match value {
                Value::Int(n) if (1..=MAX_CONNS_HARD as i64).contains(n) => {
                    parsed.max_conexoes = *n as u64;
                }
                other => {
                    return Err(VmError::new(format!(
                        "`http_serve`: `max_conexoes` exige 1–{MAX_CONNS_HARD}, encontrado {}",
                        type_name(other)
                    )));
                }
            },
            "max_requisicoes" => match value {
                Value::Int(n) if *n >= 0 => {
                    parsed.max_requisicoes = *n as u64;
                }
                other => {
                    return Err(VmError::new(format!(
                        "`http_serve`: `max_requisicoes` exige >= 0, encontrado {}",
                        type_name(other)
                    )));
                }
            },
            "limite_taxa" => match value {
                Value::Int(n) if *n >= 0 => {
                    parsed.limite_taxa = *n as u64;
                }
                other => {
                    return Err(VmError::new(format!(
                        "`http_serve`: `limite_taxa` exige >= 0, encontrado {}",
                        type_name(other)
                    )));
                }
            },
            "max_passos" => match value {
                Value::Int(n) if *n >= 0 => {
                    parsed.max_passos = *n as u64;
                }
                other => {
                    return Err(VmError::new(format!(
                        "`http_serve`: `max_passos` exige >= 0 (0 = sem limite), encontrado {}",
                        type_name(other)
                    )));
                }
            },
            "max_heap_bytes" => match value {
                Value::Int(n) if *n >= 0 => {
                    parsed.max_heap_bytes = *n as u64;
                }
                other => {
                    return Err(VmError::new(format!(
                        "`http_serve`: `max_heap_bytes` exige >= 0 (0 = sem limite), encontrado {}",
                        type_name(other)
                    )));
                }
            },
            unknown => {
                return Err(VmError::new(format!(
                    "`http_serve`: opcao desconhecida `{unknown}` (use `host`, `max_conexoes`, `max_requisicoes`, `limite_taxa`, `max_passos`, `max_heap_bytes`)"
                )));
            }
        }
    }
    Ok(parsed)
}
/// Serve `programa` chamando `handler(req)` por request.
///
/// Bloqueia ate `max_requisicoes` ou `exit()` no handler. Devolve `null`
/// ao fim normal; `exit(codigo)` vira erro especial (a CLI traduz em
/// codigo de saida).
pub fn serve(
    port: i64,
    handler: &str,
    opts: &ServeOpts,
    program: &Arc<AstProgram>,
) -> Result<Value, VmError> {
    if !(0..=65535).contains(&port) {
        return Err(VmError::new(format!(
            "`http_serve`: porta invalida {port} (0–65535; 0 = porta livre do SO)"
        )));
    }
    if handler.is_empty() {
        return Err(VmError::new(
            "`http_serve`: nome do handler vazio".to_string(),
        ));
    }
    if EM_HANDLER.get() {
        return Err(VmError::new(
            "`http_serve`: nao pode ser chamado dentro de um handler (bloquearia para sempre)"
                .to_string(),
        ));
    }
    // Falha rapida: handler precisa existir e o programa precisa compilar.
    preflight(program, handler)?;
    let host = resolve_host(&opts.host).map_err(|dica| {
        VmError::new(format!(
            "`http_serve`: `host` invalido ({dica}): `{}`",
            opts.host
        ))
    })?;
    let listener = TcpListener::bind((host, port as u16)).map_err(|e| {
        VmError::new(format!(
            "`http_serve`: bind em {host}:{port} falhou: {e} (porta em uso? permissao? interface inexistente?)"
        ))
    })?;
    listener
        .set_nonblocking(true)
        .map_err(|e| VmError::new(format!("`http_serve`: nonblock: {e}")))?;
    let bound = listener
        .local_addr()
        .map(|a| a.to_string())
        .unwrap_or_default();
    println!("rhp: ouvindo em {bound}");
    let ctl = Arc::new(ServerCtl {
        done: AtomicBool::new(false),
        exit_code: Mutex::new(None),
        ativas: AtomicUsize::new(0),
        concluidos: AtomicU64::new(0),
    });
    let max_req = opts.max_requisicoes;
    let limite = opts.limite_taxa;
    let mut taxas: HashMap<std::net::IpAddr, (u64, u32)> = HashMap::new();
    loop {
        if ctl.done.load(Ordering::SeqCst) {
            break;
        }
        if max_req > 0 && ctl.concluidos.load(Ordering::SeqCst) >= max_req {
            break;
        }
        let stream = match listener.accept() {
            Ok((stream, _)) => stream,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            Err(e) => {
                // Erro transiente (ex.: EMFILE): registra, recua e continua.
                // Parar aqui silenciosamente fingiria conclusao normal.
                eprintln!("rhp: accept falhou ({e}); tentando de novo");
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
        };
        if ctl.ativas.load(Ordering::SeqCst) >= opts.max_conexoes as usize {
            let _ = respond_503(&stream);
            ctl.concluidos.fetch_add(1, Ordering::SeqCst);
            if max_req > 0 && ctl.concluidos.load(Ordering::SeqCst) >= max_req {
                ctl.done.store(true, Ordering::SeqCst);
            }
            continue;
        }
        if limite > 0 {
            let ip = stream.peer_addr().map(|a| a.ip());
            let ok = match ip {
                Ok(ip) => permite(&mut taxas, ip, agora_segundos(), limite),
                Err(_) => true,
            };
            if !ok {
                let _ = respond_429(&stream);
                ctl.concluidos.fetch_add(1, Ordering::SeqCst);
                if max_req > 0 && ctl.concluidos.load(Ordering::SeqCst) >= max_req {
                    ctl.done.store(true, Ordering::SeqCst);
                }
                continue;
            }
        }
        ctl.ativas.fetch_add(1, Ordering::SeqCst);
        let worker_ctl = Arc::clone(&ctl);
        let worker_program = Arc::clone(program);
        let worker_handler = handler.to_string();
        let worker_limites = super::VmLimits {
            max_steps: (opts.max_passos > 0).then_some(opts.max_passos),
            max_stack_depth: None,
            max_collection_len: None,
            max_heap_bytes: (opts.max_heap_bytes > 0).then_some(opts.max_heap_bytes),
        };
        std::thread::spawn(move || {
            if let Some(code) =
                handle_one(stream, &worker_handler, &worker_program, &worker_limites)
            {
                *worker_ctl.exit_code.lock().expect("lock envenenado") = Some(code);
                worker_ctl.done.store(true, Ordering::SeqCst);
            }
            worker_ctl.ativas.fetch_sub(1, Ordering::SeqCst);
            let done = worker_ctl.concluidos.fetch_add(1, Ordering::SeqCst) + 1;
            if max_req > 0 && done >= max_req {
                worker_ctl.done.store(true, Ordering::SeqCst);
            }
        });
    }
    if let Some(code) = *ctl.exit_code.lock().expect("lock envenenado") {
        return Err(VmError::exit(code));
    }
    Ok(Value::Null)
}

/// Compila a chamada do handler preservando o retorno.
///
/// `compile` de `ExprStmt` termina com `Pop` (descarta o valor); aqui o
/// valor e o resultado, entao o `Pop` final e removido (com assercao de
/// forma; qualquer mudanca no compilador falha alto, nao silenciosa).
fn compile_handler_call(
    program: &AstProgram,
    handler: &str,
    request: &Value,
) -> Result<rhp_bytecode::Chunk, String> {
    let synthetic = synthetic_program(program, handler, request)?;
    let mut chunk = compile(&synthetic).map_err(|e| format!("compilacao: {e}"))?;
    let len = chunk.code.len();
    let shape_ok = len >= 3
        && chunk.code[len - 1] == rhp_bytecode::Op::Halt
        && chunk.code[len - 2] == rhp_bytecode::Op::Pop
        && matches!(chunk.code[len - 3], rhp_bytecode::Op::Call(1));
    if !shape_ok {
        return Err("forma inesperada da chamada (compilador mudou?)".to_string());
    }
    chunk.code.remove(len - 2);
    chunk
        .validate()
        .map_err(|e| format!("bytecode invalido: {e}"))?;
    Ok(chunk)
}

/// Compila uma vez para falhar rapido (handler inexistente, etc.).
fn preflight(program: &AstProgram, handler: &str) -> Result<(), VmError> {
    let found = program
        .statements
        .iter()
        .any(|node| matches!(&node.stmt, Stmt::FnDef { name, .. } if name == handler));
    if !found {
        return Err(VmError::new(format!(
            "`http_serve`: handler `{handler}` nao definido"
        )));
    }
    compile_handler_call(program, handler, &Value::Null)
        .map_err(|e| VmError::new(format!("`http_serve`: {e}")))?;
    Ok(())
}

/// Programa sintetico por request: todo o topo (definicoes + estado) + chamada.
/// Fase Next-Gen 7: inclui `let`/`const`/`class`/`import native`/`namespace`
/// e chamadas de registro (ex.: `web_get(...)`), nao so `fn` — o worker
/// reconstrói o estado executando o topo uma vez por request (idempotente).
/// A chamada `http_serve(...)` e filtrada (senao o worker serviria de novo).
fn synthetic_program(
    program: &AstProgram,
    handler: &str,
    request: &Value,
) -> Result<Program, String> {
    let mut statements: Vec<StmtNode> = program
        .statements
        .iter()
        .filter(|node| !is_serve_stmt(&node.stmt))
        .cloned()
        .collect();
    statements.push(StmtNode {
        leading: Vec::new(),
        trailing: None,
        blank_before: false,
        stmt: Stmt::ExprStmt {
            expr: Expr::Call(Box::new(CallData {
                name: handler.to_string(),
                type_args: Vec::new(),
                args: vec![CallArg::Pos(expr_from_value(request)?)],
            })),
        },
    });
    Ok(Program {
        statements,
        trailing_comments: Vec::new(),
    })
}

/// `true` se executar o statement no topo invocaria `http_serve(...)` —
/// filtrado do programa sintetico do worker (evita re-servir).
/// LIB-001: antes so `ExprStmt`/`Let`/`Const`/`Return`/`Throw` eram
/// filtrados; `echo http_serve(...)` ou `if (c) { http_serve(...); }` no
/// topo quebravam TODO request com 500 (anti-reentrancia).
/// Regra: expressoes e blocos que executam no topo sao varridos;
/// contextos de definicao (`fn`, metodos, closures) nao executam no topo
/// (a guarda anti-reentrancia cobre chamadas reais no handler).
fn is_serve_stmt(stmt: &Stmt) -> bool {
    match stmt {
        // Definicoes nao executam no topo: mantidas (o worker precisa delas).
        Stmt::FnDef { .. }
        | Stmt::ClassDef { .. }
        | Stmt::InterfaceDef { .. }
        | Stmt::TraitDef { .. }
        | Stmt::Import { .. }
        | Stmt::ImportNames { .. }
        | Stmt::ReExport { .. }
        | Stmt::Export { .. }
        | Stmt::Namespace { .. }
        | Stmt::ImportQualified { .. }
        | Stmt::ImportNative { .. }
        | Stmt::Break
        | Stmt::Continue
        | Stmt::BreakN(_)
        | Stmt::ContinueN(_) => false,
        Stmt::ExprStmt { expr } | Stmt::Throw { expr } => expr_calls_serve(expr),
        Stmt::Return { expr: Some(e) } => expr_calls_serve(e),
        Stmt::Return { expr: None } | Stmt::Yield { .. } => false,
        Stmt::Let { init: Some(e), .. }
        | Stmt::Const { init: e, .. }
        | Stmt::Static { init: e, .. } => expr_calls_serve(e),
        Stmt::Let { init: None, .. } => false,
        Stmt::Destructure { init, .. } => expr_calls_serve(init),
        Stmt::Assign { target, expr } => {
            expr_calls_serve(expr)
                || match target {
                    AssignTarget::Var(_) => false,
                    AssignTarget::Index { name: _, index } => expr_calls_serve(index),
                }
        }
        Stmt::Echo { expr } => expr_calls_serve(expr),
        Stmt::If {
            cond,
            then_branch,
            else_branch,
        } => {
            expr_calls_serve(cond)
                || block_calls_serve(then_branch)
                || else_branch.as_ref().is_some_and(|b| block_calls_serve(b))
        }
        Stmt::While { cond, body } => expr_calls_serve(cond) || block_calls_serve(body),
        Stmt::For { iter, body, .. } => expr_calls_serve(iter) || block_calls_serve(body),
        Stmt::ForC {
            init,
            cond,
            step,
            body,
        } => {
            init.as_ref().is_some_and(|s| is_serve_stmt(&s.stmt))
                || cond.as_ref().is_some_and(expr_calls_serve)
                || step.as_ref().is_some_and(|s| is_serve_stmt(&s.stmt))
                || block_calls_serve(body)
        }
        Stmt::Switch {
            discriminant,
            cases,
        } => {
            expr_calls_serve(discriminant)
                || cases.iter().any(|c| {
                    c.cond.as_ref().is_some_and(expr_calls_serve) || block_calls_serve(&c.body)
                })
        }
        Stmt::Try {
            body,
            catches,
            finally_body,
        } => {
            block_calls_serve(body)
                || catches.iter().any(|c| block_calls_serve(&c.body))
                || finally_body.as_ref().is_some_and(|b| block_calls_serve(b))
        }
        Stmt::Block(stmts) => block_calls_serve(stmts),
    }
}

/// Algum statement do bloco invocaria `http_serve` se executado.
fn block_calls_serve(stmts: &[StmtNode]) -> bool {
    stmts.iter().any(|node| is_serve_stmt(&node.stmt))
}

/// Procura `http_serve(...)` na expressao (direta ou aninhada em chamada).
fn expr_calls_serve(expr: &Expr) -> bool {
    match expr {
        Expr::Call(data) => data.name == "http_serve",
        Expr::CallValue { callee, args } => {
            expr_calls_serve(callee) || args.iter().any(|a| expr_calls_serve(a.value()))
        }
        Expr::Binary { left, right, .. } => expr_calls_serve(left) || expr_calls_serve(right),
        Expr::Unary { expr, .. } | Expr::Await { expr } => expr_calls_serve(expr),
        Expr::Ternary {
            cond,
            then_branch,
            else_branch,
        } => {
            expr_calls_serve(cond) || expr_calls_serve(then_branch) || expr_calls_serve(else_branch)
        }
        Expr::Array(items) => items.iter().any(expr_calls_serve),
        Expr::Map(entries) => entries.iter().any(|(_, v)| expr_calls_serve(v)),
        Expr::Index { base, index } => expr_calls_serve(base) || expr_calls_serve(index),
        Expr::Member { obj, .. } => expr_calls_serve(obj),
        Expr::New(data) => {
            expr_calls_serve(&data.class) || data.args.iter().any(|a| expr_calls_serve(a.value()))
        }
        Expr::Match { subject, arms } => {
            expr_calls_serve(subject)
                || arms.iter().any(|a| {
                    a.cond.as_ref().is_some_and(expr_calls_serve) || expr_calls_serve(&a.value)
                })
        }
        // Closures sao definicoes (nao executam no topo): a guarda
        // anti-reentrancia cobre chamadas reais dentro do handler.
        Expr::Closure(_) => false,
        _ => false,
    }
}

/// Valor RHP => literal AST (request; `Func` nao atravessa).
fn expr_from_value(value: &Value) -> Result<Expr, String> {
    match value {
        Value::Null => Ok(Expr::Null),
        Value::Bool(b) => Ok(Expr::Bool(*b)),
        Value::Int(n) => Ok(Expr::Int(*n)),
        Value::Float(n) => Ok(Expr::Float(*n)),
        Value::Str(s) => Ok(Expr::StringLiteral(s.clone())),
        Value::Array(items) => items
            .iter()
            .map(expr_from_value)
            .collect::<Result<_, _>>()
            .map(Expr::Array),
        Value::Map(entries) => entries
            .iter()
            .map(|(k, v)| Ok((k.clone(), expr_from_value(v)?)))
            .collect::<Result<_, _>>()
            .map(Expr::Map),
        other => Err(format!(
            "valor nao serializavel para request: {}",
            type_name(other)
        )),
    }
}

/// Atende UMA conexao: parse, handler, resposta (500 sem vazar detalhe).
/// Erros de protocolo (request malformado) viram 400. Devolve `Some(codigo)`
/// se `exit()` foi chamado no handler.
fn handle_one(
    mut stream: TcpStream,
    handler: &str,
    program: &Arc<AstProgram>,
    limites: &super::VmLimits,
) -> Option<i32> {
    stream
        .set_read_timeout(Some(Duration::from_secs(IO_TIMEOUT_SECS)))
        .and_then(|()| stream.set_write_timeout(Some(Duration::from_secs(IO_TIMEOUT_SECS))))
        .ok()?;
    let request = match read_request(&mut stream) {
        Ok(request) => request,
        Err(message) => {
            eprintln!("rhp: request invalido: {message}");
            let _ = respond_400(&stream);
            return None;
        }
    };
    match run_handler(&request, handler, program, limites) {
        Ok(response) => {
            if let Err(message) = write_response(&stream, &response) {
                eprintln!("rhp: responder falhou: {message}");
            }
            None
        }
        Err(HandlerFail::Exit(code)) => {
            // HTTP-006: antes a conexao caia sem resposta (EOF abrupto).
            // Envia 200 vazio com `Connection: close` antes de encerrar.
            let _ = write_response(
                &stream,
                &HttpResponse {
                    status: 200,
                    headers: vec![("connection".to_string(), "close".to_string())],
                    body: Vec::new(),
                },
            );
            Some(code)
        }
        Err(HandlerFail::Fail(message)) => {
            eprintln!("rhp: handler `{handler}`: {message}");
            let _ = respond_500(&stream);
            None
        }
    }
}

/// Falha do handler: `Exit` termina o servidor; `Fail` vira 500.
enum HandlerFail {
    Exit(i32),
    Fail(String),
}

/// RAII da guarda anti-reentrada (`EM_HANDLER`).
struct GuardaHandler;

impl GuardaHandler {
    fn armar() -> Self {
        EM_HANDLER.set(true);
        GuardaHandler
    }
}

impl Drop for GuardaHandler {
    fn drop(&mut self) {
        EM_HANDLER.set(false);
    }
}

/// Request HTTP parseado.
struct HttpRequest {
    method: String,
    path: String,
    query: HashMap<String, String>,
    headers: HashMap<String, String>,
    cookies: HashMap<String, String>,
    body: String,
}

fn read_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    // Prazo total da leitura (cabecalhos + corpo): o timeout por `read`
    // sozinho permite gotejamento indefinido (slowloris).
    let prazo = std::time::Instant::now() + Duration::from_secs(IO_TIMEOUT_SECS);
    let mut raw: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        if std::time::Instant::now() > prazo {
            return Err("tempo esgotado lendo request".to_string());
        }
        let n = stream
            .read(&mut chunk)
            .map_err(|e| format!("ler request: {e}"))?;
        if n == 0 {
            break;
        }
        raw.extend_from_slice(&chunk[..n]);
        if raw.len() > MAX_HEADERS + MAX_BODY {
            return Err("request grande demais".to_string());
        }
        if raw.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    let header_end = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| "request sem cabecalhos".to_string())?;
    if header_end > MAX_HEADERS {
        return Err(format!(
            "cabecalhos grandes demais ({} bytes; teto {MAX_HEADERS})",
            header_end
        ));
    }
    let header = String::from_utf8_lossy(&raw[..header_end]).into_owned();
    let mut lines = header.lines();
    let status_line = lines.next().ok_or_else(|| "request vazio".to_string())?;
    let mut parts = status_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| "request sem metodo".to_string())?
        .to_string();
    let target = parts
        .next()
        .ok_or_else(|| "request sem caminho".to_string())?
        .to_string();
    // HTTP-009: NUL no alvo invalida o request (evita confusao em roteador,
    // filesystem e logs downstream; `%00` decodificado tambem e barrado).
    if target.bytes().any(|b| b == 0) {
        return Err("caminho com byte NUL".to_string());
    }
    // HTTP-008: forma absoluta (`GET http://host/abc?q=1`) — normaliza para
    // origem (caminho + query); proxies de encaminhamento usam essa forma.
    let target = strip_absolute_form(&target);
    let (path, query) = match target.find('?') {
        Some(pos) => (
            percent_decode(&target[..pos], false),
            parse_query(&target[pos + 1..]),
        ),
        None => (percent_decode(target, false), HashMap::new()),
    };
    // HTTP-009 (cont.): `%00` decodificado vira NUL — barra apos decode.
    if path.bytes().any(|b| b == 0) {
        return Err("caminho com byte NUL".to_string());
    }
    let mut headers = HashMap::new();
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            // HTTP-007: linha sem `:` (obs-fold) = request invalido (400).
            // Antes era ignorada em silencio (risco de smuggling contra
            // proxies que desdobram a continuacao).
            return Err("cabecalho sem `:` (obs-fold nao suportado)".to_string());
        };
        // HTTP-009 (cont.): NUL em nome/valor de cabecalho.
        if name.bytes().any(|b| b == 0) || value.bytes().any(|b| b == 0) {
            return Err("cabecalho com byte NUL".to_string());
        }
        let entry = headers
            .entry(name.trim().to_lowercase())
            .or_insert_with(String::new);
        if !entry.is_empty() {
            entry.push_str(", ");
        }
        entry.push_str(value.trim());
    }
    let mut corpo = LeitorCorpo {
        buf: raw[header_end + 4..].to_vec(),
        pos: 0,
        stream,
        prazo,
    };
    // `Transfer-Encoding: chunked` tem precedencia sobre `Content-Length`
    // (RFC 9112 §6.3); outras codificacoes sao rejeitadas.
    let chunked = headers.get("transfer-encoding").is_some_and(|te| {
        te.split(',')
            .any(|v| v.trim().eq_ignore_ascii_case("chunked"))
    });
    if let Some(te) = headers.get("transfer-encoding") {
        if !chunked
            && te.split(',').any(|v| {
                let v = v.trim().to_lowercase();
                !v.is_empty() && v != "identity"
            })
        {
            return Err(format!("transfer-encoding nao suportado: `{te}`"));
        }
    }
    let body: Vec<u8> = if chunked {
        corpo.ler_chunked()?
    } else {
        let want: usize = match headers.get("content-length") {
            None => 0,
            Some(v) => v.parse::<usize>().map_err(|_| {
                format!("Content-Length invalido: `{v}` (esperado numero de bytes >= 0)")
            })?,
        };
        if want > MAX_BODY {
            return Err("corpo grande demais".to_string());
        }
        corpo.ler_fixo(want, "corpo")?
    };
    let body = String::from_utf8(body).map_err(|_| "corpo nao UTF-8".to_string())?;
    let cookies = parse_cookies(headers.get("cookie").map(String::as_str).unwrap_or(""));
    Ok(HttpRequest {
        method,
        path,
        query,
        headers,
        cookies,
        body,
    })
}

/// Leitor do corpo com o que ja chegou bufferizado + socket, sob prazo.
/// Tudo limitado por `MAX_BODY` (DoS por memoria contido).
struct LeitorCorpo<'a> {
    buf: Vec<u8>,
    pos: usize,
    stream: &'a mut TcpStream,
    prazo: std::time::Instant,
}

impl LeitorCorpo<'_> {
    /// Completa `out` ate `want` bytes (corpo `Content-Length`).
    fn ler_fixo(&mut self, want: usize, rotulo: &str) -> Result<Vec<u8>, String> {
        let mut out: Vec<u8> = Vec::new();
        self.encher(&mut out, want, rotulo)?;
        if out.len() < want {
            return Err(format!(
                "{rotulo} incompleto (Content-Length: {want}; recebidos: {})",
                out.len()
            ));
        }
        out.truncate(want);
        Ok(out)
    }

    /// Corpo `chunked` (RFC 9112 §7.1): `tamanho-hex[CRLF]dados[CRLF]...0[CRLF]trailers[CRLF]`.
    fn ler_chunked(&mut self) -> Result<Vec<u8>, String> {
        let mut out: Vec<u8> = Vec::new();
        loop {
            let linha = self.ler_linha()?;
            let tamanho_txt = linha.split(|b| *b == b';').next().unwrap_or(&[]);
            let tamanho_txt = String::from_utf8_lossy(tamanho_txt).trim().to_string();
            let size = usize::from_str_radix(&tamanho_txt, 16)
                .map_err(|_| format!("tamanho de chunk invalido: `{tamanho_txt}`"))?;
            if out.len().saturating_add(size) > MAX_BODY {
                return Err("corpo grande demais".to_string());
            }
            if size == 0 {
                // Trailers: descarta ate a linha vazia (teto: 32 linhas).
                for _ in 0..32 {
                    if self.ler_linha()?.is_empty() {
                        break;
                    }
                }
                break;
            }
            let alvo = out.len() + size;
            self.encher(&mut out, alvo, "chunk")?;
            if out.len() < alvo {
                return Err("chunk incompleto (EOF)".to_string());
            }
            let fim = self.ler_bytes(2)?;
            if fim != b"\r\n" {
                return Err("chunk malformado (esperado CRLF apos dados)".to_string());
            }
        }
        Ok(out)
    }

    /// Acrescenta bytes ate `out` ter `want` (ou EOF/erro/prazo).
    fn encher(&mut self, out: &mut Vec<u8>, want: usize, rotulo: &str) -> Result<(), String> {
        // Compacta o ja consumido (senao `buf` cresce sem limite).
        if self.pos > 65536 {
            self.buf.drain(..self.pos);
            self.pos = 0;
        }
        let mut chunk = [0u8; 4096];
        while out.len() < want {
            while self.pos < self.buf.len() && out.len() < want {
                out.push(self.buf[self.pos]);
                self.pos += 1;
            }
            if out.len() >= want {
                break;
            }
            if std::time::Instant::now() > self.prazo {
                return Err(format!("tempo esgotado lendo {rotulo}"));
            }
            let n = self
                .stream
                .read(&mut chunk)
                .map_err(|e| format!("ler {rotulo}: {e}"))?;
            if n == 0 {
                break;
            }
            self.buf.extend_from_slice(&chunk[..n]);
            if self.buf.len() > MAX_BODY + 65536 {
                return Err(format!("{rotulo} grande demais"));
            }
        }
        Ok(())
    }

    /// Le exatamente `n` bytes (ou erro se EOF antes).
    fn ler_bytes(&mut self, n: usize) -> Result<Vec<u8>, String> {
        let mut tmp: Vec<u8> = Vec::new();
        self.encher(&mut tmp, n, "chunk")?;
        if tmp.len() < n {
            return Err("chunk incompleto (EOF)".to_string());
        }
        Ok(tmp)
    }

    /// Lê uma linha terminada em `\n` (sem o terminador; sem o `\r`).
    /// Teto de 16 KiB por linha; EOF antes do `\n` = erro.
    fn ler_linha(&mut self) -> Result<Vec<u8>, String> {
        const TETO_LINHA: usize = 16 * 1024;
        let mut linha: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 256];
        loop {
            while self.pos < self.buf.len() {
                let b = self.buf[self.pos];
                self.pos += 1;
                if b == b'\n' {
                    if linha.last() == Some(&b'\r') {
                        linha.pop();
                    }
                    return Ok(linha);
                }
                linha.push(b);
                if linha.len() > TETO_LINHA {
                    return Err("linha do chunk grande demais".to_string());
                }
            }
            if std::time::Instant::now() > self.prazo {
                return Err("tempo esgotado lendo chunk".to_string());
            }
            let n = self
                .stream
                .read(&mut chunk)
                .map_err(|e| format!("ler chunk: {e}"))?;
            if n == 0 {
                return Err("chunk incompleto (EOF)".to_string());
            }
            self.buf.extend_from_slice(&chunk[..n]);
            if self.buf.len() > MAX_BODY + 65536 {
                return Err("chunk grande demais".to_string());
            }
        }
    }
}

/// Parseia `Cookie: a=1; b="x y"` (nomes/valores aparados; um par de
/// aspas externas e removido).
fn parse_cookies(header: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for pair in header.split(';') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        let (name, mut value) = match pair.split_once('=') {
            Some((k, v)) => (k.trim().to_string(), v.trim().to_string()),
            None => (pair.to_string(), String::new()),
        };
        if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
            value = value[1..value.len() - 1].to_string();
        }
        if !name.is_empty() {
            out.insert(name, value);
        }
    }
    out
}

/// Normaliza forma absoluta (`http://host/abc?q=1` ou `https://...`) para
/// origem (`/abc?q=1`). Sem esquema reconhecivel, devolve intacto.
fn strip_absolute_form(target: &str) -> &str {
    let rest = target
        .strip_prefix("http://")
        .or_else(|| target.strip_prefix("https://"));
    match rest {
        Some(after) => match after.find('/') {
            Some(pos) => &after[pos..],
            None => "/",
        },
        None => target,
    }
}

/// Decodifica `%XX` (e `+`, so em query); invalido = mantem literal.
/// Opera em bytes e so converte no fim (`from_utf8_lossy`): `%C3%A9`
/// vira "é", nao dois escalares latinos.
/// HTTP-001: `+` em caminho NAO vira espaco (so em query, application/
/// x-www-form-urlencoded). Antes, `/a+b` e `/a%2Bb` colidiam em `/a b`.
fn percent_decode(text: &str, plus_as_space: bool) -> String {
    let mut out: Vec<u8> = Vec::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' && plus_as_space {
            out.push(b' ');
        } else {
            out.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn parse_query(query: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        match pair.split_once('=') {
            Some((k, v)) => {
                map.insert(percent_decode(k, true), percent_decode(v, true));
            }
            None => {
                map.insert(percent_decode(pair, true), String::new());
            }
        }
    }
    map
}

/// Executa o handler e converte o retorno em resposta.
fn run_handler(
    request: &HttpRequest,
    handler: &str,
    program: &Arc<AstProgram>,
    limites: &super::VmLimits,
) -> Result<HttpResponse, HandlerFail> {
    let mut entries: HashMap<String, Value> = HashMap::new();
    entries.insert("metodo".to_string(), Value::Str(request.method.clone()));
    entries.insert("caminho".to_string(), Value::Str(request.path.clone()));
    entries.insert(
        "query".to_string(),
        Value::Map(
            request
                .query
                .iter()
                .map(|(k, v)| (k.clone(), Value::Str(v.clone())))
                .collect(),
        ),
    );
    entries.insert(
        "cabecalhos".to_string(),
        Value::Map(
            request
                .headers
                .iter()
                .map(|(k, v)| (k.clone(), Value::Str(v.clone())))
                .collect(),
        ),
    );
    entries.insert(
        "cookies".to_string(),
        Value::Map(
            request
                .cookies
                .iter()
                .map(|(k, v)| (k.clone(), Value::Str(v.clone())))
                .collect(),
        ),
    );
    entries.insert("corpo".to_string(), Value::Str(request.body.clone()));
    let req = Value::Map(entries);
    let chunk = compile_handler_call(program, handler, &req).map_err(HandlerFail::Fail)?;
    let mut vm = Vm::new();
    vm.set_limits(limites.clone());
    // Programa-fonte para `async_spawn` dentro de handlers.
    vm.set_program(Arc::clone(program));
    // Guarda anti-reentrada: `http_serve` aninhado vira erro claro.
    let _guarda = GuardaHandler::armar();
    let mut sink = std::io::sink();
    match vm.run_value(&chunk, &mut sink) {
        Ok(value) => response_from_value(&value).map_err(HandlerFail::Fail),
        Err(error) => match error.exit_code() {
            Some(code) => Err(HandlerFail::Exit(code)),
            None => Err(HandlerFail::Fail(format!("handler `{handler}`: {error}"))),
        },
    }
}

/// Resposta HTTP a enviar.
struct HttpResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

/// Retorno do handler => resposta (Str = 200 texto; mapa com
/// `status`/`cabecalhos`/`corpo`).
fn response_from_value(value: &Value) -> Result<HttpResponse, String> {
    match value {
        Value::Str(body) => Ok(HttpResponse {
            status: 200,
            headers: vec![(
                "content-type".to_string(),
                "text/plain; charset=utf-8".to_string(),
            )],
            body: body.as_bytes().to_vec(),
        }),
        Value::Map(entries) => {
            let status = match entries.get("status") {
                None => 200,
                Some(Value::Int(n)) if (100..=599).contains(n) => *n as u16,
                Some(other) => {
                    return Err(format!("`status` invalido: {}", type_name(other)));
                }
            };
            let mut headers = vec![(
                "content-type".to_string(),
                "text/plain; charset=utf-8".to_string(),
            )];
            if let Some(cabecalhos) = entries.get("cabecalhos") {
                match cabecalhos {
                    Value::Map(map) => {
                        headers.clear();
                        for (name, item) in map {
                            let item = match item {
                                Value::Str(s) => s.clone(),
                                other => {
                                    return Err(format!(
                                        "cabecalho `{name}` exige string, encontrado {}",
                                        type_name(other)
                                    ));
                                }
                            };
                            check_response_header(name, &item)?;
                            headers.push((name.clone(), item));
                        }
                    }
                    other => {
                        return Err(format!(
                            "`cabecalhos` exige mapa, encontrado {}",
                            type_name(other)
                        ));
                    }
                }
            }
            let body = match entries.get("corpo") {
                None => Vec::new(),
                Some(Value::Str(s)) => s.as_bytes().to_vec(),
                Some(other) => {
                    return Err(format!(
                        "`corpo` exige string, encontrado {}",
                        type_name(other)
                    ));
                }
            };
            Ok(HttpResponse {
                status,
                headers,
                body,
            })
        }
        other => Err(format!(
            "handler deve devolver string ou mapa, devolveu {}",
            type_name(other)
        )),
    }
}

fn check_response_header(name: &str, value: &str) -> Result<(), String> {
    if name.is_empty()
        || name.bytes().any(|b| b <= b' ' || b == b':' || b == 0x7f)
        || value.bytes().any(|b| b == b'\r' || b == b'\n')
    {
        return Err(format!("cabecalho de resposta invalido `{name}`"));
    }
    Ok(())
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        413 => "Content Too Large",
        422 => "Unprocessable Content",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Desconhecido",
    }
}

fn write_response(mut stream: &TcpStream, response: &HttpResponse) -> Result<(), String> {
    let mut head = format!(
        "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n",
        response.status,
        reason(response.status),
        response.body.len()
    );
    for (name, item) in &response.headers {
        head.push_str(&format!("{name}: {item}\r\n"));
    }
    head.push_str("\r\n");
    stream
        .write_all(head.as_bytes())
        .and_then(|()| stream.write_all(&response.body))
        .map_err(|e| format!("escrever resposta: {e}"))?;
    stream.flush().map_err(|e| format!("flush: {e}"))?;
    Ok(())
}

fn respond_503(stream: &TcpStream) -> Result<(), String> {
    write_response(
        stream,
        &HttpResponse {
            status: 503,
            headers: vec![(
                "content-type".to_string(),
                "text/plain; charset=utf-8".to_string(),
            )],
            body: b"servidor ocupado".to_vec(),
        },
    )
}

fn respond_429(stream: &TcpStream) -> Result<(), String> {
    write_response(
        stream,
        &HttpResponse {
            status: 429,
            headers: vec![
                (
                    "content-type".to_string(),
                    "text/plain; charset=utf-8".to_string(),
                ),
                ("retry-after".to_string(), "1".to_string()),
            ],
            body: b"limite de requisicoes excedido".to_vec(),
        },
    )
}

/// Resposta 400 para request malformado (erro do cliente; sem detalhe
/// interno) + log em stderr.
fn respond_400(stream: &TcpStream) -> Result<(), String> {
    write_response(
        stream,
        &HttpResponse {
            status: 400,
            headers: vec![(
                "content-type".to_string(),
                "text/plain; charset=utf-8".to_string(),
            )],
            body: b"requisicao invalida".to_vec(),
        },
    )
}

/// Resposta 500 generica (sem vazar detalhe) + log em stderr.
fn respond_500(stream: &TcpStream) -> Result<(), String> {
    write_response(
        stream,
        &HttpResponse {
            status: 500,
            headers: vec![(
                "content-type".to_string(),
                "text/plain; charset=utf-8".to_string(),
            )],
            body: b"erro interno".to_vec(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn ip(n: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, n))
    }

    #[test]
    fn synthetic_program_filters_echo_and_compound_serve() {
        // LIB-001: `echo http_serve(...)` / `if` no topo nao eram filtrados:
        // o worker re-executava a chamada e todo request dava 500
        // (anti-reentrancia). Definicoes (`fn`) sao mantidas.
        let src = "fn h(req) { return {\"status\": 200, \"corpo\": \"oi\"}; }\n\
             echo http_serve(18091, \"h\", {});\n";
        let program = rhp_parser::parse_source(src).expect("parse");
        let req = Value::Map(
            [("caminho".to_string(), Value::Str("/".to_string()))]
                .into_iter()
                .collect(),
        );
        let synth = synthetic_program(&program, "h", &req).expect("sintetico");
        let kept: Vec<String> = synth
            .statements
            .iter()
            .map(|n| format!("{:?}", n.stmt))
            .collect();
        assert!(
            kept.iter().any(|s| s.contains("FnDef")),
            "handler removido: {kept:?}"
        );
        assert!(
            !kept.iter().any(|s| s.contains("http_serve")),
            "serve nao filtrado: {kept:?}"
        );
        let src_if = "fn h(req) { return \"oi\"; }\nif (true) { http_serve(18092, \"h\", {}); }\n";
        let program_if = rhp_parser::parse_source(src_if).expect("parse");
        let synth_if = synthetic_program(&program_if, "h", &req).expect("sintetico");
        assert!(
            !synth_if
                .statements
                .iter()
                .any(|n| format!("{:?}", n.stmt).contains("http_serve")),
            "serve em `if` nao filtrado"
        );
    }

    #[test]
    fn rate_limit_allows_burst_then_blocks() {
        let mut contadores = HashMap::new();
        // limite 2/s: 2 passam, 3o bloqueia, proximo segundo reseta.
        assert!(permite(&mut contadores, ip(1), 100, 2));
        assert!(permite(&mut contadores, ip(1), 100, 2));
        assert!(!permite(&mut contadores, ip(1), 100, 2));
        assert!(permite(&mut contadores, ip(1), 101, 2));
        // IPs independentes; 0 = sem limite.
        assert!(permite(&mut contadores, ip(2), 101, 2));
        assert!(permite(&mut contadores, ip(1), 101, 0));
    }

    #[test]
    fn rejects_bad_serve_opts() {
        let bad: HashMap<String, Value> =
            [("teto".to_string(), Value::Int(1))].into_iter().collect();
        assert!(parse_serve_opts(&bad).is_err());
        let bad: HashMap<String, Value> = [("limite_taxa".to_string(), Value::Int(-1))]
            .into_iter()
            .collect();
        assert!(parse_serve_opts(&bad).is_err());
        let ok: HashMap<String, Value> = [("limite_taxa".to_string(), Value::Int(10))]
            .into_iter()
            .collect();
        let opts = parse_serve_opts(&ok).expect("opts validos");
        assert_eq!(opts.limite_taxa, 10);
        // `host`: IP literal ok; `localhost` vira 127.0.0.1; resto rejeitado.
        for bom in ["127.0.0.1", "0.0.0.0", "10.20.30.40", "::1"] {
            let m: HashMap<String, Value> = [("host".to_string(), Value::Str(bom.to_string()))]
                .into_iter()
                .collect();
            assert_eq!(parse_serve_opts(&m).expect("host valido").host, bom);
        }
        for alias in ["localhost", "LOCALHOST"] {
            let m: HashMap<String, Value> = [("host".to_string(), Value::Str(alias.to_string()))]
                .into_iter()
                .collect();
            assert_eq!(
                parse_serve_opts(&m).expect("localhost valido").host,
                "127.0.0.1"
            );
        }
        for ruim in ["", "999.1.1.1", "example.com", "0.0.0.0/0"] {
            let m: HashMap<String, Value> = [("host".to_string(), Value::Str(ruim.to_string()))]
                .into_iter()
                .collect();
            assert!(parse_serve_opts(&m).is_err(), "host aceito: {ruim}");
        }
        let m: HashMap<String, Value> = [("host".to_string(), Value::Int(1))].into_iter().collect();
        assert!(parse_serve_opts(&m).is_err());
        // Novos tetos: defaults e validacao.
        let d = ServeOpts::default();
        assert_eq!(d.host, "127.0.0.1");
        assert_eq!(d.max_passos, DEFAULT_MAX_PASSOS);
        assert_eq!(d.max_heap_bytes, 0);
        let m: HashMap<String, Value> = [
            ("max_passos".to_string(), Value::Int(100)),
            ("max_heap_bytes".to_string(), Value::Int(1024)),
        ]
        .into_iter()
        .collect();
        let opts = parse_serve_opts(&m).expect("tetos validos");
        assert_eq!(opts.max_passos, 100);
        assert_eq!(opts.max_heap_bytes, 1024);
        let m: HashMap<String, Value> = [("max_passos".to_string(), Value::Int(-1))]
            .into_iter()
            .collect();
        assert!(parse_serve_opts(&m).is_err());
    }

    #[test]
    fn percent_decode_preserva_utf8() {
        assert_eq!(percent_decode("ola%20mundo", true), "ola mundo");
        assert_eq!(percent_decode("caf%C3%A9", true), "café");
        assert_eq!(percent_decode("%ZZ", true), "%ZZ");
        assert_eq!(percent_decode("a+b", true), "a b");
        assert_eq!(percent_decode("a+b", false), "a+b");
        assert_eq!(percent_decode("a%2Bb", false), "a+b");
        assert_eq!(percent_decode("%", true), "%");
    }

    #[test]
    fn target_normalization_and_rejections() {
        // HTTP-001: `+` so vira espaco em query, nunca em caminho.
        assert_eq!(percent_decode("a+b", false), "a+b");
        // HTTP-008: forma absoluta normaliza para origem.
        assert_eq!(strip_absolute_form("http://h/abc?q=1"), "/abc?q=1");
        assert_eq!(strip_absolute_form("https://h/"), "/");
        assert_eq!(strip_absolute_form("https://h"), "/");
        assert_eq!(strip_absolute_form("/rel?q=1"), "/rel?q=1");
    }

    #[test]
    fn read_request_rejects_obs_fold_and_nul() {
        // HTTP-007/009: obs-fold e NUL viram 400 (antes, passavam).
        use std::net::TcpListener;
        fn exchange(raw: &[u8]) -> String {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
            let port = listener.local_addr().expect("porta").port();
            let raw = raw.to_vec();
            std::thread::spawn(move || {
                let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).expect("conn");
                use std::io::Write;
                s.write_all(&raw).expect("envio");
                // Mantem aberto para o servidor ler com prazo.
                std::thread::sleep(std::time::Duration::from_millis(200));
            });
            let (mut stream, _) = listener.accept().expect("accept");
            match read_request(&mut stream) {
                Ok(_) => "ok".to_string(),
                Err(message) => message,
            }
        }
        let folded = exchange(b"GET /x HTTP/1.1\r\nHost: a\r\nX-Bad\r\n  cont\r\n\r\n");
        assert!(folded.contains("obs-fold"), "obteve: {folded}");
        let nul = exchange(b"GET /a\x00b HTTP/1.1\r\nHost: a\r\n\r\n");
        assert!(nul.contains("NUL"), "obteve: {nul}");
        let ok = exchange(b"GET /a+b?q=x+y HTTP/1.1\r\nHost: a\r\n\r\n");
        assert_eq!(ok, "ok");
    }
}
