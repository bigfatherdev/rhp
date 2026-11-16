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
use rhp_parser::{CallArg, Expr, Program, Stmt};
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

/// Opcoes de `http_serve` (tudo opcional).
#[derive(Debug, Clone)]
pub struct ServeOpts {
    pub max_conexoes: u64,
    /// 0 = servir para sempre; N = servir N requests e retornar.
    pub max_requisicoes: u64,
    /// Teto de requests/segundo por IP (0 = sem limite).
    pub limite_taxa: u64,
}

impl Default for ServeOpts {
    fn default() -> Self {
        Self {
            max_conexoes: 32,
            max_requisicoes: 0,
            limite_taxa: 0,
        }
    }
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
        other => {
            return Err(VmError::new(format!(
                "`http_serve`: handler exige string (nome de `fn`), encontrado {}",
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

/// Valida o mapa de `opts` (`max_conexoes`, `max_requisicoes`, `limite_taxa`).
fn parse_serve_opts(opts: &HashMap<String, Value>) -> Result<ServeOpts, VmError> {
    let mut parsed = ServeOpts::default();
    for (key, value) in opts {
        match key.as_str() {
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
            unknown => {
                return Err(VmError::new(format!(
                    "`http_serve`: opcao desconhecida `{unknown}` (use `max_conexoes`, `max_requisicoes`, `limite_taxa`)"
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
    // Falha rapida: handler precisa existir e o programa precisa compilar.
    preflight(program, handler)?;
    let listener = TcpListener::bind(("127.0.0.1", port as u16))
        .map_err(|e| VmError::new(format!("`http_serve`: bind na porta {port}: {e}")))?;
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
            Err(_) => break,
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
        std::thread::spawn(move || {
            if let Some(code) = handle_one(stream, &worker_handler, &worker_program) {
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
        .any(|stmt| matches!(stmt, Stmt::FnDef { name, .. } if name == handler));
    if !found {
        return Err(VmError::new(format!(
            "`http_serve`: handler `{handler}` nao definido"
        )));
    }
    compile_handler_call(program, handler, &Value::Null)
        .map_err(|e| VmError::new(format!("`http_serve`: {e}")))?;
    Ok(())
}

/// Programa sintetico por request: todas as `fn` de topo + chamada.
fn synthetic_program(
    program: &AstProgram,
    handler: &str,
    request: &Value,
) -> Result<Program, String> {
    let mut statements: Vec<Stmt> = program
        .statements
        .iter()
        .filter(|stmt| matches!(stmt, Stmt::FnDef { .. }))
        .cloned()
        .collect();
    statements.push(Stmt::ExprStmt {
        expr: Expr::Call {
            name: handler.to_string(),
            args: vec![CallArg::Pos(expr_from_value(request)?)],
        },
    });
    Ok(Program { statements })
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
/// Devolve `Some(codigo)` se `exit()` foi chamado no handler.
fn handle_one(mut stream: TcpStream, handler: &str, program: &Arc<AstProgram>) -> Option<i32> {
    stream
        .set_read_timeout(Some(Duration::from_secs(IO_TIMEOUT_SECS)))
        .and_then(|()| stream.set_write_timeout(Some(Duration::from_secs(IO_TIMEOUT_SECS))))
        .ok()?;
    let request = match read_request(&mut stream) {
        Ok(request) => request,
        Err(message) => {
            eprintln!("rhp: request invalido: {message}");
            let _ = respond_500(&stream);
            return None;
        }
    };
    match run_handler(&request, handler, program) {
        Ok(response) => {
            if let Err(message) = write_response(&stream, &response) {
                eprintln!("rhp: responder falhou: {message}");
            }
            None
        }
        Err(HandlerFail::Exit(code)) => Some(code),
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
    let mut raw: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
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
    let (path, query) = match target.find('?') {
        Some(pos) => (
            percent_decode(&target[..pos]),
            parse_query(&target[pos + 1..]),
        ),
        None => (percent_decode(&target), HashMap::new()),
    };
    let mut headers = HashMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            let entry = headers
                .entry(name.trim().to_lowercase())
                .or_insert_with(String::new);
            if !entry.is_empty() {
                entry.push_str(", ");
            }
            entry.push_str(value.trim());
        }
    }
    let mut body = raw[header_end + 4..].to_vec();
    let want: usize = headers
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    if want > MAX_BODY {
        return Err("corpo grande demais".to_string());
    }
    while body.len() < want {
        let n = stream
            .read(&mut chunk)
            .map_err(|e| format!("ler corpo: {e}"))?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
        if body.len() > MAX_BODY {
            return Err("corpo grande demais".to_string());
        }
    }
    body.truncate(want.min(body.len()));
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

/// Decodifica `%XX` e `+` (query); invalido = mantem literal.
fn percent_decode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push((h * 16 + l) as char);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            out.push(' ');
        } else {
            out.push(bytes[i] as char);
        }
        i += 1;
    }
    out
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
                map.insert(percent_decode(k), percent_decode(v));
            }
            None => {
                map.insert(percent_decode(pair), String::new());
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
        500 => "Internal Server Error",
        503 => "Service Unavailable",
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
    }
}
