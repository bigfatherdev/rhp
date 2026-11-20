//! Maquina virtual baseada em pilha para o RHP (Fase 2 / Marco 2).
//!
//! Executa [`Chunk`] validado com frames explicitos, escopos lexicos e
//! globais compartilhadas. O bytecode e tratado como entrada potencialmente
//! nao confiavel e sempre validado (recursivamente) antes da execucao.

use rhp_bytecode::{
    builtin_arity, Builtin, Chunk, ClassDef, Frame, FuncDef, FuncValue, GenHandle, GenState,
    Handler, Op, PendingHof, Value, MAX_VALUE_DEPTH,
};
use rhp_parser::{Program, Stmt, StmtNode, TypeTag};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::io::Write;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::Duration;

mod builtins;
use builtins::call_builtin;

pub mod extensions;
pub use builtins::MAX_HTTP_BODY;

mod http_server;
mod json;
mod mysql;
mod pg;
mod redis;
mod sqlite;
mod tcp;
mod time;
mod tls;

/// Profundidade maxima da pilha de chamadas (protecao contra estouro).
pub const MAX_CALL_DEPTH: usize = 512;

/// Limites opcionais de execucao (opt-in; padrao = ilimitado).
///
/// Preserva compatibilidade: sem limites configurados o comportamento e o
/// mesmo de antes. `max_steps` aborta loops; `max_stack_depth` limita a
/// pilha de operandos por frame; `max_collection_len` limita tamanho em
/// bytes (strings) ou elementos (arrays/mapas) de cada valor empilhado;
/// `max_heap_bytes` limita a soma estimada (bytes do modelo, RFC 0015)
/// da heap viva — `None` = ilimitado.
#[derive(Debug, Clone, Default)]
pub struct VmLimits {
    pub max_steps: Option<u64>,
    pub max_stack_depth: Option<usize>,
    pub max_collection_len: Option<usize>,
    pub max_heap_bytes: Option<u64>,
}

/// Erro de runtime da VM.
///
/// `message` e estavel (vira valor em `try`/`catch`); `trace` e so
/// diagnostico da CLI (pilha de chamadas, da mais interna para a mais
/// externa; Etapa 4, DevX).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmError {
    pub message: String,
    /// Codigo de `exit(codigo)` quando o erro e um pedido de saida;
    /// `None` para erros comuns. Leitura publica; construcao interna.
    exit_code: Option<i32>,
    trace: Vec<String>,
}

impl VmError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            exit_code: None,
            trace: Vec::new(),
        }
    }

    /// Pedido de `exit(codigo)` (0–255): desenrola frames como erro, mas a
    /// CLI o traduz em codigo de saida em vez de mensagem de falha.
    pub(crate) fn exit(code: i32) -> Self {
        Self {
            message: format!("exit({code})"),
            exit_code: Some(code),
            trace: Vec::new(),
        }
    }

    /// Codigo de saida pedido por `exit`, se for o caso.
    #[must_use]
    pub fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }

    /// Pilha de chamadas no momento do erro (nomes de funcao, da mais
    /// interna para a mais externa; vazia no nivel superior).
    #[must_use]
    pub fn trace(&self) -> &[String] {
        &self.trace
    }

    /// Anexa a pilha de chamadas (uso interno do `run`).
    fn with_trace(mut self, trace: Vec<String>) -> Self {
        self.trace = trace;
        self
    }
}

impl fmt::Display for VmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for VmError {}

/// Argumentos separados de chamada mista: posicionais + nomeados
/// (RFC 0009; reusado por `new` e chamada de metodo, RFC 0017).
type MixedArgs = (Vec<Value>, Vec<(String, Value)>);

/// Passo de execucao: continua no mesmo frame ou termina (`Halt`).
enum Step {
    Continue,
    Done,
}

/// Maquina virtual com pilha de frames e mapa de globais.
#[derive(Debug, Default)]
pub struct Vm {
    frames: Vec<Frame>,
    globals: HashMap<String, Value>,
    limits: VmLimits,
    steps: u64,
    /// Traco de execucao (`rhp run --trace`, Etapa 4): imprime `ip` + op
    /// em stderr a cada passo. So diagnostico; nao afeta a semantica.
    trace: bool,
    argv: Vec<String>,
    /// Programa-fonte (AST) para recompilacao por thread (`http_serve`;
    /// so dados owned, `Send+Sync`).
    program: Option<Arc<Program>>,
    /// Conexoes de banco abertas (`db_open`); chave = handle `Int`.
    db: DbHandles,
    /// Estado do xorshift64* para `rand`/`rand_range` (RFC 0012; semeado
    /// uma vez em `new`, com relogio+pid; NAO criptografico).
    rng: u64,
    /// Estado `static` por definicao de funcao (RFC 0010): lista de
    /// `(definicao, variaveis)` comparada por identidade (`Rc::ptr_eq`;
    /// poucas definicoes usam `static`, busca linear e suficiente).
    /// Compartilhado entre chamadas e instancias da mesma definicao.
    statics: Vec<(Rc<FuncDef>, HashMap<String, Value>)>,
    /// Bytes estimados empilhados desde a ultima varredura da heap
    /// (teto amortizado O(1), RFC 0015; zerado em `set_limits`).
    heap_since_check: u64,
    /// Sockets TCP crus (`tcp_*`; RFC 0020).
    tcp: tcp::TcpHandles,
    /// Conexoes Redis (`redis_*`; RFC 0020).
    redis: redis::RedisHandles,
    /// Tarefas assincronas em voo (`async_spawn`/`async_join`, RFC 0019).
    tasks: TaskHandles,
    /// Extensoes nativas ja carregadas (`import native`; Fase Next-Gen 6).
    /// Segunda carga da mesma extensao = erro (evita sombra silenciosa).
    loaded_extensions: std::collections::HashSet<String>,
    /// Flag cooperativa de cancelamento desta tarefa (item RFC 0030):
    /// `Some` dentro de workers (lida por `async_cancelled`), `None` na VM
    /// principal (fora de tarefa = nunca cancelada).
    cancel_flag: Option<Arc<AtomicBool>>,
}

/// Tarefas assincronas por handle (RFC 0019): cada uma roda em thread
/// propria com programa recompilado; resultado via JSON (fronteira).
#[derive(Debug, Default)]
struct TaskHandles {
    running: HashMap<i64, TaskThread>,
    next: i64,
}

/// Tarefa em voo: canal de resultado + flag cooperativa de cancelamento +
/// nome da funcao (para diagnostico). A thread e destacada: envia o
/// resultado pelo canal e termina (sem `join`; panico = canal fechado).
/// (Item RFC 0030: timeout/cancelamento/all/race.)
#[derive(Debug)]
struct TaskThread {
    name: String,
    rx: Receiver<TaskResult>,
    cancel: Arc<AtomicBool>,
}

/// Resultado que cruza a fronteira da thread (JSON serializado; `String`
/// e `Send`, ao contrario de `Value`).
type TaskResult = Result<String, String>;

/// Dispara a thread da tarefa (item RFC 0030): a thread executa, envia o
/// resultado pelo canal e termina destacada (sem `join`; panico = canal
/// fechado, detectado como `abortou (panic)` no join).
/// Devolve `(rx, cancel)`: o receptor do resultado e a flag cooperativa.
fn spawn_task_thread(
    program: Arc<Program>,
    argv: Vec<String>,
    name: String,
    args_json: String,
    env_json: String,
) -> (Receiver<TaskResult>, Arc<AtomicBool>) {
    let cancel = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();
    let thread_cancel = Arc::clone(&cancel);
    std::thread::spawn(move || {
        let result = run_task_with_args(program, argv, name, args_json, env_json, thread_cancel);
        let _ = tx.send(result);
    });
    (rx, cancel)
}

/// Valida segundos para `await_timeout` (item RFC 0030): numero, finito,
/// `0 <= s <= 3600` (mesmos limites do `sleep`; `0` = sondagem sem bloqueio).
fn timeout_secs(value: &Value, builtin: &str) -> Result<f64, VmError> {
    let secs = match value {
        Value::Int(v) => *v as f64,
        Value::Float(v) => *v,
        other => {
            return Err(VmError::new(format!(
                "`{builtin}` exige segundos (numero), encontrado {}",
                type_name(other)
            )));
        }
    };
    if !secs.is_finite() || secs < 0.0 {
        return Err(VmError::new(format!(
            "`{builtin}` exige segundos >= 0, encontrado {secs}"
        )));
    }
    if secs > 3600.0 {
        return Err(VmError::new(format!(
            "`{builtin}` excedeu o teto (3600 segundos)"
        )));
    }
    Ok(secs)
}

/// Lista as extensoes disponiveis (para mensagens de erro).
fn available_extensions() -> String {
    let mut names: Vec<String> = extensions::registry().keys().cloned().collect();
    names.sort();
    if names.is_empty() {
        return "(nenhuma)".to_string();
    }
    names.join(", ")
}

/// Chave reservada do envelope de instancia na fronteira async.
/// Mapas do usuario contendo esta chave (em qualquer nivel) sao rejeitados
/// no spawn para nao confundir carga util com envelope.
const INSTANCE_TAG: &str = "__rhp_instancia__";

/// Valores que atravessam a fronteira da thread: planos (`Null/Bool/Int/
/// Float/Str/Array/Map` recursivos) e instancias (com classe, via envelope).
/// Funcao, classe e gerador nao atravessam. Mapas com a chave reservada do
/// envelope sao rejeitados (evita ambiguidade na decodificacao).
fn check_shareable(value: &Value) -> Result<(), String> {
    match value {
        Value::Null | Value::Bool(_) | Value::Int(_) | Value::Float(_) | Value::Str(_) => Ok(()),
        Value::Array(items) => items.iter().try_for_each(check_shareable),
        Value::Map(entries) => {
            if entries.contains_key(INSTANCE_TAG) {
                return Err(format!(
                    "mapa usa chave reservada `{INSTANCE_TAG}` (renomeie a chave)"
                ));
            }
            entries.values().try_for_each(check_shareable)
        }
        Value::Instance { fields, .. } => fields.values().try_for_each(check_shareable),
        other => Err(format!(
            "`async` nao transporta {} (use valores planos ou instancias de classes do programa)",
            type_name(other)
        )),
    }
}

/// Codifica valores para a fronteira: instancias viram envelope
/// `{"__rhp_instancia__": {"classe": Nome, "campos": {...}}}` (recursivo);
/// resto passa como esta (ja validado por `check_shareable`).
fn encode_shareable(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(encode_shareable).collect()),
        Value::Map(entries) => {
            let mut out = HashMap::with_capacity(entries.len());
            for (k, v) in entries {
                out.insert(k.clone(), encode_shareable(v));
            }
            Value::Map(out)
        }
        Value::Instance { class, fields } => {
            let mut campos = HashMap::new();
            for (k, v) in fields {
                campos.insert(k.clone(), encode_shareable(v));
            }
            let mut inner = HashMap::new();
            inner.insert("classe".to_string(), Value::Str(class.name.clone()));
            inner.insert("campos".to_string(), Value::Map(campos));
            let mut out = HashMap::new();
            out.insert(INSTANCE_TAG.to_string(), Value::Map(inner));
            Value::Map(out)
        }
        other => other.clone(),
    }
}

/// Decodifica o envelope de volta em instancia (item metodos-async).
/// `lookup` resolve a classe no VM local (worker ou chamador).
/// So o envelope EXATO (`{"__rhp_instancia__": {"classe": Str, "campos": Map}}`)
/// vira instancia; resto (incluido mapas comuns) passa como esta.
fn decode_shareable(
    value: Value,
    lookup: &dyn Fn(&str) -> Option<Rc<ClassDef>>,
) -> Result<Value, String> {
    match value {
        Value::Array(items) => items
            .into_iter()
            .map(|v| decode_shareable(v, lookup))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        Value::Map(mut entries) => {
            if entries.len() == 1 {
                if let Some(tag) = entries.remove(INSTANCE_TAG) {
                    if let Value::Map(inner) = tag {
                        let name = match inner.get("classe") {
                            Some(Value::Str(s)) => s.clone(),
                            _ => return Err("envelope de instancia invalido".to_string()),
                        };
                        let fields = match inner.get("campos") {
                            Some(Value::Map(m)) => m.clone(),
                            _ => return Err("envelope de instancia invalido".to_string()),
                        };
                        let class = lookup(&name)
                            .ok_or_else(|| format!("classe `{name}` indisponivel na tarefa"))?;
                        let mut dec = HashMap::with_capacity(fields.len());
                        for (k, v) in fields {
                            dec.insert(k, decode_shareable(v, lookup)?);
                        }
                        return Ok(Value::Instance { class, fields: dec });
                    }
                    return Err("envelope de instancia invalido".to_string());
                }
            }
            let mut out = HashMap::with_capacity(entries.len());
            for (k, v) in entries {
                out.insert(k, decode_shareable(v, lookup)?);
            }
            Ok(Value::Map(out))
        }
        other => Ok(other),
    }
}

/// Procura a definicao pelo nome em toda a arvore de chunks (item 1):
/// closures aninhadas vivem no chunk do corpo da funcao que as contem, nao
/// no raiz. Nomes de closure sao unicos (`{closure:N}`), entao o primeiro
/// match e o exato. Limite de profundidade contra bytecode adversarial.
fn find_def_by_name(chunk: &Chunk, name: &str) -> Option<Rc<FuncDef>> {
    fn dfs(chunk: &Chunk, name: &str, depth: usize) -> Option<Rc<FuncDef>> {
        if depth > 128 {
            return None;
        }
        for def in &chunk.funcs {
            if def.name == name {
                return Some(def.clone());
            }
        }
        for def in &chunk.funcs {
            if let Some(found) = dfs(&def.body, name, depth + 1) {
                return Some(found);
            }
        }
        None
    }
    dfs(chunk, name, 0)
}

/// Programa so com definicoes de topo (sem efeitos): o worker executa para
/// reconstruir o codigo compartilhado (funcoes via `DefineFunc`, classes via
/// `BuildClass`, extensoes via `LoadNative`). Todo o resto (lets com init
/// arbitrario, echos, controle, `http_serve`, etc.) e descartado: estado
/// mutavel nao atravessa a fronteira da thread (ver RFC 0023 e item 1).
fn defs_program(program: &Program) -> Program {
    let statements: Vec<StmtNode> = program
        .statements
        .iter()
        .filter(|node| {
            matches!(
                &node.stmt,
                Stmt::FnDef { .. }
                    | Stmt::ClassDef { .. }
                    | Stmt::InterfaceDef { .. }
                    | Stmt::TraitDef { .. }
                    | Stmt::Namespace { .. }
                    | Stmt::ImportNative { .. }
            )
        })
        .cloned()
        .collect();
    Program {
        statements,
        trailing_comments: Vec::new(),
    }
}

/// Variante com argumentos, captura e cancelamento (Fase Next-Gen 3 +
/// item RFC 0030): `args_json` e um array JSON (cada elemento vira um `Value`
/// posicional); `env_json` e um objeto JSON com o ambiente capturado
/// (fronteira `Send`); `cancel` e a flag cooperativa lida por
/// `async_cancelled` dentro da tarefa.
/// Recompila o programa (owned, padrao `http_serve`): executa as definicoes
/// de topo (codigo compartilhado, nao o estado), resolve a definicao pelo
/// nome (tabela cheia + globais) e executa, serializando o retorno em JSON.
/// Erros viram `Err(mensagem)` para o `join` re-lancar.
fn run_task_with_args(
    program: Arc<Program>,
    argv: Vec<String>,
    name: String,
    args_json: String,
    env_json: String,
    cancel: Arc<AtomicBool>,
) -> TaskResult {
    // 1) Codigo compartilhado: compila e executa as definicoes de topo.
    let defs = defs_program(&program);
    let defs_chunk = rhp_bytecode::compile(&defs).map_err(|e| format!("compilacao: {e}"))?;
    // Tabela cheia (mesmo programa do chamador): para resolucao por nome.
    let full_chunk = rhp_bytecode::compile(&program).map_err(|e| format!("compilacao: {e}"))?;
    let mut vm = Vm::new();
    vm.set_argv(argv);
    vm.program = Some(Arc::clone(&program));
    vm.cancel_flag = Some(cancel);
    {
        let mut sink = std::io::sink();
        vm.run(&defs_chunk, &mut sink)
            .map_err(|e| e.message.clone())?;
    }
    // 2) Alvo: nome na arvore de chunks ou global definida acima.
    let def =
        find_def_by_name(&full_chunk, &name).or_else(|| match vm.globals.get(&name).cloned() {
            Some(Value::Func(func)) => Some(func.def),
            _ => None,
        });
    let def = match def {
        Some(def) => def,
        None => return Err(format!("funcao `{name}` inexistente")),
    };
    if def.is_generator {
        return Err(format!("`async_spawn` nao suporta gerador (`{name}`)"));
    }
    // Classes disponiveis no worker (para decodificar instancias).
    let lookup = |class_name: &str| match vm.globals.get(class_name).cloned() {
        Some(Value::Class(class)) => Some(class),
        _ => None,
    };
    let args_value =
        crate::json::parse_json(&args_json).map_err(|e| format!("argumentos invalidos: {e}"))?;
    let args = match args_value {
        Value::Array(items) => items
            .into_iter()
            .map(|v| decode_shareable(v, &lookup))
            .collect::<Result<Vec<_>, _>>()?,
        _ => return Err("argumentos devem ser array".to_string()),
    };
    let env_value =
        crate::json::parse_json(&env_json).map_err(|e| format!("ambiente invalido: {e}"))?;
    let captured = match env_value {
        Value::Map(entries) => {
            let mut dec = HashMap::with_capacity(entries.len());
            for (k, v) in entries {
                dec.insert(k, decode_shareable(v, &lookup)?);
            }
            dec
        }
        _ => return Err("ambiente deve ser objeto".to_string()),
    };
    let func = FuncValue {
        def: def.clone(),
        env: captured,
    };
    // Frame raiz com `Halt` (sem limpar as globais das definicoes): sem ele,
    // o `return` da tarefa pareceria "fora de funcao"; com ele, o `drive`
    // termina e devolve o valor.
    let mut root = rhp_bytecode::Chunk::new();
    root.code.push(rhp_bytecode::Op::Halt);
    vm.frames.clear();
    vm.push_root(&root);
    vm.push_call_frame(func, args)
        .map_err(|e| e.message.clone())?;
    let mut sink = std::io::sink();
    let value = vm.drive(&mut sink).map_err(|e| e.message.clone())?;
    // Retorno cruza codificado (instancias com classe); o join decodifica.
    let encoded = encode_shareable(&value);
    crate::json::stringify_json(&encoded).map_err(|e| e.message.clone())
}

/// Conexoes de banco da VM (B2: SQLite; B3: +PostgreSQL).
#[derive(Debug, Default)]
struct DbHandles {
    conns: HashMap<i64, DbConn>,
    next: i64,
}

/// Conexao de banco por tras de um handle (`db_*`).
#[derive(Debug)]
enum DbConn {
    Sqlite(sqlite::Connection),
    Postgres(pg::Connection),
    Mysql(mysql::Connection),
}

/// Semente do xorshift64* de `rand` (RFC 0012): relogio (ns) misturado com
/// o pid; zero e evitado (xorshift trava em 0). std-only, sem cripto.
fn seed_rng() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15);
    let pid = std::process::id() as u64;
    nanos
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(pid.wrapping_mul(0xBF58_476D_1CE4_E5B9))
        | 1
}

impl Vm {
    #[must_use]
    pub fn new() -> Self {
        Self {
            frames: Vec::new(),
            globals: HashMap::new(),
            limits: VmLimits::default(),
            steps: 0,
            trace: false,
            argv: Vec::new(),
            program: None,
            db: DbHandles::default(),
            statics: Vec::new(),
            rng: seed_rng(),
            heap_since_check: 0,
            tasks: TaskHandles::default(),
            tcp: tcp::TcpHandles::default(),
            redis: redis::RedisHandles::default(),
            loaded_extensions: std::collections::HashSet::new(),
            cancel_flag: None,
        }
    }

    /// Liga/desliga o traco de execucao (`rhp run --trace`).
    pub fn set_trace(&mut self, trace: bool) {
        self.trace = trace;
    }

    /// Define os argumentos de linha de comando visiveis via `argv()`.
    pub fn set_argv(&mut self, args: Vec<String>) {
        self.argv = args;
    }

    /// Define limites opcionais de execucao (ver [`VmLimits`]).
    pub fn set_limits(&mut self, limits: VmLimits) {
        self.limits = limits;
        self.heap_since_check = 0;
    }

    /// Define o programa-fonte para recompilacao por thread (`http_serve`).
    pub fn set_program(&mut self, program: Arc<Program>) {
        self.program = Some(program);
    }

    /// Executa o chunk escrevendo a saida de `Print` em `output`.
    ///
    /// O chunk e validado (recursivamente) antes da execucao; bytecode
    /// invalido nunca e executado parcialmente.
    ///
    /// # Erros
    ///
    /// Retorna [`VmError`] em bytecode invalido, erros de tipo, variavel
    /// indefinida, condicao nao-bool, aridade divergente, `return` fora de
    /// funcao, limite de chamadas excedido, falha de I/O, entre outros.
    pub fn run(&mut self, chunk: &Chunk, output: &mut dyn Write) -> Result<(), VmError> {
        chunk
            .validate()
            .map_err(|e| VmError::new(format!("bytecode invalido: {e}")))?;
        self.reset(chunk);
        self.drive(output).map(|_| ())
    }

    /// Executa ate `Halt` (ou erro), devolvendo o valor no topo da pilha
    /// em `Done` (`Null` se vazia). Nucleo compartilhado de `run`,
    /// `run_value` e `run_continue` (com trace anexado no erro).
    fn drive(&mut self, output: &mut dyn Write) -> Result<Value, VmError> {
        while !self.frames.is_empty() {
            let idx = self.frames.len() - 1;
            match self.step(idx, output) {
                Ok(Step::Continue) => {}
                Ok(Step::Done) => {
                    let value = self.frames[idx].stack.pop().unwrap_or(Value::Null);
                    return Ok(value);
                }
                Err(error) => return Err(error.with_trace(self.call_stack())),
            }
        }
        Err(VmError::new("execucao terminou sem `Halt`"))
    }

    /// Executa um chunk sobre o estado atual, sem reiniciar (REPL, Etapa 4):
    /// globais, statics, argv e limites sobrevivem entre linhas; frames e
    /// passos reiniciam. Erro descarta os frames, preservando o estado.
    pub fn run_continue(&mut self, chunk: &Chunk, output: &mut dyn Write) -> Result<(), VmError> {
        chunk
            .validate()
            .map_err(|e| VmError::new(format!("bytecode invalido: {e}")))?;
        self.unwind_all();
        self.steps = 0;
        self.push_root(chunk);
        let result = self.drive(output).map(|_| ());
        if result.is_err() {
            self.unwind_all();
        }
        result
    }

    /// Pilha de chamadas ativas (nomes de funcao, da mais interna para a
    /// mais externa). Frames sem funcao (nivel superior) nao aparecem.
    fn call_stack(&self) -> Vec<String> {
        self.frames
            .iter()
            .rev()
            .filter_map(|frame| frame.func.as_ref().map(|def| def.name.clone()))
            .collect()
    }

    /// Executa o chunk e devolve o valor no topo da pilha (`Null` se vazia).
    /// Usado pelo `http_serve` para capturar o retorno do handler.
    pub fn run_value(&mut self, chunk: &Chunk, output: &mut dyn Write) -> Result<Value, VmError> {
        chunk
            .validate()
            .map_err(|e| VmError::new(format!("bytecode invalido: {e}")))?;
        self.reset(chunk);
        self.drive(output)
    }

    /// Reinicia frames/globais/passos com um chunk validado.
    /// Tarefas em voo sao descartadas (threads seguem detached;
    /// `join` pendente vira `handle invalido`).
    fn reset(&mut self, chunk: &Chunk) {
        self.frames.clear();
        self.globals.clear();
        self.statics.clear();
        self.steps = 0;
        self.tasks = TaskHandles::default();
        self.push_root(chunk);
    }

    /// Empilha o frame raiz de um chunk validado (sem tocar em globais,
    /// statics, argv ou limites).
    fn push_root(&mut self, chunk: &Chunk) {
        self.frames.push(Frame {
            chunk: Rc::new(chunk.clone()),
            ip: 0,
            stack: Vec::new(),
            scopes: vec![HashMap::new()],
            handlers: Vec::new(),
            pending: None,
            pending_cb: None,
            pending_gen: None,
            pending_new: None,
            pending_method: None,
            pending_super: false,
            func: None,
        });
    }

    /// Desenrola frames ate a base, descartando pilhas/escopos/handlers.
    /// Globais e statics sobrevivem (estado do REPL entre linhas).
    fn unwind_all(&mut self) {
        self.frames.clear();
    }

    /// Executa e retorna a saida como `String` (conveniencia para testes).
    ///
    /// # Erros
    ///
    /// Retorna [`VmError`] nas mesmas condicoes de [`Vm::run`], alem de
    /// falha de conversao UTF-8 (nao esperada no Marco 2).
    pub fn run_to_string(&mut self, chunk: &Chunk) -> Result<String, VmError> {
        let mut buf: Vec<u8> = Vec::new();
        self.run(chunk, &mut buf)?;
        String::from_utf8(buf).map_err(|e| VmError::new(format!("saida nao e UTF-8 valido: {e}")))
    }

    /// Executa uma instrucao com interceptacao de erros (RFC 0001).
    ///
    /// Erro sem `exit_code` tenta unwinding ate o `try` mais interno (do
    /// topo para a base); sem handler, o erro original propaga intacto.
    /// `exit()` nunca e capturado.
    fn step(&mut self, idx: usize, output: &mut dyn Write) -> Result<Step, VmError> {
        match self.step_inner(idx, output) {
            Ok(outcome) => Ok(outcome),
            Err(error) => {
                if error.exit_code().is_some() {
                    return Err(error);
                }
                let value = Self::internal_error_value(&error.message);
                self.unwind(value, error)
            }
        }
    }

    fn step_inner(&mut self, idx: usize, output: &mut dyn Write) -> Result<Step, VmError> {
        self.steps += 1;
        if let Some(max) = self.limits.max_steps {
            if self.steps > max {
                return Err(VmError::new(format!(
                    "limite de instrucoes excedido ({max} passos)"
                )));
            }
        }
        let (ip, op) = {
            let frame = self
                .frames
                .get(idx)
                .ok_or_else(|| VmError::new("frame de execucao inexistente"))?;
            let op = *frame
                .chunk
                .code
                .get(frame.ip)
                .ok_or_else(|| VmError::new("ponteiro de instrucao fora do codigo"))?;
            (frame.ip, op)
        };
        if self.trace {
            eprintln!("[trace] f{idx}:{ip} {op:?}");
        }
        match op {
            Op::Constant(const_idx) => {
                let value = self.frames[idx]
                    .chunk
                    .constants
                    .get(const_idx as usize)
                    .cloned()
                    .ok_or_else(|| VmError::new(format!("constante {const_idx} inexistente")))?;
                self.push_checked(idx, value)?;
                self.frames[idx].ip += 1;
            }
            Op::Print => {
                let value = self.frames[idx]
                    .stack
                    .pop()
                    .ok_or_else(|| VmError::new("pilha vazia em `Print`"))?;
                writeln!(output, "{value}")
                    .map_err(|e| VmError::new(format!("falha ao escrever saida: {e}")))?;
                self.frames[idx].ip += 1;
            }
            Op::Halt => {
                if self.frames.len() == 1 {
                    return Ok(Step::Done);
                }
                return Err(VmError::new("`Halt` fora do programa principal"));
            }
            Op::Add => self.binary(idx, "soma", add_values)?,
            Op::Sub => self.binary(idx, "subtracao", sub_values)?,
            Op::Mul => self.binary(idx, "multiplicacao", mul_values)?,
            Op::Div => self.binary(idx, "divisao", div_values)?,
            Op::Mod => self.binary(idx, "resto", mod_values)?,
            Op::BitAnd => self.binary(idx, "`&`", bitand_values)?,
            Op::BitOr => self.binary(idx, "`|`", bitor_values)?,
            Op::BitXor => self.binary(idx, "`^`", bitxor_values)?,
            Op::Shl => self.binary(idx, "`<<`", shl_values)?,
            Op::Shr => self.binary(idx, "`>>`", shr_values)?,
            Op::Dup => {
                let value = self.frames[idx]
                    .stack
                    .last()
                    .cloned()
                    .ok_or_else(|| VmError::new("pilha vazia em duplicacao".to_string()))?;
                self.push_checked(idx, value)?;
                self.frames[idx].ip += 1;
            }
            Op::Neg => {
                let value = self.pop(idx, "operador `-`")?;
                let result = neg_value(&value)?;
                self.push_checked(idx, result)?;
                self.frames[idx].ip += 1;
            }
            Op::Not => {
                let value = self.pop(idx, "operador `!`")?;
                let result = not_value(&value)?;
                self.push_checked(idx, result)?;
                self.frames[idx].ip += 1;
            }
            Op::BitNot => {
                let value = self.pop(idx, "operador `~`")?;
                let result = bitnot_value(&value)?;
                self.push_checked(idx, result)?;
                self.frames[idx].ip += 1;
            }
            Op::Eq => self.binary(idx, "comparacao", |a, b| {
                Ok(Value::Bool(values_equal(a, b)))
            })?,
            Op::StrictEq => self.binary(idx, "comparacao", |a, b| {
                Ok(Value::Bool(strict_equal(a, b)))
            })?,
            Op::Ne => {
                self.binary(idx, "comparacao", |a, b| {
                    Ok(Value::Bool(!values_equal(a, b)))
                })?;
            }
            Op::Lt => self.binary(idx, "comparacao", |a, b| {
                compare_values(a, b, Ordering::Less)
            })?,
            Op::Le => {
                self.binary(idx, "comparacao", |a, b| {
                    compare_values(a, b, Ordering::LessOrEqual)
                })?;
            }
            Op::Gt => {
                self.binary(idx, "comparacao", |a, b| {
                    compare_values(a, b, Ordering::Greater)
                })?;
            }
            Op::Ge => {
                self.binary(idx, "comparacao", |a, b| {
                    compare_values(a, b, Ordering::GreaterOrEqual)
                })?;
            }
            Op::JumpIfFalse(target) => {
                let value = self.pop(idx, "condicao")?;
                match value {
                    Value::Bool(cond) => {
                        if cond {
                            self.frames[idx].ip += 1;
                        } else {
                            self.frames[idx].ip = target as usize;
                        }
                    }
                    other => {
                        return Err(VmError::new(format!(
                            "condicao deve ser bool, encontrado {}",
                            type_name(&other)
                        )));
                    }
                }
            }
            Op::JumpIfTrue(target) => {
                let value = self.pop(idx, "condicao")?;
                match value {
                    Value::Bool(cond) => {
                        if cond {
                            self.frames[idx].ip = target as usize;
                        } else {
                            self.frames[idx].ip += 1;
                        }
                    }
                    other => {
                        return Err(VmError::new(format!(
                            "condicao deve ser bool, encontrado {}",
                            type_name(&other)
                        )));
                    }
                }
            }
            Op::Jump(target) => {
                self.frames[idx].ip = target as usize;
            }
            Op::JumpIfNotNull(target) => {
                let value = self.pop(idx, "condicao nula")?;
                if matches!(value, Value::Null) {
                    self.frames[idx].ip += 1;
                } else {
                    self.frames[idx].ip = target as usize;
                }
            }
            Op::LoadName(name_idx) => {
                let name = self.name_of(idx, name_idx)?;
                let value = self
                    .lookup(idx, &name)
                    .ok_or_else(|| VmError::new(format!("variavel nao definida: {name}")))?;
                self.push_checked(idx, value)?;
                self.frames[idx].ip += 1;
            }
            Op::StoreName(name_idx) => {
                let value = self.pop(idx, "atribuicao")?;
                let name = self.name_of(idx, name_idx)?;
                if !self.assign(idx, &name, value) {
                    return Err(VmError::new(format!("variavel nao definida: {name}")));
                }
                self.frames[idx].ip += 1;
            }
            Op::DeclareName(name_idx) => {
                let value = self.pop(idx, "declaracao")?;
                let name = self.name_of(idx, name_idx)?;
                self.declare(idx, &name, value)?;
                self.frames[idx].ip += 1;
            }
            Op::DeclareStatic(name_idx) => {
                // Avaliado a cada chamada, atribuido so na primeira (RFC 0010).
                let value = self.pop(idx, "declaracao `static`")?;
                let name = self.name_of(idx, name_idx)?;
                let Some(func) = self.frames[idx].func.clone() else {
                    return Err(VmError::new(
                        "`static` fora de funcao (defesa em profundidade)".to_string(),
                    ));
                };
                self.static_vars(&func).entry(name).or_insert(value);
                self.frames[idx].ip += 1;
            }
            Op::EnterScope => {
                self.frames[idx].scopes.push(HashMap::new());
                self.frames[idx].ip += 1;
            }
            Op::ExitScope => {
                if self.frames[idx].scopes.len() <= 1 {
                    return Err(VmError::new("desequilibrio de escopo em `}`"));
                }
                self.frames[idx].scopes.pop();
                self.frames[idx].ip += 1;
            }
            Op::DefineFunc(func_idx) => {
                let func = self.frames[idx]
                    .chunk
                    .funcs
                    .get(func_idx as usize)
                    .cloned()
                    .ok_or_else(|| VmError::new(format!("funcao {func_idx} inexistente")))?;
                let name = func.name.clone();
                // Semantica uniforme (RFC 0003): funcao nomeada aninhada
                // tambem fecha sobre o ambiente de definicao.
                let env = self.capture_env(idx);
                self.declare(idx, &name, Value::Func(FuncValue { def: func, env }))?;
                self.frames[idx].ip += 1;
            }
            Op::MakeFunc(func_idx) => {
                let def = self.frames[idx]
                    .chunk
                    .funcs
                    .get(func_idx as usize)
                    .cloned()
                    .ok_or_else(|| VmError::new(format!("funcao {func_idx} inexistente")))?;
                let env = self.capture_env(idx);
                self.frames[idx]
                    .stack
                    .push(Value::Func(FuncValue { def, env }));
                self.frames[idx].ip += 1;
            }
            Op::Call(arity) => {
                self.call(idx, arity)?;
            }
            Op::CallMixed(desc_idx) => {
                self.call_mixed(idx, desc_idx)?;
            }
            Op::BuildClass(name_idx, count) => {
                // Desempilha `count` pares [nome(Str), metodo(Func)] e
                // empilha a classe (tabela de metodos compartilhada).
                let name = self.name_of(idx, name_idx)?;
                let mut methods = HashMap::new();
                for _ in 0..count {
                    let func = self.pop(idx, "metodo de classe")?;
                    let method_name = self.pop(idx, "nome de metodo")?;
                    let (method_name, func) = match (method_name, func) {
                        (Value::Str(method_name), Value::Func(func)) => (method_name, func),
                        (method_name, func) => {
                            return Err(VmError::new(format!(
                                "bytecode invalido: metodo de classe espera (Str, Func), encontrou ({} {})",
                                type_name(&method_name),
                                type_name(&func),
                            )));
                        }
                    };
                    if methods.insert(method_name.clone(), func).is_some() {
                        return Err(VmError::new(format!(
                            "metodo duplicado `{method_name}` (invalido fora do compilador)"
                        )));
                    }
                }
                self.push_checked(
                    idx,
                    Value::Class(Rc::new(ClassDef {
                        name,
                        parent: None,
                        methods,
                    })),
                )?;
                self.frames[idx].ip += 1;
            }
            Op::BuildClassEx(name_idx, parent_idx, count) => {
                let name = self.name_of(idx, name_idx)?;
                let parent_name = self.name_of(idx, parent_idx)?;
                let mut methods = HashMap::new();
                for _ in 0..count {
                    let func = self.pop(idx, "metodo de classe")?;
                    let method_name = self.pop(idx, "nome de metodo")?;
                    let (method_name, func) = match (method_name, func) {
                        (Value::Str(method_name), Value::Func(func)) => (method_name, func),
                        (method_name, func) => {
                            return Err(VmError::new(format!(
                                "bytecode invalido: metodo de classe espera (Str, Func), encontrou ({} {})",
                                type_name(&method_name),
                                type_name(&func),
                            )));
                        }
                    };
                    if methods.insert(method_name.clone(), func).is_some() {
                        return Err(VmError::new(format!(
                            "metodo duplicado `{method_name}` (invalido fora do compilador)"
                        )));
                    }
                }
                self.push_checked(
                    idx,
                    Value::Class(Rc::new(ClassDef {
                        name,
                        parent: Some(parent_name),
                        methods,
                    })),
                )?;
                self.frames[idx].ip += 1;
            }
            Op::IsInstanceOf(name_idx) => {
                let type_name_want = self.name_of(idx, name_idx)?;
                let value = self.pop(idx, "instancia")?;
                let matched = match &value {
                    Value::Instance { class, .. } => {
                        self.is_subclass_of(idx, class, &type_name_want)
                    }
                    _ => false,
                };
                self.push_checked(idx, Value::Bool(matched))?;
                self.frames[idx].ip += 1;
            }
            Op::Await => {
                let value = self.pop(idx, "`await`")?;
                match value {
                    Value::Int(id) if self.tasks.running.contains_key(&id) => {
                        let task = self.tasks.running.remove(&id).expect("handle verificado");
                        let v = self.join_removed_task(idx, task, "await")?;
                        self.push_checked(idx, v)?;
                        self.frames[idx].ip += 1;
                    }
                    other => {
                        // `await` de nao-tarefa = identidade.
                        self.push_checked(idx, other)?;
                        self.frames[idx].ip += 1;
                    }
                }
            }
            Op::CallSuper(field_idx, desc_idx) => {
                self.call_super(idx, field_idx, desc_idx)?;
            }
            Op::NewMixed(desc_idx) => {
                self.new_mixed(idx, desc_idx)?;
            }
            Op::CallMethod(var_idx, field_idx, desc_idx) => {
                self.call_method(idx, var_idx, field_idx, desc_idx)?;
            }
            Op::LoadNative(name_idx) => {
                self.load_native(idx, name_idx)?;
            }
            Op::Yield => {
                let value = self.pop(idx, "`yield`")?;
                // Entrega ao retomador e salva o frame para continuar depois.
                // `pending_gen` foi armado pelo `gen_resume` no resume.
                let gen = self.frames[idx].pending_gen.take().ok_or_else(|| {
                    VmError::new("`yield` fora de retomada de gerador (interno)".to_string())
                })?;
                let mut frame = self.frames.pop().expect("frame do gerador");
                // Avanca PARA ALEM do `yield`: o resume continua na proxima.
                frame.ip += 1;
                gen.0.borrow_mut().frame = Some(frame);
                let caller = self.frames.len() - 1;
                self.push_checked(caller, value)?;
                self.frames[caller].ip += 1;
            }
            Op::Return => {
                let value = self.pop(idx, "`return`")?;
                if self.frames.len() == 1 {
                    return Err(VmError::new("`return` fora de funcao"));
                }
                // Gerador terminando (RFC 0018): marca `done`, descarta o
                // retorno e entrega `Null` (use `yield` para valores).
                // Tem precedencia: nada mais se aplica a este frame.
                if let Some(gen) = self.frames[idx].pending_gen.take() {
                    gen.0.borrow_mut().done = true;
                    gen.0.borrow_mut().frame = None;
                    self.frames.pop();
                    let caller = self.frames.len() - 1;
                    self.push_checked(caller, Value::Null)?;
                    self.frames[caller].ip += 1;
                    return Ok(Step::Continue);
                }
                // Anotacao de retorno (`: tipo`; RFC 0016).
                if let Some(tag) = self.frames[idx].func.as_ref().and_then(|d| d.return_type) {
                    check_type_or_err(&value, tag)?;
                }
                // `new` (RFC 0017): o `this` final sai desta ativacao
                // (construtor); o retorno e descartado. A flag morre com
                // o frame no unwind — nunca fica obsoleta.
                let closing_new = self.frames[idx].pending_new.take();
                let closing_method = self.frames[idx].pending_method.take();
                let closing_super = self.frames[idx].pending_super;
                self.frames[idx].pending_super = false;
                let final_this =
                    if closing_new.is_some() || closing_method.is_some() || closing_super {
                        self.frames[idx].scopes[0].get("this").cloned()
                    } else {
                        None
                    };
                self.frames.pop();
                let caller = self.frames.len() - 1;
                // Retomada de HOF (RFC 0005): o valor e o retorno do callback;
                // sem HOF pendente, entrega normal ao chamador. `pending_cb`
                // morre junto (a amarra expirou com o callback).
                if let Some(pending) = self.frames[caller].pending.take() {
                    self.frames[caller].pending_cb = None;
                    self.advance_hof(caller, pending, value)?;
                } else if closing_super {
                    // `super.metodo()`: o `this` final volta ao `this` do
                    // chamador (mutacoes persistem no metodo atual).
                    match final_this {
                        Some(Value::Instance { .. }) => {
                            let this_val = final_this.expect("this verificado");
                            if let Some(scope) = self.frames[caller].scopes.first_mut() {
                                scope.insert("this".to_string(), this_val);
                            }
                            self.push_checked(caller, value)?;
                        }
                        _ => {
                            return Err(VmError::new(
                                "metodo do pai deve preservar `this` como instancia".to_string(),
                            ));
                        }
                    }
                } else if let Some(class_name) = closing_new {
                    // `new` (RFC 0017): descarta o retorno do construtor e
                    // entrega o `this` final (deve ser instancia da classe).
                    let ok = matches!(&final_this, Some(Value::Instance { class, .. }) if class.name == class_name);
                    if ok {
                        self.push_checked(caller, final_this.expect("this verificado"))?;
                    } else {
                        return Err(VmError::new(format!(
                            "construtor `novo` de `{class_name}` deve preservar `this` como instancia"
                        )));
                    }
                } else if let Some(var_name) = closing_method {
                    // Metodo sobre variavel (RFC 0017): o retorno e o valor
                    // da chamada; o `this` final volta para a variavel
                    // (mutacoes via `obj.metodo()` persistem; coerente com
                    // `a[i] = v`, que tambem exige variavel nomeada).
                    match final_this {
                        Some(Value::Instance { .. }) => {
                            if !self.assign(caller, &var_name, final_this.expect("this verificado"))
                            {
                                return Err(VmError::new(format!(
                                    "variavel nao definida: {var_name}"
                                )));
                            }
                            self.push_checked(caller, value)?;
                        }
                        _ => {
                            return Err(VmError::new(
                                "metodo deve preservar `this` como instancia".to_string(),
                            ));
                        }
                    }
                } else {
                    self.push_checked(caller, value)?;
                }
            }
            Op::Pop => {
                self.pop(idx, "descarte de valor")?;
                self.frames[idx].ip += 1;
            }
            Op::BuildArray(count) => {
                let count = count as usize;
                if self.frames[idx].stack.len() < count {
                    return Err(VmError::new("pilha insuficiente para array"));
                }
                let mut items = Vec::with_capacity(count);
                for _ in 0..count {
                    if let Some(item) = self.frames[idx].stack.pop() {
                        items.push(item);
                    }
                }
                items.reverse();
                self.push_checked(idx, Value::Array(items))?;
                self.frames[idx].ip += 1;
            }
            Op::BuildMap(count) => {
                let count = count as usize;
                if self.frames[idx].stack.len() < count * 2 {
                    return Err(VmError::new("pilha insuficiente para mapa"));
                }
                let mut entries = HashMap::with_capacity(count);
                for _ in 0..count {
                    let value = self.frames[idx]
                        .stack
                        .pop()
                        .ok_or_else(|| VmError::new("pilha insuficiente para mapa"))?;
                    let key = self.frames[idx]
                        .stack
                        .pop()
                        .ok_or_else(|| VmError::new("pilha insuficiente para mapa"))?;
                    match key {
                        Value::Str(name) => {
                            entries.insert(name, value);
                        }
                        other => {
                            return Err(VmError::new(format!(
                                "chave de mapa deve ser string, encontrado {}",
                                type_name(&other)
                            )));
                        }
                    }
                }
                self.push_checked(idx, Value::Map(entries))?;
                self.frames[idx].ip += 1;
            }
            Op::Index => {
                let index = self.pop(idx, "indice")?;
                let base = self.pop(idx, "indexacao")?;
                let result = index_value(&base, &index)?;
                self.push_checked(idx, result)?;
                self.frames[idx].ip += 1;
            }
            Op::StoreIndex(name_idx) => {
                let value = self.pop(idx, "atribuicao indexada")?;
                let index = self.pop(idx, "indice")?;
                let name = self.name_of(idx, name_idx)?;
                let container = self
                    .lookup(idx, &name)
                    .ok_or_else(|| VmError::new(format!("variavel nao definida: {name}")))?;
                let updated = store_index_value(&container, &index, value)?;
                self.check_collection(&updated)?;
                if !self.assign(idx, &name, updated) {
                    return Err(VmError::new(format!("variavel nao definida: {name}")));
                }
                self.frames[idx].ip += 1;
            }
            Op::TryBegin(target) => {
                let scope_len = self.frames[idx].scopes.len();
                self.frames[idx].handlers.push(Handler {
                    target: target as usize,
                    scope_len,
                });
                self.frames[idx].ip += 1;
            }
            Op::TryEnd => {
                self.frames[idx]
                    .handlers
                    .pop()
                    .ok_or_else(|| VmError::new("desequilibrio interno de handlers".to_string()))?;
                self.frames[idx].ip += 1;
            }
            Op::Throw => {
                let value = self.pop(idx, "`throw`")?;
                let original = VmError::new(format!("excecao nao capturada: {value}"));
                return self.unwind(value, original);
            }
            Op::Rethrow => {
                let value = self.pop(idx, "repropagacao")?;
                let original = VmError::new(Self::uncaught_message(&value));
                return self.unwind(value, original);
            }
            Op::CheckType(tag) => {
                let value = self.frames[idx]
                    .stack
                    .last()
                    .ok_or_else(|| VmError::new("pilha vazia em checagem de tipo".to_string()))?;
                check_type_or_err(value, tag)?;
                self.frames[idx].ip += 1;
            }
            Op::IterList => {
                let value = self.pop(idx, "iteracao")?;
                let list = match value {
                    Value::Array(items) => items,
                    Value::Map(entries) => {
                        let mut keys: Vec<String> = entries.keys().cloned().collect();
                        keys.sort();
                        keys.into_iter().map(Value::Str).collect()
                    }
                    other => {
                        return Err(VmError::new(format!(
                            "`for` exige array, mapa ou gerador, encontrado {}",
                            type_name(&other)
                        )));
                    }
                };
                self.push_checked(idx, Value::Array(list))?;
                self.frames[idx].ip += 1;
            }
            Op::CallBuiltin(id) => {
                let builtin = id
                    .resolve()
                    .map_err(|e| VmError::new(format!("bytecode invalido: {e}")))?;
                let arity = builtin_arity(builtin);
                if self.frames[idx].stack.len() < arity {
                    return Err(VmError::new("pilha insuficiente para builtin"));
                }
                let mut args = Vec::with_capacity(arity);
                for _ in 0..arity {
                    if let Some(arg) = self.frames[idx].stack.pop() {
                        args.push(arg);
                    }
                }
                args.reverse();
                // HOFs com callback (RFC 0005): protocolo de chamada pendente
                // (nunca passam pelo `call_builtin` sincrono).
                if matches!(builtin, Builtin::Map | Builtin::Filter | Builtin::Usort) {
                    self.hof_begin(idx, builtin, args)?;
                } else if matches!(builtin, Builtin::Next) {
                    // `next(gen)` (RFC 0018): retoma o gerador (precisa dos
                    // frames; nunca passa pelo `call_builtin` sincrono).
                    self.gen_resume(args)?;
                } else if matches!(
                    builtin,
                    Builtin::AsyncSpawn
                        | Builtin::AsyncJoin
                        | Builtin::AwaitTimeout
                        | Builtin::AsyncCancel
                        | Builtin::AsyncCancelled
                        | Builtin::AwaitAll
                        | Builtin::AwaitRace
                ) {
                    // Tarefas (RFC 0019): precisam da VM (programa + tabela).
                    self.async_begin(idx, builtin, args)?;
                    self.frames[idx].ip += 1;
                } else {
                    let result = call_builtin(
                        builtin,
                        &args,
                        &self.argv,
                        &mut self.db,
                        &mut self.rng,
                        self.program.clone(),
                        &mut self.tcp,
                        &mut self.redis,
                    )?;
                    self.push_checked(idx, result)?;
                    self.frames[idx].ip += 1;
                }
            }
        }
        self.check_stack_depth(idx)?;
        Ok(Step::Continue)
    }

    fn pop(&mut self, idx: usize, context: &str) -> Result<Value, VmError> {
        self.frames[idx]
            .stack
            .pop()
            .ok_or_else(|| VmError::new(format!("pilha vazia em {context}")))
    }

    fn push_checked(&mut self, idx: usize, value: Value) -> Result<(), VmError> {
        self.check_collection(&value)?;
        self.check_heap_growth(&value)?;
        self.frames[idx].stack.push(value);
        Ok(())
    }

    /// Teto de heap amortizado O(1) (RFC 0015): soma a estimativa de cada
    /// valor empilhado; ao cruzar o gatilho (`max/16`, minimo 64 bytes),
    /// varre a heap viva e reancora o contador no valor real — `pop`s
    /// liberam orcamento, entao aloca-e-libera cabe em teto pequeno.
    fn check_heap_growth(&mut self, value: &Value) -> Result<(), VmError> {
        let Some(max) = self.limits.max_heap_bytes else {
            return Ok(());
        };
        self.heap_since_check = self.heap_since_check.saturating_add(heap_size_of(value, 0));
        if self.heap_since_check >= (max / 16).max(64) {
            let live = self.heap_bytes();
            self.heap_since_check = live;
            if live > max {
                return Err(VmError::new(format!("heap excedeu o limite ({max} bytes)")));
            }
        }
        Ok(())
    }

    /// Varredura completa da heap viva: pilhas + escopos de cada frame,
    /// globais e statics.
    fn heap_bytes(&self) -> u64 {
        let mut total = 0u64;
        for frame in &self.frames {
            for value in &frame.stack {
                total = total.saturating_add(heap_size_of(value, 0));
            }
            for scope in &frame.scopes {
                total = total.saturating_add(heap_size_of_entries(scope, 0));
            }
        }
        total = total.saturating_add(heap_size_of_entries(&self.globals, 0));
        for (_, vars) in &self.statics {
            total = total.saturating_add(heap_size_of_entries(vars, 0));
        }
        total
    }

    fn check_collection(&self, value: &Value) -> Result<(), VmError> {
        if let Some(max) = self.limits.max_collection_len {
            if collection_len(value) > max {
                return Err(VmError::new(format!(
                    "colecao excede o limite ({max} elementos/bytes)"
                )));
            }
        }
        Ok(())
    }

    fn check_stack_depth(&self, idx: usize) -> Result<(), VmError> {
        if let Some(max) = self.limits.max_stack_depth {
            // O frame pode ter sido removido por `Return` (idx obsoleto):
            // sem frame vivo, nao ha pilha a medir. O topo corrente e
            // medido no passo seguinte (o `run` recalcula o idx a cada
            // iteracao), entao a fiscalizacao nao enfraquece.
            if let Some(frame) = self.frames.get(idx) {
                if frame.stack.len() > max {
                    return Err(VmError::new(format!(
                        "pilha de operandos excedeu o limite ({max})"
                    )));
                }
            }
        }
        Ok(())
    }

    fn binary(
        &mut self,
        idx: usize,
        context: &str,
        apply: fn(&Value, &Value) -> Result<Value, VmError>,
    ) -> Result<(), VmError> {
        let right = self.pop(idx, context)?;
        let left = self.pop(idx, context)?;
        let result = apply(&left, &right)?;
        self.push_checked(idx, result)?;
        self.frames[idx].ip += 1;
        Ok(())
    }

    fn name_of(&self, idx: usize, name_idx: u32) -> Result<String, VmError> {
        self.frames[idx]
            .chunk
            .names
            .get(name_idx as usize)
            .cloned()
            .ok_or_else(|| VmError::new(format!("nome {name_idx} inexistente")))
    }

    fn lookup(&self, idx: usize, name: &str) -> Option<Value> {
        for scope in self.frames[idx].scopes.iter().rev() {
            if let Some(value) = scope.get(name) {
                return Some(value.clone());
            }
        }
        // `static` da funcao atual (RFC 0010): abaixo dos escopos lexicos.
        if let Some(func) = self.frames[idx].func.clone() {
            if let Some(value) = self
                .statics
                .iter()
                .find(|(def, _)| Rc::ptr_eq(def, &func))
                .and_then(|(_, vars)| vars.get(name).cloned())
            {
                return Some(value);
            }
        }
        self.globals.get(name).cloned()
    }

    /// Mapa de estaticas da definicao (cria vazio se inedito; RFC 0010).
    fn static_vars(&mut self, func: &Rc<FuncDef>) -> &mut HashMap<String, Value> {
        if let Some(pos) = self
            .statics
            .iter()
            .position(|(def, _)| Rc::ptr_eq(def, func))
        {
            return &mut self.statics[pos].1;
        }
        self.statics.push((func.clone(), HashMap::new()));
        &mut self
            .statics
            .last_mut()
            .expect("entrada recem-criada existe")
            .1
    }

    /// Atribui a variavel existente; retorna `false` se nao encontrada.
    fn assign(&mut self, idx: usize, name: &str, value: Value) -> bool {
        for scope in self.frames[idx].scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                scope.insert(name.to_string(), value);
                return true;
            }
        }
        // `static` da funcao atual (RFC 0010): abaixo dos escopos lexicos.
        if let Some(func) = self.frames[idx].func.clone() {
            if let Some(pos) = self
                .statics
                .iter()
                .position(|(def, _)| Rc::ptr_eq(def, &func))
            {
                let vars = &mut self.statics[pos].1;
                if vars.contains_key(name) {
                    vars.insert(name.to_string(), value);
                    return true;
                }
            }
        }
        if self.globals.contains_key(name) {
            self.globals.insert(name.to_string(), value);
            return true;
        }
        false
    }

    /// Fotografa o ambiente atual por valor (RFC 0003): funde os escopos
    /// do mais externo ao mais interno, de modo que o interno sombreie.
    fn capture_env(&self, idx: usize) -> std::collections::HashMap<String, Value> {
        let mut env = std::collections::HashMap::new();
        for scope in self.frames[idx].scopes.iter() {
            for (k, v) in scope {
                env.insert(k.clone(), v.clone());
            }
        }
        env
    }

    /// Declara no escopo atual (ou globais, no topo do programa principal).
    fn declare(&mut self, idx: usize, name: &str, value: Value) -> Result<(), VmError> {
        if self.frames.len() == 1 && self.frames[idx].scopes.len() == 1 {
            if self.globals.contains_key(name) {
                return Err(VmError::new(format!("variavel ja declarada: {name}")));
            }
            self.globals.insert(name.to_string(), value);
            return Ok(());
        }
        let frame = &mut self.frames[idx];
        let scope = frame
            .scopes
            .last_mut()
            .ok_or_else(|| VmError::new("sem escopo ativo para declaracao"))?;
        if scope.contains_key(name) {
            return Err(VmError::new(format!("variavel ja declarada: {name}")));
        }
        scope.insert(name.to_string(), value);
        Ok(())
    }

    fn call(&mut self, idx: usize, arity: u32) -> Result<(), VmError> {
        let arity = arity as usize;
        if self.frames[idx].stack.len() < arity + 1 {
            return Err(VmError::new("pilha insuficiente para chamada"));
        }
        let mut args = Vec::with_capacity(arity);
        for _ in 0..arity {
            if let Some(arg) = self.frames[idx].stack.pop() {
                args.push(arg);
            }
        }
        args.reverse();
        let callee = self.frames[idx]
            .stack
            .pop()
            .ok_or_else(|| VmError::new("pilha insuficiente para chamada"))?;
        let func = match callee {
            Value::Func(func) => func,
            other => {
                return Err(VmError::new(format!(
                    "tentativa de chamar {} como funcao",
                    type_name(&other)
                )));
            }
        };
        // Nativa (Fase Next-Gen 6): executa Rust sem frame.
        if let Some(native_id) = func.def.native_id.clone() {
            let (ext, fname) = native_id
                .split_once("::")
                .ok_or_else(|| VmError::new(format!("id nativo invalido: `{native_id}`")))?;
            let result = extensions::call(ext, fname, args).map_err(VmError::new)?;
            self.push_checked(idx, result)?;
            self.frames[idx].ip += 1;
            return Ok(());
        }
        // `async fn` (Fase Next-Gen 3): gera tarefa em vez de bloquear.
        if func.def.is_async {
            self.spawn_async_call(idx, func, args)?;
            self.frames[idx].ip += 1;
            return Ok(());
        }
        self.push_call_frame(func, args)?;
        self.frames[idx].ip += 1;
        Ok(())
    }

    /// Empilha a ativacao de `func` com `args` (checa aridade e profundidade;
    /// escopo unico = clone do env + params). NAO avanca o `ip` do chamador:
    /// quem chama avanca exatamente uma vez (no HOF, so na primeira chamada;
    /// as seguintes retomam do `Return` com o `ip` ja posicionado).
    /// Usado por `Call` e pelo protocolo de HOFs (RFC 0005).
    fn push_call_frame(&mut self, func: FuncValue, args: Vec<Value>) -> Result<(), VmError> {
        // Aridade flexivel (RFC 0006): `required` = iniciais sem padrao
        // (parse garante: opcionais so no fim); excedentes vao para `...rest`.
        let total = func.def.params.len();
        let required =
            func.def.params.len() - func.def.defaults.iter().filter(|d| d.is_some()).count();
        if args.len() < required {
            return Err(VmError::new(format!(
                "funcao `{}` espera ao menos {required} argumentos, recebeu {}",
                func.def.name,
                args.len()
            )));
        }
        if args.len() > total && func.def.variadic.is_none() {
            return Err(VmError::new(format!(
                "funcao `{}` espera {} argumentos, recebeu {}",
                func.def.name,
                total,
                args.len()
            )));
        }
        // Escopo unico da ativacao: ambiente capturado + parametros por
        // cima (parametro sombreia variavel capturada com o mesmo nome).
        // Posicoes ausentes usam o padrao compilado (`null` explicito e valor,
        // nao aciona padrao); excedentes vao para `...rest` (`[]` se nenhum).
        let mut bindings = Vec::with_capacity(total + 1);
        for (i, name) in func.def.params.iter().enumerate() {
            if i < args.len() {
                bindings.push((name.clone(), args[i].clone()));
            } else if let Some(default) = &func.def.defaults[i] {
                bindings.push((name.clone(), default.clone()));
            } else {
                return Err(VmError::new(format!(
                    "funcao `{}` sem valor para `{name}` (bug interno: padrao ausente)",
                    func.def.name
                )));
            }
        }
        if let Some(rest) = &func.def.variadic {
            let extra = if args.len() > total {
                args[total..].to_vec()
            } else {
                Vec::new()
            };
            bindings.push((rest.clone(), Value::Array(extra)));
        }
        self.activate(func, bindings)
    }

    /// Empilha a ativacao com vinculos ja resolvidos (ordem irrelevante:
    /// vao direto para o escopo unico). Checa profundidade e preserva a
    /// guarda de `ExitScope` (base com 1 escopo). Nao avanca o `ip`.
    /// Monta o frame de ativacao (sem empilhar, sem checar profundidade).
    fn make_frame(func: &FuncValue, scope: HashMap<String, Value>) -> Frame {
        Frame {
            chunk: func.def.body.clone(),
            ip: 0,
            stack: Vec::new(),
            scopes: vec![scope],
            handlers: Vec::new(),
            pending_new: None,
            pending_method: None,
            pending_super: false,
            pending_gen: None,
            pending: None,
            pending_cb: None,
            func: Some(func.def.clone()),
        }
    }

    /// Empilha a ativacao com vinculos ja resolvidos (ordem irrelevante:
    /// vao direto para o escopo unico). Checa profundidade e preserva a
    /// guarda de `ExitScope` (base com 1 escopo). Nao avanca o `ip`.
    /// Funcao geradora NAO executa: empilha `Value::Generator` lazy
    /// (RFC 0018) no chamador.
    fn activate(&mut self, func: FuncValue, bindings: Vec<(String, Value)>) -> Result<(), VmError> {
        if self.frames.len() >= MAX_CALL_DEPTH {
            return Err(VmError::new(
                "limite de chamadas excedido (recursao muito profunda?)",
            ));
        }
        // Escopo unico da ativacao: ambiente capturado + vinculos por cima.
        // Guarda de `ExitScope` preservada: base continua com 1 escopo.
        let mut scope = func.env.clone();
        for (name, value) in bindings {
            scope.insert(name, value);
        }
        // Anotacoes de parametros (`a: int`; RFC 0016): checa os valores
        // vinculados (defaults ja preenchidos pelo chamador).
        for (i, name) in func.def.params.iter().enumerate() {
            if let Some(tag) = func.def.param_types.get(i).copied().flatten() {
                if let Some(value) = scope.get(name) {
                    check_type_or_err(value, tag)?;
                }
            }
        }
        if func.def.is_generator {
            let frame = Self::make_frame(&func, scope);
            let gen = Value::Generator(GenHandle(Rc::new(RefCell::new(GenState {
                func,
                frame: Some(frame),
                done: false,
            }))));
            let top = self.frames.len() - 1;
            self.push_checked(top, gen)?;
            return Ok(());
        }
        self.frames.push(Self::make_frame(&func, scope));
        Ok(())
    }

    /// Liga chamada mista (RFC 0009): posicionais em ordem, nomeados por
    /// nome, spreads ja expandidos em `pos`. Regras: nome desconhecido e
    /// named repetindo posicao = erro; ausentes usam `defaults`; excedentes
    /// vao para `...rest` (sem ele = erro).
    fn push_call_frame_mixed(
        &mut self,
        func: FuncValue,
        pos: Vec<Value>,
        named: Vec<(String, Value)>,
    ) -> Result<(), VmError> {
        let total = func.def.params.len();
        let required =
            func.def.params.len() - func.def.defaults.iter().filter(|d| d.is_some()).count();
        let received = pos.len() + named.len();
        if received < required {
            return Err(VmError::new(format!(
                "funcao `{}` espera ao menos {required} argumentos, recebeu {received}",
                func.def.name,
            )));
        }
        if pos.len() > total && func.def.variadic.is_none() {
            return Err(VmError::new(format!(
                "funcao `{}` espera {total} argumentos, recebeu {received}",
                func.def.name,
            )));
        }
        let mut bound: Vec<Option<Value>> = vec![None; total];
        for (i, value) in pos.iter().enumerate() {
            if i >= total {
                break;
            }
            bound[i] = Some(value.clone());
        }
        for (name, value) in &named {
            match func.def.params.iter().position(|p| p == name) {
                Some(i) if bound[i].is_some() => {
                    return Err(VmError::new(format!(
                        "parametro `{name}` ja preenchido por posicao"
                    )));
                }
                Some(i) => {
                    bound[i] = Some(value.clone());
                }
                None => {
                    return Err(VmError::new(format!("argumento desconhecido: `{name}`")));
                }
            }
        }
        let mut bindings = Vec::with_capacity(total + 1);
        for (i, name) in func.def.params.iter().enumerate() {
            match bound[i].take() {
                Some(value) => bindings.push((name.clone(), value)),
                None => match &func.def.defaults[i] {
                    Some(default) => bindings.push((name.clone(), default.clone())),
                    None => {
                        return Err(VmError::new(format!(
                            "funcao `{}` espera ao menos {required} argumentos, recebeu {received}",
                            func.def.name,
                        )));
                    }
                },
            }
        }
        if let Some(rest) = &func.def.variadic {
            let extra = if pos.len() > total {
                pos[total..].to_vec()
            } else {
                Vec::new()
            };
            bindings.push((rest.clone(), Value::Array(extra)));
        }
        self.activate(func, bindings)
    }

    /// Desempilha os valores de uma chamada com descritor e os separa em
    /// posicionais/nomeados (expandindo spreads). Compartilhado por
    /// `CallMixed`, `NewMixed` e `CallMethod` (RFCs 0009 e 0017).
    fn mixed_args(&mut self, idx: usize, desc_idx: u32, what: &str) -> Result<MixedArgs, VmError> {
        let desc = match self.frames[idx].chunk.constants.get(desc_idx as usize) {
            Some(Value::Array(items)) => items.clone(),
            _ => {
                return Err(VmError::new(format!(
                    "bytecode invalido: descritor de chamada mista {desc_idx} invalido"
                )));
            }
        };
        let n = desc.len();
        if self.frames[idx].stack.len() < n {
            return Err(VmError::new(format!("pilha insuficiente para {what}")));
        }
        let mut values = Vec::with_capacity(n);
        for _ in 0..n {
            if let Some(value) = self.frames[idx].stack.pop() {
                values.push(value);
            }
        }
        values.reverse();
        let mut pos = Vec::new();
        let mut named = Vec::new();
        for (marker, value) in desc.iter().zip(values) {
            match marker {
                Value::Int(0) => pos.push(value),
                Value::Int(1) => match value {
                    Value::Array(items) => pos.extend(items),
                    other => {
                        return Err(VmError::new(format!(
                            "spread exige array, encontrado {}",
                            type_name(&other)
                        )));
                    }
                },
                Value::Str(name) => named.push((name.clone(), value)),
                other => {
                    return Err(VmError::new(format!(
                        "bytecode invalido: marcador de chamada mista invalido ({})",
                        type_name(other)
                    )));
                }
            }
        }
        Ok((pos, named))
    }

    /// Executa `Op::CallMixed` (RFC 0009): le o descritor, desempilha callee
    /// + valores em ordem do fonte, expande spreads e liga por nome/posicao.
    fn call_mixed(&mut self, idx: usize, desc_idx: u32) -> Result<(), VmError> {
        let (pos, named) = self.mixed_args(idx, desc_idx, "chamada")?;
        let callee = self.frames[idx]
            .stack
            .pop()
            .ok_or_else(|| VmError::new("pilha insuficiente para chamada"))?;
        let func = match callee {
            Value::Func(func) => func,
            other => {
                return Err(VmError::new(format!(
                    "tentativa de chamar {} como funcao",
                    type_name(&other)
                )));
            }
        };
        if let Some(native_id) = func.def.native_id.clone() {
            if !named.is_empty() {
                return Err(VmError::new(
                    "funcao nativa nao aceita argumentos nomeados/spread".to_string(),
                ));
            }
            let (ext, fname) = native_id
                .split_once("::")
                .ok_or_else(|| VmError::new(format!("id nativo invalido: `{native_id}`")))?;
            let result = extensions::call(ext, fname, pos).map_err(VmError::new)?;
            self.push_checked(idx, result)?;
            self.frames[idx].ip += 1;
            return Ok(());
        }
        if func.def.is_async {
            // Reconstrucao posicional simples: `call_mixed` com nomeados/spread
            // em `async fn` nao e suportado (use posicional ou `async_spawn`).
            if !named.is_empty() {
                return Err(VmError::new(
                    "`async fn` nao aceita argumentos nomeados/spread (use posicionais)"
                        .to_string(),
                ));
            }
            // `pos` ja esta na ordem ligada? `mixed_args` devolve (pos, named);
            // para so-posicional, `pos` e a lista completa.
            self.spawn_async_call(idx, func, pos)?;
            self.frames[idx].ip += 1;
            return Ok(());
        }
        self.push_call_frame_mixed(func, pos, named)?;
        self.frames[idx].ip += 1;
        Ok(())
    }

    /// Executa `Op::NewMixed` (RFC 0017): como `call_mixed`, mas o callee
    /// deve ser `Value::Class` e o alvo e sempre o construtor `novo`.
    /// Ativa com `this` = instancia vazia e arma `pending_new` no frame
    /// do construtor (o `Return` entrega o `this` final).
    fn new_mixed(&mut self, idx: usize, desc_idx: u32) -> Result<(), VmError> {
        let (pos, named) = self.mixed_args(idx, desc_idx, "`new`")?;
        let callee = self.frames[idx]
            .stack
            .pop()
            .ok_or_else(|| VmError::new("pilha insuficiente para `new`"))?;
        let class = match callee {
            Value::Class(class) => class,
            other => {
                return Err(VmError::new(format!(
                    "`new` exige classe, encontrou {}",
                    type_name(&other)
                )));
            }
        };
        let novo = class.methods.get("novo").cloned().ok_or_else(|| {
            VmError::new(format!(
                "classe `{}` sem construtor `novo` (defina `fn novo(...)`)",
                class.name
            ))
        })?;
        if novo.def.is_generator {
            return Err(VmError::new(format!(
                "construtor `novo` de `{}` nao pode ser gerador",
                class.name
            )));
        }
        let instance = Value::Instance {
            class: class.clone(),
            fields: HashMap::new(),
        };
        let mut func = novo;
        // `this` disponivel no corpo do construtor (e em closures dele).
        func.env.insert("this".to_string(), instance);
        self.push_call_frame_mixed(func, pos, named)?;
        self.frames[idx].ip += 1;
        if let Some(frame) = self.frames.last_mut() {
            frame.pending_new = Some(class.name.clone());
        }
        Ok(())
    }

    /// Executa `Op::CallMethod` (RFC 0017): `variavel.metodo(args)`.
    /// Como `Index` + chamada, mas com write-back: apos o retorno, o
    /// `this` final e gravado de volta na variavel (mutacoes de metodo
    /// sobre variaveis nomeadas persistem; coerente com `a[i] = v`, que
    /// tambem exige variavel). Sobre expressoes temporarias, use
    /// `obj.metodo()` via `Index` (sem write-back).
    fn call_method(
        &mut self,
        idx: usize,
        var_idx: u32,
        field_idx: u32,
        desc_idx: u32,
    ) -> Result<(), VmError> {
        let var_name = self.name_of(idx, var_idx)?;
        let field = match self.frames[idx].chunk.constants.get(field_idx as usize) {
            Some(Value::Str(field)) => field.clone(),
            _ => {
                return Err(VmError::new(format!(
                    "bytecode invalido: campo de metodo {field_idx} invalido"
                )));
            }
        };
        let (pos, named) = self.mixed_args(idx, desc_idx, "chamada de metodo")?;
        let base = self
            .lookup(idx, &var_name)
            .ok_or_else(|| VmError::new(format!("variavel nao definida: {var_name}")))?;
        match base {
            Value::Instance { class, fields } => {
                // Campos primeiro (como no `Index`): funcao em campo
                // chama sem `this` e sem write-back.
                if let Some(value) = fields.get(&field) {
                    let func = match value.clone() {
                        Value::Func(func) => func,
                        other => {
                            return Err(VmError::new(format!(
                                "tentativa de chamar {} como funcao",
                                type_name(&other)
                            )));
                        }
                    };
                    // Campo com funcao async: gera tarefa (sem write-back).
                    if func.def.is_async {
                        if !named.is_empty() {
                            return Err(VmError::new(
                                "`async` nao aceita argumentos nomeados nem spread (use posicionais)"
                                    .to_string(),
                            ));
                        }
                        self.spawn_async_call(idx, func, pos)?;
                        self.frames[idx].ip += 1;
                        return Ok(());
                    }
                    self.push_call_frame_mixed(func, pos, named)?;
                    self.frames[idx].ip += 1;
                    return Ok(());
                }
                let func = class.methods.get(&field).cloned().ok_or_else(|| {
                    VmError::new(format!(
                        "membro inexistente na instancia de `{}`: `{field}` (sem campo nem metodo)",
                        class.name,
                    ))
                })?;
                let mut func = func;
                func.env.insert(
                    "this".to_string(),
                    Value::Instance {
                        class: class.clone(),
                        fields,
                    },
                );
                // Metodo async: gera tarefa sobre snapshot (sem write-back:
                // mutacoes nao voltam; devolva valores com `return`).
                if func.def.is_async {
                    if !named.is_empty() {
                        return Err(VmError::new(
                            "metodo `async` nao aceita argumentos nomeados nem spread (use posicionais)"
                                .to_string(),
                        ));
                    }
                    self.spawn_async_call(idx, func, pos)?;
                    self.frames[idx].ip += 1;
                    return Ok(());
                }
                // Metodo gerador: lazy, sem write-back (nunca executa aqui).
                let is_gen = func.def.is_generator;
                self.push_call_frame_mixed(func, pos, named)?;
                self.frames[idx].ip += 1;
                if !is_gen {
                    if let Some(frame) = self.frames.last_mut() {
                        frame.pending_method = Some(var_name);
                    }
                }
                Ok(())
            }
            other => {
                // Nao-instancia: equivale a `Index` + chamada comum (sem
                // write-back; ex.: mapa com campo funcao).
                let member = index_value(&other, &Value::Str(field))?;
                let func = match member {
                    Value::Func(func) => func,
                    other => {
                        return Err(VmError::new(format!(
                            "tentativa de chamar {} como funcao",
                            type_name(&other)
                        )));
                    }
                };
                self.push_call_frame_mixed(func, pos, named)?;
                self.frames[idx].ip += 1;
                Ok(())
            }
        }
    }

    /// `super.metodo(args)` (Fase Next-Gen 1): metodo da classe pai com o
    /// `this` atual; write-back no `this` do chamador via `pending_super`.
    fn call_super(&mut self, idx: usize, field_idx: u32, desc_idx: u32) -> Result<(), VmError> {
        let field = match self.frames[idx].chunk.constants.get(field_idx as usize) {
            Some(Value::Str(field)) => field.clone(),
            _ => {
                return Err(VmError::new(format!(
                    "bytecode invalido: campo de super {field_idx} invalido"
                )));
            }
        };
        let (pos, named) = self.mixed_args(idx, desc_idx, "chamada de `super`")?;
        let this_val = self.frames[idx].scopes[0]
            .get("this")
            .cloned()
            .ok_or_else(|| VmError::new("`super` fora de metodo (sem `this`)".to_string()))?;
        let (class, fields) = match this_val {
            Value::Instance { class, fields } => (class, fields),
            other => {
                return Err(VmError::new(format!(
                    "`super` exige `this` instancia, encontrou {}",
                    type_name(&other)
                )));
            }
        };
        let parent_name = class.parent.clone().ok_or_else(|| {
            VmError::new(format!(
                "classe `{}` sem pai (`super` indisponivel)",
                class.name
            ))
        })?;
        let parent_class = self
            .lookup_class(idx, &parent_name)
            .ok_or_else(|| VmError::new(format!("classe pai `{parent_name}` nao encontrada")))?;
        let func = parent_class.methods.get(&field).cloned().ok_or_else(|| {
            VmError::new(format!(
                "metodo inexistente no pai `{parent_name}`: `{field}`"
            ))
        })?;
        let mut func = func;
        func.env.insert(
            "this".to_string(),
            Value::Instance {
                class: class.clone(),
                fields,
            },
        );
        if func.def.is_generator {
            return Err(VmError::new(
                "metodo gerador do pai nao suportado via `super`".to_string(),
            ));
        }
        if func.def.is_async {
            if !named.is_empty() {
                return Err(VmError::new(
                    "metodo `async` do pai nao aceita argumentos nomeados nem spread (use posicionais)"
                        .to_string(),
                ));
            }
            // Snapshot como em `call_method` (sem write-back no `this` local).
            self.spawn_async_call(idx, func, pos)?;
            self.frames[idx].ip += 1;
            return Ok(());
        }
        self.push_call_frame_mixed(func, pos, named)?;
        self.frames[idx].ip += 1;
        if let Some(frame) = self.frames.last_mut() {
            frame.pending_super = true;
        }
        Ok(())
    }

    /// Busca classe por nome (escopos + globais).
    fn lookup_class(&self, idx: usize, name: &str) -> Option<Rc<ClassDef>> {
        match self.lookup(idx, name) {
            Some(Value::Class(class)) => Some(class),
            _ => None,
        }
    }

    /// `import native "ext"` (Fase Next-Gen 6): valida ABI e registra as
    /// funcoes como globais RHP (sem frame; idempotente = erro na segunda).
    fn load_native(&mut self, idx: usize, name_idx: u32) -> Result<(), VmError> {
        let ext_name = self.name_of(idx, name_idx)?;
        if self.loaded_extensions.contains(&ext_name) {
            return Err(VmError::new(format!("extensao `{ext_name}` ja carregada")));
        }
        let reg = extensions::registry();
        let ext = reg.get(&ext_name).ok_or_else(|| {
            VmError::new(format!(
                "extensao nativa desconhecida: `{ext_name}` (disponiveis: {})",
                available_extensions()
            ))
        })?;
        if ext.abi != extensions::RHP_EXTENSION_ABI {
            return Err(VmError::new(format!(
                "extensao `{ext_name}` com ABI {} incompativel (esperava {})",
                ext.abi,
                extensions::RHP_EXTENSION_ABI
            )));
        }
        for native in &ext.funcs {
            if self.globals.contains_key(&native.name) {
                return Err(VmError::new(format!(
                    "extensao `{ext_name}`: simbolo global `{}` ja existe",
                    native.name
                )));
            }
        }
        for native in &ext.funcs {
            // FuncDef nativa: params sinteticos `a0..`, sem defaults, sem corpo.
            let params: Vec<String> = (0..native.arity).map(|i| format!("a{i}")).collect();
            let body = Rc::new(rhp_bytecode::Chunk {
                constants: vec![],
                names: vec![],
                funcs: vec![],
                code: vec![rhp_bytecode::Op::Halt],
            });
            let def = Rc::new(rhp_bytecode::FuncDef {
                name: native.name.clone(),
                params,
                defaults: vec![None; native.arity],
                variadic: None,
                param_types: vec![None; native.arity],
                return_type: None,
                is_generator: false,
                is_async: false,
                native_id: Some(format!("{}::{}", ext_name, native.name)),
                body,
            });
            self.globals.insert(
                native.name.clone(),
                Value::Func(rhp_bytecode::FuncValue {
                    def,
                    env: HashMap::new(),
                }),
            );
        }
        self.loaded_extensions.insert(ext_name);
        self.frames[idx].ip += 1;
        Ok(())
    }

    /// `true` se `class` e `target` ou subclasse (cadeia `parent`).
    fn is_subclass_of(&self, idx: usize, class: &Rc<ClassDef>, target: &str) -> bool {
        let mut current: Option<Rc<ClassDef>> = Some(class.clone());
        // Limite anti-ciclo (hierarquia valida nao tem ciclo, mas bytecode
        // artesanal poderia forjar `parent` inconsistente).
        for _ in 0..256 {
            match current {
                Some(c) => {
                    if c.name == target {
                        return true;
                    }
                    match c.parent.as_ref() {
                        Some(pname) => {
                            current = self.lookup_class(idx, pname);
                            // Pai ausente = fim da cadeia (falso, sem erro:
                            // captura tipada so casa o que existe).
                            if current.is_none() {
                                // Se o nome do pai sumiu, ainda compara o nome?
                                // Nao: sem definicao, sem hierarquia.
                                return false;
                            }
                        }
                        None => return false,
                    }
                }
                None => return false,
            }
        }
        false
    }

    /// Gera tarefa para `async fn` ou metodo `async` (item metodos-async):
    /// valida, codifica args e captura (instancias com classe) em JSON e
    /// dispara thread com programa recompilado. Devolve handle.
    /// Semantica de snapshot: metodos async rodam sobre UMA COPIA (`this`
    /// reconstruido no worker); mutacoes NAO voltam ao chamador (sem
    /// write-back). Devolva valores com `return`.
    fn spawn_async_call(
        &mut self,
        idx: usize,
        func: FuncValue,
        args: Vec<Value>,
    ) -> Result<(), VmError> {
        let name = func.def.name.clone();
        if func.def.is_generator {
            return Err(VmError::new(format!(
                "`async` nao suporta gerador (`{name}`)"
            )));
        }
        let program = self
            .program
            .clone()
            .ok_or_else(|| VmError::new("`async` sem programa-fonte (interno)".to_string()))?;
        // Fronteira `Send`: args sao posicionais (nao-plano = erro claro,
        // sem reordenar). A captura e por nome: transporta valores planos e
        // instancias (com classe); o resto fica indisponivel (se o corpo
        // usar, falha la com `variavel nao definida`, como global ausente).
        for arg in &args {
            check_shareable(arg).map_err(VmError::new)?;
        }
        // Serializa args e captura como JSON (fronteira `Send`).
        let encoded_args: Vec<Value> = args.iter().map(encode_shareable).collect();
        let args_json = crate::json::stringify_json(&Value::Array(encoded_args))
            .map_err(|e| VmError::new(format!("`async` nao suporta argumento: {}", e.message)))?;
        let shared_env: HashMap<String, Value> = func
            .env
            .iter()
            .filter(|(_, v)| check_shareable(v).is_ok())
            .map(|(k, v)| (k.clone(), encode_shareable(v)))
            .collect();
        let env_json = crate::json::stringify_json(&Value::Map(shared_env))
            .map_err(|e| VmError::new(format!("`async fn` nao suporta captura: {}", e.message)))?;
        let argv = self.argv.clone();
        let thread_name = name.clone();
        let (rx, cancel) = spawn_task_thread(program, argv, thread_name, args_json, env_json);
        let id = self.tasks.next;
        self.tasks.next += 1;
        self.tasks
            .running
            .insert(id, TaskThread { name, rx, cancel });
        self.push_checked(idx, Value::Int(id))?;
        Ok(())
    }

    /// Junta a tarefa ja removida do mapa (item RFC 0030): bloqueia ate o
    /// resultado, propaga erro/panico da tarefa e decodifica o retorno.
    /// Mensagens identicas as dos caminhos legados (`async_join`/`await`).
    fn join_removed_task(
        &self,
        idx: usize,
        task: TaskThread,
        what: &str,
    ) -> Result<Value, VmError> {
        let result = task.rx.recv().map_err(|_| {
            VmError::new(format!("`{what}`: tarefa `{}` abortou (panic)", task.name))
        })?;
        match result {
            Ok(json) => self.decode_task_value(idx, &json, what),
            Err(message) => Err(VmError::new(format!(
                "tarefa `{}` falhou: {message}",
                task.name
            ))),
        }
    }

    /// Decodifica o retorno JSON de tarefa (item metodos-async): instancias
    /// voltam com a classe do programa local (a codificacao preservou nome +
    /// campos). Erro claro se a classe sumiu.
    fn decode_task_value(&self, idx: usize, json: &str, what: &str) -> Result<Value, VmError> {
        let value = crate::json::parse_json(json)
            .map_err(|e| VmError::new(format!("`{what}`: retorno invalido: {e}")))?;
        let lookup = |name: &str| match self.lookup(idx, name) {
            Some(Value::Class(class)) => Some(class),
            _ => None,
        };
        decode_shareable(value, &lookup)
            .map_err(|e| VmError::new(format!("`{what}`: retorno invalido: {e}")))
    }

    /// Retoma um gerador (`next`; RFC 0018, Etapa 5): empilha o frame
    /// suspenso e arma `pending_gen` nele. O chamador NAO avanca: quem
    /// avancar e o `Yield` (valor) ou o `Return` (fim) do gerador.
    /// Gerador esgotado = erro (teste com `exhausted` antes de `next`).
    fn gen_resume(&mut self, args: Vec<Value>) -> Result<(), VmError> {
        let gen = match args.as_slice() {
            [Value::Generator(gen)] => gen.clone(),
            [other] => {
                return Err(VmError::new(format!(
                    "`next` exige gerador, encontrou {}",
                    type_name(other)
                )));
            }
            _ => return Err(VmError::new("`next` exige 1 argumento".to_string())),
        };
        let frame = {
            let mut state = gen.0.borrow_mut();
            if state.done {
                return Err(VmError::new(
                    "gerador esgotado (teste com `exhausted` antes de `next`)".to_string(),
                ));
            }
            match state.frame.take() {
                Some(mut frame) => {
                    frame.pending_gen = Some(gen.clone());
                    frame
                }
                None => {
                    // Sem frame e sem `done`: retomada reentrante (o frame
                    // esta na pilha) ou frame destruido por unwind de erro.
                    // Nos dois casos nao ha como continuar: marca esgotado.
                    state.done = true;
                    return Err(VmError::new(
                        "gerador interrompido (retomada reentrante ou erro anterior; crie outro)"
                            .to_string(),
                    ));
                }
            }
        };
        self.frames.push(frame);
        Ok(())
    }

    /// Inicia (`async_spawn("f")`) ou aguarda (`async_join(h)`) tarefa
    /// (RFC 0019, Etapa 5): thread propria com programa recompilado
    /// (padrao `http_serve`: `Arc<Program>` e owned+`Send`); resultado
    /// cruza a fronteira como JSON (fronteira `String`, `Send`).
    fn async_begin(
        &mut self,
        idx: usize,
        builtin: Builtin,
        args: Vec<Value>,
    ) -> Result<(), VmError> {
        match builtin {
            Builtin::AsyncSpawn => {
                let name = match args.as_slice() {
                    [Value::Str(name)] => name.clone(),
                    [other] => {
                        return Err(VmError::new(format!(
                            "`async_spawn` exige funcao (nome em string), encontrou {}",
                            type_name(other)
                        )));
                    }
                    _ => return Err(VmError::new("`async_spawn` exige 1 argumento".to_string())),
                };
                // Valida ja: `fn` de nivel superior existente, nao-geradora,
                // sem captura (o worker recompila e revalida pelo nome).
                let func = match self.lookup(idx, &name) {
                    Some(Value::Func(func)) => func,
                    Some(other) => {
                        return Err(VmError::new(format!(
                            "`async_spawn` exige funcao, `{name}` e {}",
                            type_name(&other)
                        )));
                    }
                    None => {
                        return Err(VmError::new(format!(
                            "`async_spawn`: funcao `{name}` inexistente (so `fn` de nivel superior)"
                        )));
                    }
                };
                if func.def.is_generator {
                    return Err(VmError::new(format!(
                        "`async_spawn` nao suporta gerador (`{name}`; use `next`/`for`)"
                    )));
                }
                if !func.env.is_empty() {
                    return Err(VmError::new(format!(
                        "`async_spawn` exige funcao sem captura (`{name}` captura ambiente)"
                    )));
                }
                let program = self.program.clone().ok_or_else(|| {
                    VmError::new("`async_spawn` sem programa-fonte (interno)".to_string())
                })?;
                let argv = self.argv.clone();
                let thread_name = name.clone();
                let (rx, cancel) = spawn_task_thread(
                    program,
                    argv,
                    thread_name,
                    "[]".to_string(),
                    "{}".to_string(),
                );
                let id = self.tasks.next;
                self.tasks.next += 1;
                self.tasks
                    .running
                    .insert(id, TaskThread { name, rx, cancel });
                self.push_checked(idx, Value::Int(id))?;
                Ok(())
            }
            Builtin::AsyncJoin => {
                let id = match args.as_slice() {
                    [Value::Int(id)] => *id,
                    [other] => {
                        return Err(VmError::new(format!(
                            "`async_join` exige handle (int), encontrou {}",
                            type_name(other)
                        )));
                    }
                    _ => return Err(VmError::new("`async_join` exige 1 argumento".to_string())),
                };
                let task = self.tasks.running.remove(&id).ok_or_else(|| {
                    VmError::new(format!(
                        "`async_join`: handle {id} invalido ou ja consumido"
                    ))
                })?;
                let value = self.join_removed_task(idx, task, "async_join")?;
                self.push_checked(idx, value)?;
                Ok(())
            }
            Builtin::AwaitTimeout => {
                let (id, secs) = match args.as_slice() {
                    [Value::Int(id), s] => (*id, timeout_secs(s, "await_timeout")?),
                    [other, _] => {
                        return Err(VmError::new(format!(
                            "`await_timeout` exige handle (int), encontrou {}",
                            type_name(other)
                        )));
                    }
                    _ => {
                        return Err(VmError::new(
                            "`await_timeout` exige 2 argumentos".to_string(),
                        ))
                    }
                };
                let task = self.tasks.running.remove(&id).ok_or_else(|| {
                    VmError::new(format!(
                        "`await_timeout`: handle {id} invalido ou ja consumido"
                    ))
                })?;
                let task_name = task.name.clone();
                match task.rx.recv_timeout(Duration::from_secs_f64(secs)) {
                    Ok(Ok(json)) => {
                        let value = self.decode_task_value(idx, &json, "await_timeout")?;
                        self.push_checked(idx, value)?;
                        Ok(())
                    }
                    Ok(Err(message)) => Err(VmError::new(format!(
                        "tarefa `{task_name}` falhou: {message}"
                    ))),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        // Devolve ao mapa: segue em voo, pode juntar depois.
                        self.tasks.running.insert(id, task);
                        Err(VmError::new(format!(
                            "`await_timeout`: tempo esgotado apos {secs}s aguardando tarefa `{task_name}` (segue em voo; junte depois)"
                        )))
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(VmError::new(
                        format!("`await_timeout`: tarefa `{task_name}` abortou (panic)"),
                    )),
                }
            }
            Builtin::AsyncCancel => {
                let id = match args.as_slice() {
                    [Value::Int(id)] => *id,
                    [other] => {
                        return Err(VmError::new(format!(
                            "`async_cancel` exige handle (int), encontrou {}",
                            type_name(other)
                        )));
                    }
                    _ => return Err(VmError::new("`async_cancel` exige 1 argumento".to_string())),
                };
                let task = self.tasks.running.get(&id).ok_or_else(|| {
                    VmError::new(format!(
                        "`async_cancel`: handle {id} invalido ou ja consumido"
                    ))
                })?;
                task.cancel.store(true, AtomicOrdering::SeqCst);
                self.push_checked(idx, Value::Null)?;
                Ok(())
            }
            Builtin::AsyncCancelled => {
                let flag = self
                    .cancel_flag
                    .as_ref()
                    .is_some_and(|f| f.load(AtomicOrdering::SeqCst));
                self.push_checked(idx, Value::Bool(flag))?;
                Ok(())
            }
            Builtin::AwaitAll => {
                let items = match args.as_slice() {
                    [Value::Array(items)] => items.clone(),
                    [other] => {
                        return Err(VmError::new(format!(
                            "`await_all` exige array de handles, encontrado {}",
                            type_name(other)
                        )));
                    }
                    _ => return Err(VmError::new("`await_all` exige 1 argumento".to_string())),
                };
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    let id = match item {
                        Value::Int(id) => id,
                        other => {
                            return Err(VmError::new(format!(
                                "`await_all` exige array de handles (int), encontrou {}",
                                type_name(&other)
                            )));
                        }
                    };
                    let task = self.tasks.running.remove(&id).ok_or_else(|| {
                        VmError::new(format!("`await_all`: handle {id} invalido ou ja consumido"))
                    })?;
                    // Falha rapida: erro aborta, demais seguem no mapa.
                    out.push(self.join_removed_task(idx, task, "await_all")?);
                }
                self.push_checked(idx, Value::Array(out))?;
                Ok(())
            }
            Builtin::AwaitRace => {
                let items = match args.as_slice() {
                    [Value::Array(items)] if !items.is_empty() => items.clone(),
                    [Value::Array(_)] => {
                        return Err(VmError::new(
                            "`await_race` exige ao menos 1 tarefa".to_string(),
                        ))
                    }
                    [other] => {
                        return Err(VmError::new(format!(
                            "`await_race` exige array de handles, encontrado {}",
                            type_name(other)
                        )));
                    }
                    _ => return Err(VmError::new("`await_race` exige 1 argumento".to_string())),
                };
                // Valida tudo antes de sondar (sem consumo parcial).
                let mut ids = Vec::with_capacity(items.len());
                for item in &items {
                    match item {
                        Value::Int(id) if self.tasks.running.contains_key(id) => ids.push(*id),
                        Value::Int(id) => {
                            return Err(VmError::new(format!(
                                "`await_race`: handle {id} invalido ou ja consumido"
                            )));
                        }
                        other => {
                            return Err(VmError::new(format!(
                                "`await_race` exige array de handles (int), encontrou {}",
                                type_name(other)
                            )));
                        }
                    }
                }
                // Sondagem: primeiro pronto vence; demais seguem no mapa.
                // Panic conta como conclusao (erro claro, como no `join`).
                loop {
                    enum Ready {
                        Done(i64, String),
                        Failed(i64, String),
                        Panicked(i64),
                    }
                    let mut ready: Option<Ready> = None;
                    for id in &ids {
                        let Some(task) = self.tasks.running.get(id) else {
                            return Err(VmError::new(
                                "`await_race`: handle {id} sumiu durante a sondagem".to_string(),
                            ));
                        };
                        match task.rx.try_recv() {
                            Ok(Ok(json)) => {
                                ready = Some(Ready::Done(*id, json));
                                break;
                            }
                            Ok(Err(message)) => {
                                ready = Some(Ready::Failed(*id, message));
                                break;
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => {}
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                ready = Some(Ready::Panicked(*id));
                                break;
                            }
                        }
                    }
                    match ready {
                        Some(Ready::Done(id, json)) => {
                            self.tasks.running.remove(&id).expect("validado");
                            let value = self.decode_task_value(idx, &json, "await_race")?;
                            self.push_checked(idx, value)?;
                            return Ok(());
                        }
                        Some(Ready::Failed(id, message)) => {
                            let task = self.tasks.running.remove(&id).expect("validado");
                            return Err(VmError::new(format!(
                                "tarefa `{}` falhou: {message}",
                                task.name
                            )));
                        }
                        Some(Ready::Panicked(id)) => {
                            let task = self.tasks.running.remove(&id).expect("validado");
                            return Err(VmError::new(format!(
                                "`await_race`: tarefa `{}` abortou (panic)",
                                task.name
                            )));
                        }
                        None => std::thread::sleep(Duration::from_millis(1)),
                    }
                }
            }
            _ => Err(VmError::new(
                "interno: builtin async desconhecido".to_string(),
            )),
        }
    }

    /// Inicia `map`/`filter`/`usort` (RFC 0005): valida `(colecao, funcao)`,
    /// guarda o progresso em `pending` e empilha a primeira ativacao do
    /// callback. Array vazio finaliza sem chamar.
    /// O indice do frame do callback vai para `pending_cb` (amarracao
    /// anti-obsoleto: ver RHP-AUD-IND-001).
    fn push_hof_call(
        &mut self,
        idx: usize,
        func: &FuncValue,
        args: Vec<Value>,
    ) -> Result<(), VmError> {
        self.push_call_frame(func.clone(), args)?;
        self.frames[idx].pending_cb = Some(self.frames.len() - 1);
        Ok(())
    }

    /// Inicia `map`/`filter`/`usort` (RFC 0005): valida `(colecao, funcao)`,
    /// guarda o progresso em `pending` e empilha a primeira ativacao do
    /// callback. Array vazio finaliza sem chamar.
    fn hof_begin(&mut self, idx: usize, builtin: Builtin, args: Vec<Value>) -> Result<(), VmError> {
        let name = match builtin {
            Builtin::Map => "map",
            Builtin::Filter => "filter",
            _ => "usort",
        };
        let (func, items) = match (&args[0], &args[1]) {
            (Value::Array(items), Value::Func(func)) => (func.clone(), items.clone()),
            (arr, f) => {
                return Err(VmError::new(format!(
                    "`{name}` exige (array, funcao), encontrado ({}, {})",
                    type_name(arr),
                    type_name(f)
                )));
            }
        };
        if func.def.is_generator {
            return Err(VmError::new(format!(
                "callback de `{name}` nao pode ser gerador (use `for x in gen` ou `next`)"
            )));
        }
        if items.is_empty() {
            self.push_checked(idx, Value::Array(Vec::new()))?;
            self.frames[idx].ip += 1;
            return Ok(());
        }
        match builtin {
            Builtin::Map | Builtin::Filter => {
                let first = items[0].clone();
                let pending = if matches!(builtin, Builtin::Map) {
                    PendingHof::Map {
                        func: func.clone(),
                        items,
                        next: 0,
                        acc: Vec::new(),
                    }
                } else {
                    PendingHof::Filter {
                        func: func.clone(),
                        items,
                        next: 0,
                        acc: Vec::new(),
                    }
                };
                self.frames[idx].pending = Some(pending);
                self.push_hof_call(idx, &func, vec![first])?;
                self.frames[idx].ip += 1;
                Ok(())
            }
            // Insertion sort passo a passo: primeira comparacao do par (1, 0).
            _ => {
                if items.len() < 2 {
                    self.push_checked(idx, Value::Array(items))?;
                    self.frames[idx].ip += 1;
                    return Ok(());
                }
                let a = items[1].clone();
                let b = items[0].clone();
                self.frames[idx].pending = Some(PendingHof::Sort {
                    func: func.clone(),
                    sorted: items,
                    i: 1,
                    j: 1,
                });
                self.push_hof_call(idx, &func, vec![a, b])?;
                self.frames[idx].ip += 1;
                Ok(())
            }
        }
    }

    /// Avanca o HOF com o retorno do callback: acumula e empilha a proxima
    /// chamada, ou finaliza empilhando o resultado. Chamado no `Return`.
    fn advance_hof(&mut self, idx: usize, pending: PendingHof, ret: Value) -> Result<(), VmError> {
        match pending {
            PendingHof::Map {
                func,
                items,
                mut next,
                mut acc,
            } => {
                acc.push(ret);
                next += 1;
                if next < items.len() {
                    let arg = items[next].clone();
                    self.frames[idx].pending = Some(PendingHof::Map {
                        func: func.clone(),
                        items,
                        next,
                        acc,
                    });
                    self.push_hof_call(idx, &func, vec![arg])
                } else {
                    self.push_checked(idx, Value::Array(acc))
                }
            }
            PendingHof::Filter {
                func,
                items,
                mut next,
                mut acc,
            } => {
                match ret {
                    Value::Bool(true) => acc.push(items[next].clone()),
                    Value::Bool(false) => {}
                    other => {
                        return Err(VmError::new(format!(
                            "`filter` exige retorno bool, encontrado {}",
                            type_name(&other)
                        )));
                    }
                }
                next += 1;
                if next < items.len() {
                    let arg = items[next].clone();
                    self.frames[idx].pending = Some(PendingHof::Filter {
                        func: func.clone(),
                        items,
                        next,
                        acc,
                    });
                    self.push_hof_call(idx, &func, vec![arg])
                } else {
                    self.push_checked(idx, Value::Array(acc))
                }
            }
            PendingHof::Sort {
                func,
                mut sorted,
                mut i,
                mut j,
            } => {
                let cmp = match ret {
                    Value::Int(c) => c,
                    other => {
                        return Err(VmError::new(format!(
                            "comparador de `usort` deve retornar int, encontrado {}",
                            type_name(&other)
                        )));
                    }
                };
                let n = sorted.len();
                if cmp < 0 {
                    sorted.swap(j, j - 1);
                    if j > 1 {
                        j -= 1;
                    } else {
                        i += 1;
                        j = i;
                    }
                } else {
                    i += 1;
                    j = i;
                }
                if i < n {
                    let a = sorted[j].clone();
                    let b = sorted[j - 1].clone();
                    self.frames[idx].pending = Some(PendingHof::Sort {
                        func: func.clone(),
                        sorted,
                        i,
                        j,
                    });
                    self.push_hof_call(idx, &func, vec![a, b])
                } else {
                    self.push_checked(idx, Value::Array(sorted))
                }
            }
        }
    }

    /// Desenrola frames ate o handler de `try` mais interno.
    ///
    /// Trunca frames, escopos e operandos; empilha o valor do erro e salta
    /// ao alvo. Cada frame desenrolado conta em `max_steps` (sem loop
    /// infinito: frames sao finitos). Sem handler, devolve o erro original
    /// (comportamento idêntico ao de antes das excecoes).
    fn unwind(&mut self, value: Value, original: VmError) -> Result<Step, VmError> {
        for depth in (0..self.frames.len()).rev() {
            self.steps += 1;
            if let Some(max) = self.limits.max_steps {
                if self.steps > max {
                    return Err(VmError::new(format!(
                        "limite de instrucoes excedido ({max} passos)"
                    )));
                }
            }
            if let Some(handler) = self.frames[depth].handlers.pop() {
                self.frames.truncate(depth + 1);
                {
                    let frame = &mut self.frames[depth];
                    frame.scopes.truncate(handler.scope_len);
                    frame.stack.clear();
                }
                // HOFs obsoletos: `pending` cujo callback foi truncado acima
                // nunca mais vai avancar — descarta agora, senao um `Return`
                // posterior nao relacionado o consumiria como retorno do
                // callback (RHP-AUD-IND-001: erro fantasma / valor errado).
                // `pending` com callback vivo (`cb <= depth`) e preservado:
                // o callback retomara e avancara normalmente.
                for frame in self.frames.iter_mut().take(depth + 1) {
                    if frame.pending_cb.is_some_and(|cb| cb > depth) {
                        frame.pending = None;
                        frame.pending_cb = None;
                    }
                }
                // `push_checked` aplica `max_collection_len` ao erro.
                self.push_checked(depth, value)?;
                self.frames[depth].ip = handler.target;
                return Ok(Step::Continue);
            }
        }
        Err(original)
    }

    /// Mensagem de erro nao capturado a partir do valor propagado: erro
    /// interno preserva a mensagem original (via mapa `{tipo, mensagem}`);
    /// valor lancado a mao usa `excecao nao capturada: ...`.
    fn uncaught_message(value: &Value) -> String {
        match value {
            Value::Map(entries) if matches!(entries.get("tipo"), Some(Value::Str(t)) if t == "runtime") => {
                match entries.get("mensagem") {
                    Some(Value::Str(message)) => message.clone(),
                    _ => format!("{value}"),
                }
            }
            other => format!("excecao nao capturada: {other}"),
        }
    }

    /// Valor do erro interno para `catch`: mapa `{tipo, mensagem}`.
    fn internal_error_value(message: &str) -> Value {
        let mut entries = HashMap::with_capacity(2);
        entries.insert("tipo".to_string(), Value::Str("runtime".to_string()));
        entries.insert("mensagem".to_string(), Value::Str(message.to_string()));
        Value::Map(entries)
    }
}

/// Tamanho para fins de teto: bytes (strings) ou elementos.
fn collection_len(value: &Value) -> usize {
    match value {
        Value::Str(s) => s.len(),
        Value::Array(items) => items.len(),
        Value::Map(entries) => entries.len(),
        _ => 0,
    }
}

/// Estimativa da heap viva para fins de teto (bytes do modelo, RFC 0015):
/// escalar = 0; `Str` = `len`; `Array` = 8 por elemento + recursao;
/// `Map`/`Func.env` = `key.len` + 8 por entrada + recursao. Nao mede o
/// alocador real (overhead de `Vec`/`HashMap` fora; direcao documentada
/// na spec). Alem de `MAX_VALUE_DEPTH`, conta so o nivel imediato.
fn heap_size_of(value: &Value, depth: usize) -> u64 {
    match value {
        Value::Str(s) => s.len() as u64,
        Value::Array(items) => {
            let mut total = (items.len() as u64).saturating_mul(8);
            if depth <= MAX_VALUE_DEPTH {
                for item in items {
                    total = total.saturating_add(heap_size_of(item, depth + 1));
                }
            }
            total
        }
        Value::Map(entries) => heap_size_of_entries(entries, depth),
        Value::Func(f) => heap_size_of_entries(&f.env, depth),
        Value::Class(def) => def.name.len() as u64,
        Value::Instance { class, fields } => {
            (class.name.len() as u64).saturating_add(heap_size_of_entries(fields, depth))
        }
        Value::Generator(gen) => {
            // `try_borrow`: se o frame suspenso (transitivamente) contem
            // este mesmo gerador, o borrow aninhado falharia (`panic`) —
            // nesse caso conta 0 (estimativa; fail-safe documentado).
            let Ok(state) = gen.0.try_borrow() else {
                return 0;
            };
            let mut total = heap_size_of_entries(&state.func.env, depth);
            if let Some(frame) = &state.frame {
                for value in &frame.stack {
                    total = total.saturating_add(heap_size_of(value, depth + 1));
                }
                for scope in &frame.scopes {
                    total = total.saturating_add(heap_size_of_entries(scope, depth + 1));
                }
            }
            total
        }
        _ => 0,
    }
}

/// Soma de um mapa `nome -> valor` (escopo, global, statics, env).
fn heap_size_of_entries(entries: &HashMap<String, Value>, depth: usize) -> u64 {
    let mut total = 0u64;
    for (k, v) in entries {
        total = total.saturating_add(k.len() as u64).saturating_add(8);
        if depth <= MAX_VALUE_DEPTH {
            total = total.saturating_add(heap_size_of(v, depth + 1));
        }
    }
    total
}

fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Int(_) => "int",
        Value::Float(_) => "float",
        Value::Str(_) => "string",
        Value::Array(_) => "array",
        Value::Map(_) => "mapa",
        Value::Func(_) => "funcao",
        Value::Class(_) => "classe",
        Value::Instance { .. } => "instancia",
        Value::Generator(_) => "gerador",
    }
}

/// Checagem de anotacao de tipo (`: tipo`; RFC 0016, Etapa 5). So valida,
/// nunca converte. Mensagem com prefixo `tipos incompativeis` (faixa E3001).
fn check_type(value: &Value, tag: TypeTag) -> bool {
    match (value, tag) {
        (Value::Int(_), TypeTag::Int)
        | (Value::Float(_), TypeTag::Float)
        | (Value::Str(_), TypeTag::String)
        | (Value::Bool(_), TypeTag::Bool)
        | (Value::Array(_), TypeTag::Array)
        | (Value::Map(_), TypeTag::Map)
        | (Value::Null, TypeTag::Null)
        | (Value::Func(_), TypeTag::Func) => true,
        // Classes/instancias/geradores nao tem tag (distintos de
        // mapa/funcao): nunca casam.
        _ => false,
    }
}

fn check_type_or_err(value: &Value, tag: TypeTag) -> Result<(), VmError> {
    if check_type(value, tag) {
        Ok(())
    } else {
        Err(VmError::new(format!(
            "tipos incompativeis: esperava {}, encontrou {}",
            tag.name(),
            type_name(value)
        )))
    }
}

fn add_values(left: &Value, right: &Value) -> Result<Value, VmError> {
    match (left, right) {
        (Value::Int(a), Value::Int(b)) => a
            .checked_add(*b)
            .map(Value::Int)
            .ok_or_else(|| VmError::new("estouro de inteiro em `+`".to_string())),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 + b)),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a + *b as f64)),
        (Value::Str(a), Value::Str(b)) => Ok(Value::Str(format!("{a}{b}"))),
        _ => Err(VmError::new(format!(
            "tipos incompativeis para `+`: {} e {}",
            type_name(left),
            type_name(right)
        ))),
    }
}

fn sub_values(left: &Value, right: &Value) -> Result<Value, VmError> {
    match (left, right) {
        (Value::Int(a), Value::Int(b)) => a
            .checked_sub(*b)
            .map(Value::Int)
            .ok_or_else(|| VmError::new("estouro de inteiro em `-`".to_string())),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 - b)),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a - *b as f64)),
        _ => Err(VmError::new(format!(
            "tipos incompativeis para `-`: {} e {}",
            type_name(left),
            type_name(right)
        ))),
    }
}

fn mul_values(left: &Value, right: &Value) -> Result<Value, VmError> {
    match (left, right) {
        (Value::Int(a), Value::Int(b)) => a
            .checked_mul(*b)
            .map(Value::Int)
            .ok_or_else(|| VmError::new("estouro de inteiro em `*`".to_string())),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 * b)),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a * *b as f64)),
        _ => Err(VmError::new(format!(
            "tipos incompativeis para `*`: {} e {}",
            type_name(left),
            type_name(right)
        ))),
    }
}

fn div_values(left: &Value, right: &Value) -> Result<Value, VmError> {
    match (left, right) {
        (Value::Int(_), Value::Int(0)) => Err(VmError::new("divisao por zero")),
        (Value::Int(a), Value::Int(b)) => a
            .checked_div(*b)
            .map(Value::Int)
            .ok_or_else(|| VmError::new("estouro de inteiro em `/`".to_string())),
        (_, Value::Float(b)) if *b == 0.0 => Err(VmError::new("divisao por zero")),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 / b)),
        (Value::Float(a), Value::Int(b)) => {
            if *b == 0 {
                return Err(VmError::new("divisao por zero"));
            }
            Ok(Value::Float(a / *b as f64))
        }
        _ => Err(VmError::new(format!(
            "tipos incompativeis para `/`: {} e {}",
            type_name(left),
            type_name(right)
        ))),
    }
}

fn mod_values(left: &Value, right: &Value) -> Result<Value, VmError> {
    match (left, right) {
        (_, Value::Int(0)) => Err(VmError::new("resto por zero")),
        (Value::Int(a), Value::Int(b)) => a
            .checked_rem(*b)
            .map(Value::Int)
            .ok_or_else(|| VmError::new("estouro de inteiro em `%`".to_string())),
        _ => Err(VmError::new(format!(
            "operador `%` exige int e int, encontrado {} e {}",
            type_name(left),
            type_name(right)
        ))),
    }
}

fn neg_value(value: &Value) -> Result<Value, VmError> {
    match value {
        Value::Int(v) => v
            .checked_neg()
            .map(Value::Int)
            .ok_or_else(|| VmError::new("estouro de inteiro em `-`".to_string())),
        Value::Float(v) => Ok(Value::Float(-v)),
        other => Err(VmError::new(format!(
            "operador `-` exige numero, encontrado {}",
            type_name(other)
        ))),
    }
}

fn not_value(value: &Value) -> Result<Value, VmError> {
    match value {
        Value::Bool(b) => Ok(Value::Bool(!b)),
        other => Err(VmError::new(format!(
            "operador `!` exige bool, encontrado {}",
            type_name(other)
        ))),
    }
}

fn values_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Int(a), Value::Int(b)) => a == b,
        (Value::Float(a), Value::Float(b)) => a == b,
        (Value::Int(a), Value::Float(b)) => *a as f64 == *b,
        (Value::Float(a), Value::Int(b)) => *a == *b as f64,
        (Value::Str(a), Value::Str(b)) => a == b,
        (Value::Array(a), Value::Array(b)) => a == b,
        (Value::Map(a), Value::Map(b)) => a == b,
        (Value::Func(a), Value::Func(b)) => Rc::ptr_eq(&a.def, &b.def) && a.env == b.env,
        // Classes: igualdade por nome (tabelas de metodos sao imutaveis e
        // compartilhadas; comparar corpos seria profundo e fragil).
        (Value::Class(a), Value::Class(b)) => a.name == b.name,
        // Instancias: mesma classe (por nome) + campos iguais.
        // Sem risco de recursao infinita: valores sao arvores finitas
        // (copias por valor nunca formam ciclos; vide MEMORY_MODEL).
        (
            Value::Instance {
                class: ca,
                fields: fa,
            },
            Value::Instance {
                class: cb,
                fields: fb,
            },
        ) => ca.name == cb.name && fa == fb,
        // Geradores: identidade (`Rc::ptr_eq` + `done`).
        (Value::Generator(a), Value::Generator(b)) => a == b,
        _ => false,
    }
}

/// Igualdade estrita (RFC 0008, `match`): mesmo tipo + valor; `Int`/`Float`
/// cruzam numericamente (como no `==`); resto exige a mesma variante.
fn strict_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Int(a), Value::Int(b)) => a == b,
        (Value::Float(a), Value::Float(b)) => a == b,
        (Value::Int(a), Value::Float(b)) => *a as f64 == *b,
        (Value::Float(a), Value::Int(b)) => *a == *b as f64,
        _ => {
            std::mem::discriminant(left) == std::mem::discriminant(right)
                && values_equal(left, right)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ordering {
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

fn compare_values(left: &Value, right: &Value, op: Ordering) -> Result<Value, VmError> {
    let ordered = match (left, right) {
        (Value::Int(a), Value::Int(b)) => Some(a.cmp(b)),
        (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
        (Value::Int(a), Value::Float(b)) => (*a as f64).partial_cmp(b),
        (Value::Float(a), Value::Int(b)) => a.partial_cmp(&(*b as f64)),
        (Value::Str(a), Value::Str(b)) => Some(a.cmp(b)),
        _ => None,
    };
    let Some(order) = ordered else {
        return Err(VmError::new(format!(
            "tipos incompativeis para comparacao: {} e {}",
            type_name(left),
            type_name(right)
        )));
    };
    let result = match op {
        Ordering::Less => order == std::cmp::Ordering::Less,
        Ordering::LessOrEqual => order != std::cmp::Ordering::Greater,
        Ordering::Greater => order == std::cmp::Ordering::Greater,
        Ordering::GreaterOrEqual => order != std::cmp::Ordering::Less,
    };
    Ok(Value::Bool(result))
}

fn bitand_values(left: &Value, right: &Value) -> Result<Value, VmError> {
    match (left, right) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a & b)),
        _ => Err(VmError::new(format!(
            "operador `&` exige int e int, encontrado {} e {}",
            type_name(left),
            type_name(right)
        ))),
    }
}

fn bitor_values(left: &Value, right: &Value) -> Result<Value, VmError> {
    match (left, right) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a | b)),
        _ => Err(VmError::new(format!(
            "operador `|` exige int e int, encontrado {} e {}",
            type_name(left),
            type_name(right)
        ))),
    }
}

fn bitxor_values(left: &Value, right: &Value) -> Result<Value, VmError> {
    match (left, right) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a ^ b)),
        _ => Err(VmError::new(format!(
            "operador `^` exige int e int, encontrado {} e {}",
            type_name(left),
            type_name(right)
        ))),
    }
}

fn shl_values(left: &Value, right: &Value) -> Result<Value, VmError> {
    match (left, right) {
        (Value::Int(a), Value::Int(b)) => a
            .checked_shl(*b as u32)
            .map(Value::Int)
            .ok_or_else(|| VmError::new(format!("deslocamento invalido em `<<`: {b}"))),
        _ => Err(VmError::new(format!(
            "operador `<<` exige int e int, encontrado {} e {}",
            type_name(left),
            type_name(right)
        ))),
    }
}

fn shr_values(left: &Value, right: &Value) -> Result<Value, VmError> {
    match (left, right) {
        (Value::Int(a), Value::Int(b)) => a
            .checked_shr(*b as u32)
            .map(Value::Int)
            .ok_or_else(|| VmError::new(format!("deslocamento invalido em `>>`: {b}"))),
        _ => Err(VmError::new(format!(
            "operador `>>` exige int e int, encontrado {} e {}",
            type_name(left),
            type_name(right)
        ))),
    }
}

fn bitnot_value(value: &Value) -> Result<Value, VmError> {
    match value {
        Value::Int(v) => Ok(Value::Int(!v)),
        other => Err(VmError::new(format!(
            "operador `~` exige int, encontrado {}",
            type_name(other)
        ))),
    }
}

fn index_value(base: &Value, index: &Value) -> Result<Value, VmError> {
    match (base, index) {
        (Value::Array(items), Value::Int(i)) => {
            if *i < 0 || (*i as usize) >= items.len() {
                return Err(VmError::new(format!(
                    "indice {} fora do intervalo (tamanho {})",
                    i,
                    items.len()
                )));
            }
            Ok(items[*i as usize].clone())
        }
        (Value::Map(entries), Value::Str(key)) => entries
            .get(key)
            .cloned()
            .ok_or_else(|| VmError::new(format!("chave inexistente no mapa: {key}"))),
        (Value::Instance { class, fields }, Value::Str(key)) => {
            // Campos primeiro; depois metodos (vinculados a `this`).
            // Sem aliasing: a instancia e por valor (copia), como mapas.
            if let Some(value) = fields.get(key) {
                Ok(value.clone())
            } else if let Some(func) = class.methods.get(key) {
                // Metodo vinculado: env capturado na definicao + `this`.
                let mut env = func.env.clone();
                env.insert(
                    "this".to_string(),
                    Value::Instance {
                        class: class.clone(),
                        fields: fields.clone(),
                    },
                );
                Ok(Value::Func(FuncValue {
                    def: func.def.clone(),
                    env,
                }))
            } else {
                Err(VmError::new(format!(
                    "membro inexistente na instancia de `{}`: `{key}` (sem campo nem metodo)",
                    class.name,
                )))
            }
        }
        (Value::Class(_), _) => Err(VmError::new(
            "indexacao de classe nao suportada (instancie com `new`)".to_string(),
        )),
        _ => Err(VmError::new(format!(
            "indexacao nao suportada para {}[{}]",
            type_name(base),
            type_name(index)
        ))),
    }
}

fn store_index_value(container: &Value, index: &Value, value: Value) -> Result<Value, VmError> {
    match (container, index) {
        (Value::Array(items), Value::Int(i)) => {
            if *i < 0 || (*i as usize) >= items.len() {
                return Err(VmError::new(format!(
                    "indice {} fora do intervalo (tamanho {})",
                    i,
                    items.len()
                )));
            }
            let mut updated = items.clone();
            updated[*i as usize] = value;
            Ok(Value::Array(updated))
        }
        (Value::Map(entries), Value::Str(key)) => {
            let mut updated = entries.clone();
            updated.insert(key.clone(), value);
            Ok(Value::Map(updated))
        }
        (Value::Instance { class, fields }, Value::Str(key)) => {
            // Campos sao mutaveis (`this["n"] = ...`); metodos nao sao
            // sobrescreviveis por atribuicao (erro claro).
            if class.methods.contains_key(key) {
                return Err(VmError::new(format!(
                    "metodo `{key}` nao pode ser sobrescrito por atribuicao"
                )));
            }
            let mut updated = fields.clone();
            updated.insert(key.clone(), value);
            Ok(Value::Instance {
                class: class.clone(),
                fields: updated,
            })
        }
        _ => Err(VmError::new(format!(
            "atribuicao indexada nao suportada para {}[{}]",
            type_name(container),
            type_name(index)
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rhp_bytecode::CompileError;
    use rhp_parser::parse_source;

    fn compile_src(src: &str) -> Chunk {
        let program = parse_source(src).expect("parse deve passar");
        rhp_bytecode::compile(&program).expect("compile deve passar")
    }

    fn run_src(src: &str) -> Result<String, VmError> {
        let chunk = compile_src(src);
        Vm::new().run_to_string(&chunk)
    }

    fn run_err(src: &str) -> String {
        run_src(src).expect_err("deve falhar").message
    }

    fn run_limited(src: &str, limits: VmLimits) -> Result<String, VmError> {
        let chunk = compile_src(src);
        let mut vm = Vm::new();
        vm.set_limits(limits);
        vm.run_to_string(&chunk)
    }

    fn limited(max_collection: usize) -> VmLimits {
        VmLimits {
            max_collection_len: Some(max_collection),
            ..Default::default()
        }
    }

    #[test]
    fn respects_stack_depth_limit() {
        let mut code = Vec::new();
        for _ in 0..200 {
            code.push(Op::Constant(0));
        }
        code.push(Op::Halt);
        let chunk = Chunk {
            constants: vec![Value::Int(1)],
            names: vec![],
            funcs: vec![],
            code,
        };
        let mut vm = Vm::new();
        vm.set_limits(VmLimits {
            max_stack_depth: Some(100),
            ..Default::default()
        });
        let err = vm.run_to_string(&chunk).expect_err("deve falhar");
        assert!(err.message.contains("pilha de operandos"));
    }

    #[test]
    fn respects_collection_limit() {
        let err = run_limited("echo [1, 2, 3];", limited(2)).expect_err("deve falhar");
        assert!(err.message.contains("colecao excede o limite"));
        let err = run_limited("echo \"ab\" + \"c\";", limited(2)).expect_err("deve falhar");
        assert!(err.message.contains("colecao excede o limite"));
        let err =
            run_limited("let a = [1]; echo push(a, 2);", limited(1)).expect_err("deve falhar");
        assert!(err.message.contains("colecao excede o limite"));
        // Sem limite, passa.
        assert_eq!(run_src("echo [1, 2, 3];").expect("ok"), "[1, 2, 3]\n");
    }

    #[test]
    fn runs_hello_world() {
        let chunk = compile_src("echo \"Hello from RHP!\";");
        let out = Vm::new().run_to_string(&chunk).expect("vm deve passar");
        assert_eq!(out, "Hello from RHP!\n");
    }

    #[test]
    fn runs_empty_chunk_without_output() {
        let chunk = Chunk {
            constants: vec![],
            names: vec![],
            funcs: vec![],
            code: vec![Op::Halt],
        };
        let out = Vm::new().run_to_string(&chunk).expect("deve passar");
        assert_eq!(out, "");
    }

    #[test]
    fn evaluates_precedence_and_parens() {
        assert_eq!(run_src("echo 1 + 2 * 3;").expect("ok"), "7\n");
        assert_eq!(run_src("echo (1 + 2) * 3;").expect("ok"), "9\n");
        assert_eq!(run_src("echo 10 - 4 - 3;").expect("ok"), "3\n");
        assert_eq!(run_src("echo 20 / 4;").expect("ok"), "5\n");
        assert_eq!(run_src("echo 20 % 6;").expect("ok"), "2\n");
        assert_eq!(run_src("echo -5 + 8;").expect("ok"), "3\n");
        assert_eq!(run_src("echo 1.5 + 2;").expect("ok"), "3.5\n");
        assert_eq!(run_src("echo !true;").expect("ok"), "false\n");
    }

    #[test]
    fn concatenates_strings() {
        assert_eq!(
            run_src("echo \"ola\" + \" \" + \"mundo\";").expect("ok"),
            "ola mundo\n"
        );
    }

    #[test]
    fn compares_values() {
        assert_eq!(run_src("echo 1 < 2;").expect("ok"), "true\n");
        assert_eq!(run_src("echo 2 <= 2;").expect("ok"), "true\n");
        assert_eq!(run_src("echo 3 > 4;").expect("ok"), "false\n");
        assert_eq!(run_src("echo 1 == 1.0;").expect("ok"), "true\n");
        assert_eq!(run_src("echo 1 != 2;").expect("ok"), "true\n");
        assert_eq!(run_src("echo null == null;").expect("ok"), "true\n");
    }

    #[test]
    fn reports_division_by_zero() {
        assert!(run_err("echo 1 / 0;").contains("divisao por zero"));
        assert!(run_err("echo 1 % 0;").contains("resto por zero"));
    }

    #[test]
    fn reports_integer_overflow() {
        assert!(run_err("echo 9223372036854775807 + 1;").contains("estouro"));
    }

    #[test]
    fn reports_type_errors() {
        assert!(run_err("echo 1 + true;").contains("incompativeis"));
        assert!(run_err("echo 1 < true;").contains("incompativeis"));
        assert!(run_err("echo !1;").contains("exige bool"));
        assert!(run_err("if 1 { echo 2; }").contains("deve ser bool"));
    }

    #[test]
    fn handles_variables_and_scopes() {
        assert_eq!(run_src("let x = 40 + 2; echo x;").expect("ok"), "42\n");
        assert_eq!(run_src("let x; echo x;").expect("ok"), "null\n");
        // Shadowing em bloco interno nao afeta o externo.
        assert_eq!(
            run_src("let x = 1; { let x = 2; echo x; } echo x;").expect("ok"),
            "2\n1\n"
        );
    }

    #[test]
    fn reports_undefined_and_redeclared() {
        assert!(run_err("echo y;").contains("nao definida"));
        assert!(run_err("x = 1;").contains("nao definida"));
        assert!(run_err("let x = 1; let x = 2;").contains("ja declarada"));
    }

    #[test]
    fn runs_if_else_and_while() {
        assert_eq!(
            run_src("let x = 2; if x > 1 { echo \"maior\"; } else { echo \"menor\"; }")
                .expect("ok"),
            "maior\n"
        );
        assert_eq!(
            run_src("if false { echo 1; } else { echo 2; }").expect("ok"),
            "2\n"
        );
        assert_eq!(
            run_src("let i = 0; while i < 3 { echo i; i = i + 1; }").expect("ok"),
            "0\n1\n2\n"
        );
    }

    #[test]
    fn calls_functions_and_returns() {
        assert_eq!(
            run_src("fn soma(a, b) { return a + b; } echo soma(3, 4);").expect("ok"),
            "7\n"
        );
        assert_eq!(run_src("fn f() { } echo f();").expect("ok"), "null\n");
        // Recursao: fib(10) = 55.
        let fib =
            "fn fib(n) { if n < 2 { return n; } return fib(n - 1) + fib(n - 2); } echo fib(10);";
        assert_eq!(run_src(fib).expect("ok"), "55\n");
    }

    #[test]
    fn runs_map_with_callback() {
        assert_eq!(
            run_src("echo map([1, 2, 3], fn (x) { return x * 2; });").expect("ok"),
            "[2, 4, 6]\n"
        );
        // Callback captura o ambiente (closure de verdade, nao funcao solta).
        assert_eq!(
            run_src("let k = 10; echo map([1, 2], fn (x) { return x + k; });").expect("ok"),
            "[11, 12]\n"
        );
        // Vazio nao chama; aninhado funciona (`pending` e por frame).
        assert_eq!(
            run_src("echo map([], fn (x) { return x; });").expect("ok"),
            "[]\n"
        );
        assert_eq!(
            run_src(
                "echo map([[1, 2], [3]], fn (r) { return map(r, fn (x) { return x * 10; }); });"
            )
            .expect("ok"),
            "[[10, 20], [30]]\n"
        );
        // Erros claros: tipos e aridade do callback.
        assert!(run_err("echo map(1, fn (x) { return x; });").contains("array, funcao"));
        assert!(run_err("echo map([1], 42);").contains("array, funcao"));
        assert!(run_err("echo map([1], fn (a, b) { return a; });").contains("ao menos 2"));
        // `throw` no callback propaga (estado pendente descartado com o frame).
        assert_eq!(
            run_src("try { echo map([1], fn (x) { throw \"boom\"; }); } catch e { echo e; }")
                .expect("ok"),
            "boom\n"
        );
    }

    #[test]
    fn runs_hof_throw_does_not_leak_stale_pending() {
        // RHP-AUD-IND-001: `pending` do HOF cujo callback falhou era
        // consumido por um `Return` posterior nao relacionado (erro
        // fantasma ou valor errado). O `unwind` agora descarta pendings
        // cujo callback foi truncado.
        assert_eq!(
            run_src("try { echo usort([3, 1, 2], fn (a, b) { throw \"x\"; }); } catch e { echo \"pegou\"; } fn f() { try { throw \"fundo\"; } catch e { return \"n:\" + e; } } echo f();").expect("ok"),
            "pegou\nn:fundo\n"
        );
        // Variante com `map` + retorno de array (antes: resultado errado).
        assert_eq!(
            run_src("try { echo map([1], fn (x) { throw \"x\"; }); } catch e { echo \"pegou\"; } fn f() { return [7]; } echo f();").expect("ok"),
            "pegou\n[7]\n"
        );
        // Variante com `filter` + chamada aninhada apos a captura.
        assert_eq!(
            run_src("try { echo filter([1], fn (x) { throw \"x\"; }); } catch e { echo \"pegou\"; } fn g() { return 1; } fn f() { return g() + 1; } echo f();").expect("ok"),
            "pegou\n2\n"
        );
        // HOF normal apos falha anterior segue funcionando (sem resto).
        assert_eq!(
            run_src("try { echo map([1], fn (x) { throw \"x\"; }); } catch e { echo \"pegou\"; } echo map([1, 2], fn (x) { return x * 2; });").expect("ok"),
            "pegou\n[2, 4]\n"
        );
    }

    #[test]
    fn runs_filter_with_callback() {
        assert_eq!(
            run_src("echo filter([1, 2, 3, 4], fn (x) { return x > 2; });").expect("ok"),
            "[3, 4]\n"
        );
        assert_eq!(
            run_src("echo filter([], fn (x) { return true; });").expect("ok"),
            "[]\n"
        );
        // Sem truthiness: retorno nao-bool e erro (divergencia PHP documentada).
        assert!(run_err("echo filter([1], fn (x) { return x; });").contains("bool"));
    }

    #[test]
    fn runs_usort_with_comparator() {
        assert_eq!(
            run_src("echo usort([3, 1, 2], fn (a, b) { return a - b; });").expect("ok"),
            "[1, 2, 3]\n"
        );
        assert_eq!(
            run_src("echo usort([1, 2, 3], fn (a, b) { return b - a; });").expect("ok"),
            "[3, 2, 1]\n"
        );
        // Estavel: empates mantem a ordem original.
        assert_eq!(
            run_src("echo usort([{k: 1, v: \"a\"}, {k: 1, v: \"b\"}, {k: 0, v: \"c\"}], fn (x, y) { return x[\"k\"] - y[\"k\"]; });")
                .expect("ok"),
            "[{k: 0, v: c}, {k: 1, v: a}, {k: 1, v: b}]\n"
        );
        assert_eq!(
            run_src("echo usort([], fn (a, b) { return 0; });").expect("ok"),
            "[]\n"
        );
        assert_eq!(
            run_src("echo usort([5], fn (a, b) { return 0; });").expect("ok"),
            "[5]\n"
        );
        // Comparador deve retornar int.
        assert!(run_err("echo usort([2, 1], fn (a, b) { return true; });").contains("int"));
        assert!(run_err("echo usort(1, fn (a, b) { return 0; });").contains("array, funcao"));
    }

    #[test]
    fn runs_default_and_variadic_args() {
        // Default aplicado; expressao constante dobrada em compilacao.
        assert_eq!(
            run_src("fn f(a, b = 2, c = 1 + 2) { return a + b + c; } echo f(10);").expect("ok"),
            "15\n"
        );
        // `null` explicito e valor (nao aciona default).
        assert_eq!(
            run_src(
                "fn f(a = 1) { if a == null { return \"nulo\"; } return \"cheio\"; } echo f(null);"
            )
            .expect("ok"),
            "nulo\n"
        );
        // `...rest` coleta excedentes (`[]` se nenhum).
        assert_eq!(
            run_src("fn f(a, ...rest) { return rest; } echo f(1); echo f(1, 2, 3);").expect("ok"),
            "[]\n[2, 3]\n"
        );
        // Combinado: default + variadico; `...rest` sombreia captura.
        assert_eq!(
            run_src("let rest = 99; let f = fn (a, b = 20, ...rest) { return a + b + rest[0]; }; echo f(1, 2, 3); echo rest;")
                .expect("ok"),
            "6\n99\n"
        );
        // Callback com default funciona via HOF (mesmo caminho de chamada).
        assert_eq!(
            run_src("echo map([1], fn (x, y = 10) { return x + y; });").expect("ok"),
            "[11]\n"
        );
        // Erros: abaixo do minimo, acima do maximo sem `...`, default nao-const.
        assert!(run_err("fn f(a, b = 1) { return a; } echo f();").contains("ao menos 1"));
        assert!(run_err("fn f(a) { return a; } echo f(1, 2);").contains("espera 1"));
    }

    #[test]
    fn rejects_second_variadic_at_parse() {
        // `...` so um e por ultimo: o segundo nao e aceito no parse.
        let err = parse_source("fn f(a, ...r, ...s) { return a; }")
            .expect_err("segundo `...` deve falhar");
        assert!(err.contains("`)`"), "obteve: {err}");
    }

    #[test]
    fn runs_destructuring() {
        // Array por posicao; excedentes ignorados.
        assert_eq!(
            run_src("let [a, b] = [1, 2, 3]; echo a; echo b;").expect("ok"),
            "1\n2\n"
        );
        // Mapa: atalho e renomeado.
        assert_eq!(
            run_src("let {x} = {x: 10, y: 20}; echo x;").expect("ok"),
            "10\n"
        );
        assert_eq!(
            run_src("let {k: v} = {k: 99}; echo v;").expect("ok"),
            "99\n"
        );
        // `const` congela todos (atribuir depois e erro de compilacao).
        assert_eq!(
            run_src("const [c, d] = [3, 4]; echo c + d;").expect("ok"),
            "7\n"
        );
        // RHS avaliado uma unica vez.
        assert_eq!(
            run_src("let n = 0; fn prox() { n = n + 1; return [7, 8]; } let [a, b] = prox(); echo a; echo b; echo n;")
                .expect("ok"),
            "7\n8\n1\n"
        );
        // Erros herdados: indice fora, chave ausente, duplicata, const.
        assert!(run_err("let [a, b] = [1]; echo a;").contains("fora do intervalo"));
        assert!(run_err("let {z} = {x: 1}; echo z;").contains("chave"));
        assert!(run_err("let [a, a] = [1, 2];").contains("ja declarada"));
        // Atribuir a `const` desestruturado e erro de compilacao (regra vigente).
        let program = parse_source("const [c] = [1]; c = 2;").expect("parse deve passar");
        let err = rhp_bytecode::compile(&program).expect_err("atribuicao a const deve falhar");
        assert!(err.message.contains("constante"), "obteve: {}", err.message);
    }

    #[test]
    fn runs_named_and_spread_calls() {
        // Ordem irrelevante; posicional + nomeado mistos.
        assert_eq!(
            run_src("fn soma(a, b) { return a + b; } echo soma(b: 2, a: 1); echo soma(10, 20);")
                .expect("ok"),
            "3\n30\n"
        );
        // Defaults + variadico combinados.
        assert_eq!(
            run_src("fn f(a, b = 5, ...rest) { echo a; echo b; echo rest; } f(a: 1); f(1, 2, 3);")
                .expect("ok"),
            "1\n5\n[]\n1\n2\n[3]\n"
        );
        // Spread: splice em ordem, multiplo, com posicional e nomeado.
        assert_eq!(
            run_src("fn soma(a, b) { return a + b; } let t = [7, 8]; echo soma(...t); echo soma(100, ...[1]);")
                .expect("ok"),
            "15\n101\n"
        );
        assert_eq!(
            run_src("fn g(a, b, c) { return a + b + c; } echo g(...[1, 2], c: 30);").expect("ok"),
            "33\n"
        );
        // `null` explicito nao aciona default; callee arbitrario funciona.
        assert_eq!(
            run_src("fn f(a = 1) { return a; } echo f(a: null); let g = fn (x, y) { return x - y; }; echo g(y: 10, x: 3); echo g(...[20, 4]);")
                .expect("ok"),
            "null\n-7\n16\n"
        );
        // Erros de runtime: desconhecido, conflito posicional+nomeado, spread nao-array.
        assert!(run_err("fn f(a) { return a; } echo f(x: 1);").contains("desconhecido"));
        assert!(run_err("fn f(a) { return a; } echo f(1, a: 2);").contains("ja preenchido"));
        assert!(run_err("fn f(a) { return a; } echo f(...1);").contains("spread exige array"));
        assert!(run_err("fn f(a, b) { return a; } echo f(1);").contains("ao menos"));
    }

    #[test]
    fn runs_function_statics() {
        // Contador persiste entre chamadas; por definicao (outra funcao isolada).
        assert_eq!(
            run_src("fn c() { static n = 0; n = n + 1; return n; } echo c(); echo c(); echo c();")
                .expect("ok"),
            "1\n2\n3\n"
        );
        assert_eq!(
            run_src("fn a() { static n = 0; return n + 1; } fn b() { static n = 100; return n + 1; } echo a(); echo b();")
                .expect("ok"),
            "1\n101\n"
        );
        // Instancias da mesma closure compartilham (mesma definicao).
        assert_eq!(
            run_src("let mk = fn () { static c = 0; c = c + 1; return c; }; let f = mk; let g = mk; echo f(); echo g();")
                .expect("ok"),
            "1\n2\n"
        );
        // Init roda por chamada, mas so a primeira atribui (documentado).
        assert_eq!(
            run_src("let n = 0; fn p() { n = n + 1; return n * 100; } fn f() { static v = p(); return v; } echo f(); echo f(); echo n;")
                .expect("ok"),
            "100\n100\n2\n"
        );
        // Leitura sem declaracao executada = erro; fora de funcao = compilacao.
        assert!(
            run_err("fn f() { if false { static n = 1; } echo n; } f();").contains("nao definida")
        );
        let program = parse_source("static x = 1;").expect("parse deve passar");
        let err = rhp_bytecode::compile(&program).expect_err("static fora deve falhar");
        assert!(
            err.message.contains("fora de funcao"),
            "obteve: {}",
            err.message
        );
    }

    #[test]
    fn runs_rand_builtin() {
        // Faixa e tipo: 100 sorteios em `[0, i64::MAX]`, todos `Int`.
        for _ in 0..100 {
            match run_src("echo rand();").expect("ok").trim().parse::<i64>() {
                Ok(n) => assert!(n >= 0, "fora da faixa: {n}"),
                Err(e) => panic!("rand nao devolveu Int: {e}"),
            }
        }
        // `rand_range`: incluso nos limites; `min == max` devolve ele.
        for _ in 0..200 {
            let n: i64 = run_src("echo rand_range(1, 6);")
                .expect("ok")
                .trim()
                .parse()
                .expect("deve ser Int");
            assert!((1..=6).contains(&n), "fora da faixa: {n}");
        }
        assert_eq!(run_src("echo rand_range(5, 5);").expect("ok"), "5\n");
        // Erros: min > max, nao-int.
        assert!(run_err("echo rand_range(2, 1);").contains("min <="));
        assert!(run_err("echo rand_range(\"a\", 1);").contains("inteiros"));
    }

    #[test]
    fn hashes_and_verifies_passwords() {
        // Round-trip: formato PHC Argon2id (defaults OWASP); errada => false.
        let out = run_src("let h = hash_password(\"hunter2\"); echo h; echo verify_password(\"hunter2\", h); echo verify_password(\"errada\", h);")
            .expect("ok");
        let mut lines = out.lines();
        let hash = lines.next().expect("tem hash");
        assert!(
            hash.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"),
            "formato inesperado: {hash}"
        );
        assert_eq!(lines.collect::<Vec<_>>().join("\n"), "true\nfalse");
        // Vetores independentes: ferramenta online ($2a$) e Python ($2b$).
        assert_eq!(
            run_src("echo verify_password(\"password\", \"$2a$04$UuTkLRZZ6QofpDOlMz32MuuxEHA43WOemOYHPz6.SjsVsyO1tDU96\");")
                .expect("ok"),
            "true\n"
        );
        assert_eq!(
            run_src("echo verify_password(\"correctbatteryhorsestapler\", \"$2b$04$EGdrhbKUv8Oc9vGiXX0HQOxSg445d458Muh7DAHskb6QbtCvdxcie\");")
                .expect("ok"),
            "true\n"
        );
        // `$2y$` do PHP: mesmo algoritmo, marcador diferente.
        assert_eq!(
            run_src("echo verify_password(\"correctbatteryhorsestapler\", \"$2y$04$EGdrhbKUv8Oc9vGiXX0HQOxSg445d458Muh7DAHskb6QbtCvdxcie\");")
                .expect("ok"),
            "true\n"
        );
        // Erros: hash malformado, algoritmo desconhecido, teto anti-DoS.
        assert!(run_err("echo verify_password(\"x\", \"nao-e-hash\");").contains("nao suportado"));
        assert!(
            run_err("echo verify_password(\"x\", \"$scrypt$n=1$r=8$p=1$c2FsdA$aGFzaA\");")
                .contains("nao suportado")
        );
        assert!(run_err("echo verify_password(\"x\", \"$argon2id$v=19$m=100000,t=1,p=1$AAAAAAAAAAAAAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\");")
            .contains("acima do teto"));
        assert!(run_err("echo hash_password(1);").contains("string"));
        assert!(run_err("echo verify_password(1, \"x\");").contains("strings"));
    }

    #[test]
    fn runs_regex_and_hashing_builtins() {
        // regex_match: encontra / nao encontra; padrao invalido = erro claro.
        assert_eq!(
            run_src("echo regex_match(\"abc123\", \"[a-z]+[0-9]+\");").expect("ok"),
            "true\n"
        );
        assert_eq!(
            run_src("echo regex_match(\"abc\", \"^[0-9]+$\");").expect("ok"),
            "false\n"
        );
        assert!(run_err("echo regex_match(\"abc\", \"([a-z]\");").contains("regex invalida"));
        assert!(run_err("echo regex_match(\"abc\", \"(?=a)b\");").contains("regex invalida"));
        // regex_replace com grupos ($1) e regex_split.
        assert_eq!(
            run_src(
                "echo regex_replace(\"2026-11-16\", \"([0-9]+)-([0-9]+)-([0-9]+)\", \"$3/$2/$1\");"
            )
            .expect("ok"),
            "16/11/2026\n"
        );
        assert_eq!(
            run_src("echo regex_split(\"a,b,c\", \",\");").expect("ok"),
            "[a, b, c]\n"
        );
        // sha256: vetor canonico ("abc").
        assert_eq!(
            run_src("echo sha256(\"abc\");").expect("ok"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad\n"
        );
        // hmac: round-trip + verificacao em tempo constante.
        assert_eq!(
            run_src("let t = hmac_sha256(\"chave\", \"msg\"); echo hmac_verify(\"chave\", \"msg\", t); echo hmac_verify(\"outra\", \"msg\", t); echo hmac_verify(\"chave\", \"msg\", \"00\");").expect("ok"),
            "true\nfalse\nfalse\n"
        );
        // Tipos: exige strings.
        assert!(run_err("echo regex_match(1, \"a\");").contains("strings"));
        assert!(run_err("echo sha256(1);").contains("string"));
        assert!(run_err("echo hmac_sha256(\"k\", 1);").contains("strings"));
        assert!(run_err("echo hmac_verify(\"k\", \"m\", \"zz\");").contains("hexadecimal"));
    }

    #[test]
    fn runs_match_expression() {
        // Valor retornado; primeiro match vence; sem fallthrough.
        assert_eq!(
            run_src("echo match 2 { 1 => \"a\", 2 => \"b\", default => \"d\" };").expect("ok"),
            "b\n"
        );
        // Estrito: `1` casa com `1.0`, mas `"1"` e `true` nao casam com `1`.
        assert_eq!(
            run_src("echo match 1 { 1.0 => \"num\", default => \"outro\" };").expect("ok"),
            "num\n"
        );
        assert_eq!(
            run_src("echo match \"1\" { 1 => \"int\", default => \"outro\" };").expect("ok"),
            "outro\n"
        );
        assert_eq!(
            run_src("echo match true { 1 => \"um\", default => \"outro\" };").expect("ok"),
            "outro\n"
        );
        // `default` no meio casa por ordem (sem fallthrough no `match`:
        // casou, terminou — como no PHP).
        assert_eq!(
            run_src("echo match 2 { 1 => \"a\", default => \"d\", 2 => \"b\" };").expect("ok"),
            "d\n"
        );
        // Como expressao: em `let`, com chamada direta e aninhado.
        assert_eq!(
            run_src("let x = match 1 { 1 => 40 + 2 }; echo x;").expect("ok"),
            "42\n"
        );
        assert_eq!(
            run_src("echo (match 2 { 1 => \"a\", 2 => \"b\" });").expect("ok"),
            "b\n"
        );
        // Sem match e sem `default`: erro de runtime (capturavel).
        assert_eq!(
            run_src("try { echo match 99 { 1 => \"a\" }; } catch e { echo e; }").expect("ok"),
            "match sem braco correspondente\n"
        );
    }

    #[test]
    fn runs_elvis_arrow_forc_and_break_levels() {
        // Elvis: `null` aciona; valor passa; avalia 1x.
        assert_eq!(
            run_src(
                "let n = 0; fn prox() { n = n + 1; return null; } echo prox() ?: \"d\"; echo n;"
            )
            .expect("ok"),
            "d\n1\n"
        );
        assert_eq!(run_src("echo 5 ?: \"d\";").expect("ok"), "5\n");
        // Arrow: valor, captura e chamada direta.
        assert_eq!(
            run_src("let k = 10; echo map([1], fn (x) => x + k); echo (fn (a) => a * 2)(21);")
                .expect("ok"),
            "[11]\n42\n"
        );
        // For-C: classico, condicao ausente com `break`, passo roda no `continue`.
        assert_eq!(
            run_src("for let i = 0; i < 3; i = i + 1 { echo i; }").expect("ok"),
            "0\n1\n2\n"
        );
        assert_eq!(
            run_src("for ;; { echo \"uma\"; break; }").expect("ok"),
            "uma\n"
        );
        assert_eq!(
            run_src("for let j = 0; j < 3; j = j + 1 { if j == 1 { continue; } echo j; }")
                .expect("ok"),
            "0\n2\n"
        );
        // `for` com init `let` nao vaza.
        assert!(
            run_err("for let i = 0; i < 1; i = i + 1 { echo i; } echo i;").contains("nao definida")
        );
        // `break 2` sai do `switch` e do laco; `continue 2` pula o `switch`.
        assert_eq!(
            run_src("let i = 0; while i < 3 { i = i + 1; switch i { case 1 { echo \"um\"; break 2; } default { echo i; } } echo \"depois\"; } echo \"fim\";")
                .expect("ok"),
            "um\nfim\n"
        );
        assert_eq!(
            run_src("for let j = 0; j < 3; j = j + 1 { switch j { case 1 { continue 2; } default { echo j; } } echo \"x\"; }")
                .expect("ok"),
            "0\nx\n2\nx\n"
        );
        // Niveis alem do aninhamento: erro de compilacao.
        let program = parse_source("while true { break 2; }").expect("parse deve passar");
        let err = rhp_bytecode::compile(&program).expect_err("deve falhar");
        assert!(
            err.message.contains("alem do aninhamento"),
            "obteve: {}",
            err.message
        );
        let program =
            parse_source("switch 1 { case 1 { continue 2; } }").expect("parse deve passar");
        let err = rhp_bytecode::compile(&program).expect_err("deve falhar");
        assert!(
            err.message.contains("alem do aninhamento"),
            "obteve: {}",
            err.message
        );
    }

    #[test]
    fn runs_switch_statement() {
        // Match + break (sem fallthrough).
        assert_eq!(
            run_src("switch 2 { case 1 { echo \"a\"; } case 2 { echo \"b\"; break; } default { echo \"d\"; } }")
                .expect("ok"),
            "b\n"
        );
        // Fallthrough ate o fim quando nao ha `break`.
        assert_eq!(
            run_src("switch 1 { case 1 { echo \"a\"; } case 2 { echo \"b\"; } default { echo \"d\"; } }")
                .expect("ok"),
            "a\nb\nd\n"
        );
        // `default` no meio: casa por ordem, fallthrough continua depois.
        assert_eq!(
            run_src("switch 2 { case 1 { echo \"a\"; } default { echo \"d\"; } case 2 { echo \"b\"; } }")
                .expect("ok"),
            "d\nb\n"
        );
        // Sem match e sem default: nada executa.
        assert_eq!(
            run_src("switch 99 { case 1 { echo \"x\"; } }").expect("ok"),
            ""
        );
        // So default.
        assert_eq!(
            run_src("switch 99 { default { echo \"d\"; } }").expect("ok"),
            "d\n"
        );
        // Vazio.
        assert_eq!(run_src("switch 1 { }").expect("ok"), "");
        // Comparacao segue o `==` do RHP (int/float com promocao; resto
        // mesmo tipo): `1` casa com `1.0`, mas `true` nao casa com `1`.
        assert_eq!(
            run_src("switch 1 { case 1.0 { echo \"num\"; } }").expect("ok"),
            "num\n"
        );
        assert_eq!(
            run_src("switch true { case 1 { echo \"x\"; } default { echo \"d\"; } }").expect("ok"),
            "d\n"
        );
        // Discriminante avaliado uma unica vez.
        assert_eq!(
            run_src("let n = 0; fn prox() { n = n + 1; return n; } switch prox() { case 1 { echo \"ok\"; } } echo n;")
                .expect("ok"),
            "ok\n1\n"
        );
        // `let` repetido em cases distintos nao colide (escopo por case).
        assert_eq!(
            run_src(
                "switch 2 { case 1 { let v = 1; echo v; } case 2 { let v = 2; echo v; break; } }"
            )
            .expect("ok"),
            "2\n"
        );
        // Aninhado + `break` so sai do interno.
        assert_eq!(
            run_src("switch 1 { case 1 { switch 2 { case 2 { echo \"dentro\"; break; } } echo \"fora\"; break; } }")
                .expect("ok"),
            "dentro\nfora\n"
        );
        // `switch` em laco: `continue` em `switch` equivale a `break`
        // dele (gotcha documentado do PHP); `break` sai do `switch`.
        // i=1: default+`f`; i=2: `continue` sai do `switch`+`f`;
        // i=3: default+`f` (nao ha `case 3`).
        assert_eq!(
            run_src("let i = 0; while i < 3 { i = i + 1; switch i { case 2 { continue; } default { echo i; } } echo \"f\"; }")
                .expect("ok"),
            "1\nf\nf\n3\nf\n"
        );
        // Laco em case: `break` sai do laco (mais interno).
        assert_eq!(
            run_src("switch 1 { case 1 { let j = 0; while j < 5 { j = j + 1; if j == 2 { break; } } echo j; break; } }")
                .expect("ok"),
            "2\n"
        );
        // `return` em case sai da ativacao.
        assert_eq!(
            run_src("fn f(x) { switch x { case 1 { return \"um\"; } default { return \"outro\"; } } } echo f(1); echo f(9);")
                .expect("ok"),
            "um\noutro\n"
        );
    }

    #[test]
    fn reports_call_errors() {
        assert!(run_err("echo f();").contains("nao definida"));
        assert!(run_err("let f = 1; echo f();").contains("como funcao"));
        assert!(run_err("fn f(a) { return a; } echo f(1, 2);").contains("espera 1"));
    }

    #[test]
    fn closures_capture_by_value_and_call() {
        // Basico: atribuicao + chamada.
        assert_eq!(
            run_src("let f = fn (x) { return x + 1; }; echo f(41);").expect("ok"),
            "42\n"
        );
        // IIFE: chamada direta do literal.
        assert_eq!(
            run_src("echo (fn (x) { return x * 2; })(21);").expect("ok"),
            "42\n"
        );
        // Captura por valor: snapshot do escopo de ativacao. `n = 20`
        // altera o escopo, nao o clone que a closure carrega.
        assert_eq!(
            run_src("fn mk() { let n = 10; let f = fn () { return n; }; n = 20; return f; } let g = mk(); echo g();").expect("ok"),
            "10\n"
        );
        // Nivel superior: `let` vive em `globals` (mapa do modulo), fora
        // da cadeia de escopos — closure e `fn` nomeada enxergam o valor
        // atual, em paridade (nao congela comportamento existente).
        assert_eq!(
            run_src("let n = 10; let f = fn () { return n; }; n = 20; echo f();").expect("ok"),
            "20\n"
        );
        assert_eq!(
            run_src("let n = 10; fn g() { return n; } n = 20; echo g();").expect("ok"),
            "20\n"
        );
        // Parametro sombreia variavel capturada com o mesmo nome.
        assert_eq!(
            run_src("let x = 1; let f = fn (x) { return x; }; echo f(2);").expect("ok"),
            "2\n"
        );
        // Encadeada via variavel: `f(x)(y)`.
        assert_eq!(
            run_src("let f = fn (x) { return fn (y) { return x + y; }; }; echo f(40)(2);")
                .expect("ok"),
            "42\n"
        );
        // Nao-funcao em callee arbitrario continua erro claro.
        assert!(run_err("let f = 1; echo (f)(2);").contains("como funcao"));
    }

    #[test]
    fn reports_deep_recursion() {
        let src = "fn f(n) { if n == 0 { return 0; } return f(n - 1); } echo f(600);";
        assert!(run_err(src).contains("limite de chamadas"));
    }

    #[test]
    fn rejects_invalid_constant() {
        let chunk = Chunk {
            constants: vec![],
            names: vec![],
            funcs: vec![],
            code: vec![Op::Constant(5), Op::Halt],
        };
        let err = Vm::new().run_to_string(&chunk).expect_err("deve falhar");
        assert!(err.message.contains("bytecode invalido"));
        assert!(matches!(
            chunk.validate().expect_err("validate deve falhar"),
            CompileError { .. }
        ));
    }

    #[test]
    fn rejects_print_with_empty_stack() {
        let chunk = Chunk {
            constants: vec![Value::Str("x".to_string())],
            names: vec![],
            funcs: vec![],
            code: vec![Op::Print, Op::Halt],
        };
        // Desde a analise estatica de pilha (NG-202), chunks artesanais
        // assim sao rejeitados ja na validacao, antes de executar.
        let err = Vm::new().run_to_string(&chunk).expect_err("deve falhar");
        assert!(err.message.contains("pilha"));
    }

    #[test]
    fn rejects_chunk_without_halt() {
        let chunk = Chunk {
            constants: vec![Value::Str("x".to_string())],
            names: vec![],
            funcs: vec![],
            code: vec![Op::Constant(0), Op::Print],
        };
        let err = Vm::new().run_to_string(&chunk).expect_err("deve falhar");
        assert!(err.message.contains("bytecode invalido"));
    }

    #[test]
    fn builds_and_indexes_arrays() {
        assert_eq!(run_src("echo [1, 2, 3][1];").expect("ok"), "2\n");
        assert_eq!(run_src("echo [];\necho [1];").expect("ok"), "[]\n[1]\n");
        assert_eq!(
            run_src("let a = [1, 2]; a[0] = 9; echo a;").expect("ok"),
            "[9, 2]\n"
        );
        assert_eq!(
            run_src("let a = [1]; a = push(a, 2); echo a; echo len(a);").expect("ok"),
            "[1, 2]\n2\n"
        );
        assert!(run_err("echo [1][5];").contains("fora do intervalo"));
        assert!(run_err("echo [1][-1];").contains("fora do intervalo"));
        assert!(run_err("let a = 1; a[0] = 2;").contains("nao suportada"));
    }

    #[test]
    fn builds_and_indexes_maps() {
        assert_eq!(
            run_src("echo {nome: \"rhp\", v: 3}[\"v\"];").expect("ok"),
            "3\n"
        );
        assert_eq!(
            run_src("let m = {}; m[\"k\"] = 42; echo m;").expect("ok"),
            "{k: 42}\n"
        );
        assert_eq!(run_src("echo keys({b: 1, a: 2});").expect("ok"), "[a, b]\n");
        assert!(run_err("echo {}[\"x\"];").contains("chave inexistente"));
    }

    #[test]
    fn runs_builtins() {
        assert_eq!(run_src("echo len(\"ola\");").expect("ok"), "3\n");
        assert_eq!(run_src("echo len({a: 1});").expect("ok"), "1\n");
        assert_eq!(run_src("echo pop([7, 8]);").expect("ok"), "8\n");
        assert_eq!(run_src("echo str(42);").expect("ok"), "42\n");
        assert_eq!(run_src("echo int(\"42\");").expect("ok"), "42\n");
        assert_eq!(run_src("echo float(2);").expect("ok"), "2\n");
        assert_eq!(run_src("echo type([1]);").expect("ok"), "array\n");
        assert_eq!(run_src("echo type({});").expect("ok"), "mapa\n");
        assert_eq!(run_src("assert(1 == 1); echo \"ok\";").expect("ok"), "ok\n");
        assert!(run_err("echo int(\"abc\");").contains("nao converte"));
        assert!(run_err("echo pop([]);").contains("vazio"));
        assert!(run_err("assert(false);").contains("assercao"));
        assert!(run_err("echo len(1);").contains("nao suporta"));
    }

    #[test]
    fn compares_compounds() {
        assert_eq!(run_src("echo [1, 2] == [1, 2];").expect("ok"), "true\n");
        assert_eq!(run_src("echo {a: 1} != {};").expect("ok"), "true\n");
        assert!(run_err("echo [1] < [2];").contains("incompativeis"));
    }

    #[test]
    fn rejects_int_overflow_boundary() {
        // 2^63 nao cabe em i64: deve errar, nao saturar.
        assert!(run_err("echo int(9223372036854775808.0);").contains("nao converte"));
        assert_eq!(run_src("echo int(42.0);").expect("ok"), "42\n");
    }

    #[test]
    fn pins_duplicate_map_keys_first_wins() {
        assert_eq!(run_src("echo {a: 1, a: 2};").expect("ok"), "{a: 1}\n");
    }

    #[test]
    fn pins_let_shadowing_param_is_error() {
        assert!(run_err("fn f(a) { let a = 1; } echo f(0);").contains("ja declarada"));
    }

    #[test]
    fn pins_builtin_name_precedence_in_call_position() {
        assert_eq!(
            run_src("let len = 5; echo len; echo len([1, 2]);").expect("ok"),
            "5\n2\n"
        );
    }

    #[test]
    fn pins_nan_semantics() {
        assert_eq!(
            run_src("echo float(\"nan\") == float(\"nan\");").expect("ok"),
            "false\n"
        );
        assert!(run_err("echo float(\"nan\") < 1;").contains("incompativeis"));
    }

    /// Gerador pseudoaleatorio deterministico (LCG; sem dependencias externas).
    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self, bound: u64) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (self.0 >> 33) % bound
        }
    }

    /// Gera expressao de `+`/`-`/`*` sobre 0..=9 com valor de referencia.
    /// Faixa sem overflow possivel: profundidade 4 limita o modulo a
    /// ~1.8e15 (<< 2^63), seguro para a referencia em i64 em debug.
    fn gen_arith(rng: &mut Lcg, depth: u32) -> (String, i64) {
        if depth == 0 || rng.next(3) == 0 {
            let v = rng.next(10) as i64;
            return (v.to_string(), v);
        }
        let (l_src, l_val) = gen_arith(rng, depth - 1);
        let (r_src, r_val) = gen_arith(rng, depth - 1);
        match rng.next(3) {
            0 => (format!("({l_src} + {r_src})"), l_val + r_val),
            1 => (format!("({l_src} - {r_src})"), l_val - r_val),
            _ => (format!("({l_src} * {r_src})"), l_val * r_val),
        }
    }

    /// Gera comparacao `<`/`==` sobre aritmetica rasa, com referencia.
    fn gen_cmp(rng: &mut Lcg) -> (String, bool) {
        let (l_src, l_val) = gen_arith(rng, 2);
        let (r_src, r_val) = gen_arith(rng, 2);
        if rng.next(2) == 0 {
            (format!("({l_src} < {r_src})"), l_val < r_val)
        } else {
            (format!("({l_src} == {r_src})"), l_val == r_val)
        }
    }

    /// Gera concatenacao de fragmentos ASCII com comprimento de referencia.
    fn gen_concat(rng: &mut Lcg) -> (String, usize) {
        const FRAGS: [&str; 4] = ["a", "bc", "def", ""];
        let n = 1 + rng.next(4) as usize;
        let mut src = String::from("\"\"");
        let mut total = 0;
        for _ in 0..n {
            let f = FRAGS[rng.next(4) as usize];
            total += f.len();
            src = format!("({src} + \"{f}\")");
        }
        (src, total)
    }

    #[test]
    fn property_arithmetic_matches_reference() {
        let mut rng = Lcg(0x1234_5678_9ABC_DEF0);
        for _ in 0..1000 {
            let (src, expected) = gen_arith(&mut rng, 4);
            let got = run_src(&format!("echo {src};")).expect("programa gerado deve rodar");
            assert_eq!(got, format!("{expected}\n"), "fonte: {src}");
        }
    }

    /// Gera uma instrucao aleatoria (operandos as vezes fora da faixa, de
    /// proposito). `n_consts`/`n_names` = tamanho do pool; saltos miram
    /// `0..code_len+2` (podem cair fora).
    fn fuzz_op(rng: &mut Lcg, n_consts: u32, n_names: u32, code_len: u32) -> Op {
        match rng.next(53) {
            0 => Op::Constant(rng.next(n_consts as u64 + 2) as u32),
            1 => Op::LoadName(rng.next(n_names as u64 + 2) as u32),
            2 => Op::StoreName(rng.next(n_names as u64 + 2) as u32),
            3 => Op::DeclareName(rng.next(n_names as u64 + 2) as u32),
            4 => Op::DeclareStatic(rng.next(n_names as u64 + 2) as u32),
            5 => Op::DefineFunc(rng.next(2) as u32),
            6 => Op::MakeFunc(rng.next(2) as u32),
            7 => Op::Call(rng.next(3) as u32),
            8 => Op::CallMixed(rng.next(n_consts as u64 + 2) as u32),
            9 => Op::CallBuiltin(rhp_bytecode::BuiltinId(rng.next(70) as u32)),
            10 => Op::Jump(rng.next(code_len as u64 + 2) as u32),
            11 => Op::JumpIfFalse(rng.next(code_len as u64 + 2) as u32),
            12 => Op::JumpIfNotNull(rng.next(code_len as u64 + 2) as u32),
            13 => Op::TryBegin(rng.next(code_len as u64 + 2) as u32),
            14 => Op::BuildArray(rng.next(4) as u32),
            15 => Op::BuildMap(rng.next(4) as u32),
            16 => Op::StoreIndex(rng.next(n_names as u64 + 2) as u32),
            17 => Op::Print,
            18 => Op::Halt,
            19 => Op::Add,
            20 => Op::Sub,
            21 => Op::Mul,
            22 => Op::Div,
            23 => Op::Mod,
            24 => Op::Neg,
            25 => Op::Not,
            26 => Op::BitNot,
            27 => Op::BitAnd,
            28 => Op::BitOr,
            29 => Op::BitXor,
            30 => Op::Shl,
            31 => Op::Shr,
            32 => Op::Dup,
            33 => Op::Pop,
            34 => Op::Eq,
            35 => Op::StrictEq,
            36 => Op::Ne,
            37 => Op::Lt,
            38 => Op::Le,
            39 => Op::Gt,
            40 => Op::Ge,
            41 => Op::Index,
            42 => Op::IterList,
            43 => Op::TryEnd,
            44 => Op::Throw,
            45 => Op::Rethrow,
            46 => Op::Return,
            47 => Op::CheckType(match rng.next(8) {
                0 => TypeTag::Int,
                1 => TypeTag::Float,
                2 => TypeTag::String,
                3 => TypeTag::Bool,
                4 => TypeTag::Array,
                5 => TypeTag::Map,
                6 => TypeTag::Null,
                _ => TypeTag::Func,
            }),
            48 => Op::BuildClass(rng.next(n_names as u64 + 2) as u32, rng.next(3) as u32),
            49 => Op::NewMixed(rng.next(n_consts as u64 + 2) as u32),
            50 => Op::CallMethod(
                rng.next(n_names as u64 + 2) as u32,
                rng.next(n_consts as u64 + 2) as u32,
                rng.next(n_consts as u64 + 2) as u32,
            ),
            51 => Op::JumpIfTrue(rng.next(code_len as u64 + 2) as u32),
            52 => Op::Yield,
            _ => Op::EnterScope,
        }
    }

    /// Executa um chunk sob tetos estritos; nunca entra em panico (o
    /// resultado, Ok ou Err, e descartado de proposito).
    fn run_hostile(chunk: &Chunk) {
        let _ = chunk.validate();
        let mut vm = Vm::new();
        vm.set_limits(VmLimits {
            max_steps: Some(500),
            max_stack_depth: Some(16),
            max_collection_len: Some(32),
            ..Default::default()
        });
        let mut out = Vec::new();
        let _ = vm.run(chunk, &mut out);
    }

    #[test]
    fn fuzz_hostile_bytecode_never_panics() {
        // Etapa 3: chunks aleatorios + mutacoes de chunks validos, com seed
        // fixa e sem dependencias (GOVERNANCE §8.5/§9). Validador e VM so
        // retornam Ok/Err, nunca panico — mesmo com operandos fora da
        // faixa, saltos invalidos e `Halt` ausente.
        let pool = vec![
            Value::Int(0),
            Value::Int(1),
            Value::Str("x".to_string()),
            Value::Bool(true),
            Value::Null,
            Value::Array(vec![Value::Int(0)]),
        ];
        let names = vec!["a".to_string(), "b".to_string()];
        let mut rng = Lcg(0x0BAD_C0DE_F00D_C0DE);
        for _ in 0..8000 {
            let n = 1 + rng.next(12) as usize;
            let mut code = Vec::with_capacity(n + 1);
            for _ in 0..n {
                code.push(fuzz_op(
                    &mut rng,
                    pool.len() as u32,
                    names.len() as u32,
                    n as u32,
                ));
            }
            if rng.next(2) == 0 {
                code.push(Op::Halt);
            }
            run_hostile(&Chunk {
                constants: pool.clone(),
                names: names.clone(),
                funcs: vec![],
                code,
            });
        }
        // Mutacoes de programas validos (formas realistas).
        let seeds = [
            "echo 1;",
            "let a = [1, 2]; echo a[0];",
            "fn f(x) { return x + 1; } echo f(41);",
            "try { throw \"e\"; } catch e { echo e; }",
            "for x in [1, 2] { echo x; }",
            "echo match 1 { 1 => \"um\", default => \"outro\" };",
        ];
        for src in seeds {
            let base = compile_src(src);
            for _ in 0..100 {
                let mut chunk = base.clone();
                for _ in 0..(1 + rng.next(3)) {
                    let i = rng.next(chunk.code.len() as u64) as usize;
                    chunk.code[i] = fuzz_op(
                        &mut rng,
                        chunk.constants.len() as u32,
                        chunk.names.len() as u32,
                        chunk.code.len() as u32,
                    );
                }
                run_hostile(&chunk);
            }
        }
    }

    #[test]
    fn respects_stack_depth_across_return() {
        // Regressao do fuzz (Etapa 3): `Return` remove o frame e o idx
        // fica obsoleto; a checagem final de profundidade nao pode
        // entrar em panico com `max_stack_depth` configurado.
        let chunk = compile_src("fn f() { return 1; } echo f();");
        let mut vm = Vm::new();
        vm.set_limits(VmLimits {
            max_stack_depth: Some(16),
            ..Default::default()
        });
        let out = vm
            .run_to_string(&chunk)
            .expect("return com teto folgado passa");
        assert_eq!(out, "1\n");
    }

    #[test]
    fn property_comparison_matches_reference() {
        let mut rng = Lcg(0x0BAD_F00D_CAFE_1234);
        for _ in 0..500 {
            let (src, expected) = gen_cmp(&mut rng);
            let got = run_src(&format!("echo {src};")).expect("programa gerado deve rodar");
            assert_eq!(got, format!("{expected}\n"), "fonte: {src}");
        }
    }

    #[test]
    fn property_string_concat_length() {
        let mut rng = Lcg(0x5EED_1234_ABCD_5678);
        for _ in 0..300 {
            let (src, expected) = gen_concat(&mut rng);
            let got = run_src(&format!("echo len({src});")).expect("programa gerado deve rodar");
            assert_eq!(got, format!("{expected}\n"), "fonte: {src}");
        }
    }

    #[test]
    fn respects_step_limit() {
        let chunk = compile_src("let i = 0; while true { i = i + 1; }");
        let mut vm = Vm::new();
        vm.set_limits(VmLimits {
            max_steps: Some(1000),
            ..Default::default()
        });
        let err = vm.run_to_string(&chunk).expect_err("deve falhar");
        assert!(err.message.contains("limite de instrucoes"));
        // Sem limite, programa finito passa normalmente.
        let out = run_src("let i = 0; while i < 3 { i = i + 1; } echo i;").expect("ok");
        assert_eq!(out, "3\n");
    }

    #[test]
    fn error_carries_call_stack() {
        // Etapa 4: pilha da mais interna para a mais externa; vazia no
        // nivel superior; `message` intacta (vira valor em `try`/`catch`).
        let chunk = compile_src("fn f() { g(); } fn g() { x_indefinida; } f();");
        let err = Vm::new().run_to_string(&chunk).expect_err("deve falhar");
        assert_eq!(err.trace(), &["g".to_string(), "f".to_string()]);
        assert!(err.message.contains("nao definida"));
        let top = compile_src("x_indefinida;");
        let err = Vm::new().run_to_string(&top).expect_err("deve falhar");
        assert!(err.trace().is_empty());
        // Capturado por `try`: mensagem preservada, sem trace no valor.
        let out = run_src("fn f() { x_indefinida; } try { f(); } catch e { echo e[\"tipo\"]; }")
            .expect("catch passa");
        assert_eq!(out, "runtime\n");
    }

    #[test]
    fn run_continue_preserves_state_across_lines() {
        // Etapa 4 (REPL): globais/funcoes sobrevivem; erro nao corrompe.
        let mut vm = Vm::new();
        let mut out = Vec::new();
        let line = |vm: &mut Vm, out: &mut Vec<u8>, src: &str| {
            let chunk = compile_src(src);
            vm.run_continue(&chunk, out).map(|_| ())
        };
        line(&mut vm, &mut out, "let x = 40;").expect("linha 1 passa");
        line(&mut vm, &mut out, "fn soma(a) { return a + 2; }").expect("linha 2 passa");
        line(&mut vm, &mut out, "echo soma(x);").expect("linha 3 passa");
        assert_eq!(String::from_utf8(out.clone()).expect("utf8"), "42\n");
        out.clear();
        line(&mut vm, &mut out, "echo x_indefinida;").expect_err("linha com erro falha");
        line(&mut vm, &mut out, "echo x;").expect("sessao sobrevive ao erro");
        assert_eq!(String::from_utf8(out).expect("utf8"), "40\n");
    }

    #[test]
    fn checks_type_annotations() {
        // Etapa 5 (RFC 0016): 8 tipos aceitam; mismatch = erro capturavel.
        assert_eq!(
            run_src("let a: int = 1; let b: float = 2.5; let c: string = \"x\"; let d: bool = true; let e: array = [1]; let f: mapa = {k: 1}; let g: null = null; let h: funcao = fn () { }; echo a + b;").expect("ok"),
            "3.5\n"
        );
        let err = run_err("let x: int = \"a\";");
        assert!(err.contains("tipos incompativeis"), "obteve: {err}");
        assert!(err.contains("esperava int"), "obteve: {err}");
        assert!(err.contains("string"), "obteve: {err}");
        // Nulo implicito tambem checa.
        assert!(run_err("let x: int;").contains("esperava int"));
        // Parametros (incluipos apos default) e retorno.
        assert_eq!(
            run_src("fn soma(a: int, b: int = 10): int { return a + b; } echo soma(1);")
                .expect("ok"),
            "11\n"
        );
        assert!(run_err("fn f(a: int) { return a; } echo f(\"x\");").contains("esperava int"));
        assert!(run_err("fn f(): int { return \"x\"; } echo f();").contains("esperava int"));
        // Closure anotada; try/catch preserva a mensagem.
        assert_eq!(
            run_src("let d = fn (v: int): int => v * 2; echo d(21);").expect("ok"),
            "42\n"
        );
        assert_eq!(
            run_src(
                "try { let x: int = 1.5; } catch e { echo e[\"tipo\"]; echo e[\"mensagem\"]; }"
            )
            .expect("ok"),
            "runtime\ntipos incompativeis: esperava int, encontrou float\n"
        );
        // Sem anotacao: nada muda (nenhum CheckType emitido).
        assert_eq!(run_src("let x = \"a\"; echo x;").expect("ok"), "a\n");
    }

    #[test]
    fn respects_heap_byte_limit() {
        // ~10 strings de 100 bytes retidas: ~1 KB vivo > teto de 512.
        let src = "let a = []; let i = 0; while i < 10 { a = push(a, \"0123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789\"); i = i + 1; } echo len(a);";
        let chunk = compile_src(src);
        let mut vm = Vm::new();
        vm.set_limits(VmLimits {
            max_heap_bytes: Some(512),
            ..Default::default()
        });
        let err = vm.run_to_string(&chunk).expect_err("deve falhar");
        assert!(err.message.contains("heap excedeu o limite"));
        // Teto folgado passa; sem teto passa (compatibilidade).
        let mut vm_ok = Vm::new();
        vm_ok.set_limits(VmLimits {
            max_heap_bytes: Some(1 << 20),
            ..Default::default()
        });
        let out = vm_ok.run_to_string(&chunk).expect("teto folgado passa");
        assert_eq!(out, "10\n");
        let out = run_src(src).expect("sem teto passa");
        assert_eq!(out, "10\n");
        // Aloca-e-libera cabe em teto pequeno (varredura reseta o contador).
        let transiente = "let i = 0; while i < 50 { let t = \"0123456789\"; i = i + 1; } echo i;";
        let chunk_t = compile_src(transiente);
        let mut vm_t = Vm::new();
        vm_t.set_limits(VmLimits {
            max_heap_bytes: Some(2048),
            ..Default::default()
        });
        let out = vm_t.run_to_string(&chunk_t).expect("transiente passa");
        assert_eq!(out, "50\n");
    }

    #[test]
    fn classes_methods_and_this() {
        // Etapa 5 (RFC 0017): ciclo completo; mutacao via `c.m()` persiste
        // (write-back); metodo destacado nao persiste (documentado).
        let cls = "class Contador { fn novo() { this[\"n\"] = 0; } fn inc() { this[\"n\"] = this[\"n\"] + 1; return this[\"n\"]; } fn get() { return this[\"n\"]; } } ";
        assert_eq!(
            run_src(&format!(
                "{cls} let c = new Contador(); echo c.inc(); echo c.inc(); echo c.get();"
            ))
            .expect("ok"),
            "1\n2\n2\n"
        );
        assert_eq!(
            run_src(&format!(
                "{cls} let c = new Contador(); let f = c.inc; echo f(); echo c.get();"
            ))
            .expect("ok"),
            "1\n0\n"
        );
        // Erros: sem `novo`, metodo ausente, `new` em nao-classe, aridade.
        assert!(run_err("class A { } let a = new A();").contains("sem construtor"));
        assert!(
            run_err(&format!("{cls} let c = new Contador(); echo c.ausente;"))
                .contains("membro inexistente")
        );
        assert!(run_err("let x = 1; let y = new x();").contains("`new` exige classe"));
        assert!(run_err(&format!("{cls} let c = new Contador(1);")).contains("espera"));
        // `this` fora de metodo = variavel indefinida; construtor que
        // descarta `this` = erro.
        assert!(run_err("echo this;").contains("nao definida"));
        assert!(
            run_err("class B { fn novo() { this = 5; } } let b = new B();")
                .contains("preservar `this`")
        );
        // Metodo com named/spread + retorno anotado + `try`.
        assert_eq!(
            run_src("class S { fn novo(s) { this[\"s\"] = s; } fn rep(n) { return this[\"s\"]; } } let s = new S(s: \"ab\"); echo s.rep(...[1]);").expect("ok"),
            "ab\n"
        );
        assert_eq!(
            run_src("class T { fn novo(): int { return 1; } } let t = new T(); echo type(t);")
                .expect("ok"),
            "instancia\n"
        );
    }

    #[test]
    fn generators_yield_resume_and_finish() {
        // Etapa 5 (RFC 0018): chamada nao executa (lazy); `next` retoma;
        // `exhausted` marca o fim; valor de `return` e descartado.
        let src = "fn g() { echo \"ini\"; yield 1; echo \"mei\"; yield 2; echo \"fim\"; return 99; } let x = g(); echo \"criado\"; echo next(x); echo exhausted(x); echo next(x); echo exhausted(x); echo next(x);";
        assert_eq!(
            run_src(src).expect("ok"),
            "criado\nini\n1\nfalse\nmei\n2\nfalse\nfim\nnull\n"
        );
        assert!(run_err(
            "fn g() { yield 1; } let x = g(); echo next(x); echo next(x); echo next(x);"
        )
        .contains("esgotado"));
        // `yield` sem valor entrega `Null`; `yield null` nao confunde o fim.
        assert_eq!(run_src("fn g() { yield; yield null; } let x = g(); echo next(x) == null; echo next(x) == null; echo exhausted(x);").expect("ok"), "true\ntrue\nfalse\n");
    }

    #[test]
    fn generators_for_in_and_control_flow() {
        // `for x in gen`: break/continue, parada antecipada, vazio, reuso.
        assert_eq!(
            run_src("fn g() { yield 1; yield 2; yield 3; } for x in g() { if x == 2 { continue; } if x == 3 { break; } echo x; } echo \"ok\";").expect("ok"),
            "1\nok\n"
        );
        assert_eq!(
            run_src("fn v() { if false { yield 1; } } for x in v() { echo x; } echo \"fim\";")
                .expect("ok"),
            "fim\n"
        );
        assert_eq!(
            run_src("fn nats() { let i = 0; while true { yield i; i = i + 1; } } for x in nats() { if x > 2 { break; } echo x; }").expect("ok"),
            "0\n1\n2\n"
        );
        // Gerador esgotado em novo `for`: zero iteracoes (sem erro).
        assert_eq!(
            run_src("fn g() { yield 1; } let x = g(); echo next(x); for y in x { echo y; } echo \"fim\";").expect("ok"),
            "1\nfim\n"
        );
    }

    #[test]
    fn generators_reject_misuse() {
        // `new` em nao-classe, `yield` fora de funcao, `next`/`exhausted`
        // em nao-gerador, construtor/metodo gerador, callback gerador.
        assert!(run_err("fn f() { return 1; } echo next(f);").contains("exige gerador"));
        assert!(run_err("echo exhausted(1);").contains("exige gerador"));
        {
            let prog = parse_source("yield 1;").expect("parse deve passar");
            assert!(rhp_bytecode::compile(&prog)
                .expect_err("yield fora de funcao deve falhar")
                .message
                .contains("fora de funcao"));
        }
        assert!(
            run_err("class C { fn novo() { yield 1; } } let c = new C();")
                .contains("nao pode ser gerador")
        );
        assert!(run_err("fn g() { yield 1; } echo map([1], g);").contains("nao pode ser gerador"));
        // Erro dentro do gerador: `try` externo captura; resume abortado.
        assert_eq!(
            run_src("fn g() { yield 1; x_indefinida; } let x = g(); try { echo next(x); echo next(x); } catch e { echo e[\"tipo\"]; } try { echo next(x); } catch e { echo \"morto\"; }").expect("ok"),
            "1\nruntime\nmorto\n"
        );
        // Igualdade: gerador so e igual a si mesmo; instancias e classes
        // comparam por estrutura/nome.
        assert_eq!(
            run_src("fn g() { yield 1; } let a = g(); let b = g(); echo a == a; echo a == b;")
                .expect("ok"),
            "true\nfalse\n"
        );
        assert_eq!(
            run_src("class C { fn novo() { this[\"n\"] = 0; } } let c1 = new C(); let c2 = new C(); echo c1 == c2; c2[\"n\"] = 5; echo c1 == c2; echo C == C;").expect("ok"),
            "true\nfalse\ntrue\n"
        );
    }

    #[test]
    fn tasks_run_in_parallel_and_join() {
        // Etapa 5 (RFC 0019): spawn/join com threads; 2x sleep(200ms)
        // termina bem abaixo do serial (margem folgada anti-flake).
        // `run_src` nao define programa; aqui vai o pipeline completo.
        use std::sync::Arc;
        use std::time::Instant;
        fn run_with_program(src: &str) -> Result<String, VmError> {
            let program = Arc::new(parse_source(src).expect("parse deve passar"));
            let chunk = rhp_bytecode::compile(&program).expect("compile deve passar");
            let mut vm = Vm::new();
            vm.set_program(program);
            vm.run_to_string(&chunk)
        }
        fn run_err_with_program(src: &str) -> String {
            run_with_program(src).expect_err("deve falhar").message
        }
        let src = "fn dorme() { sleep(0.2); return \"ok\"; } fn agora() { return 7; } let a = async_spawn(\"dorme\"); let b = async_spawn(\"agora\"); echo async_join(b); echo async_join(a);";
        let start = Instant::now();
        assert_eq!(run_with_program(src).expect("ok"), "7\nok\n");
        assert!(
            start.elapsed().as_millis() < 350,
            "paralelo esperado (< 350ms para 200ms de sono)"
        );
        // Erro da tarefa vira erro do join; handle one-shot.
        assert!(run_err_with_program(
            "fn ruim() { x_indefinida; } let h = async_spawn(\"ruim\"); echo async_join(h);"
        )
        .contains("tarefa"));
        assert!(run_err_with_program("echo async_join(999);").contains("invalido"));
        assert!(run_err_with_program("fn f() { return 1; } let h = async_spawn(\"f\"); echo async_join(h); echo async_join(h);").contains("invalido"));
        // Validacao no spawn: inexistente, nao-funcao, gerador, captura.
        assert!(run_err_with_program("echo async_spawn(\"naoexiste\");").contains("inexistente"));
        assert!(run_err_with_program("let x = 1; echo async_spawn(x);").contains("exige funcao"));
        assert!(
            run_err_with_program("fn g() { yield 1; } echo async_spawn(\"g\");")
                .contains("gerador")
        );
        assert!(run_err_with_program("fn outer() { let y = 1; fn f() { return y; } return f; } let g = outer(); echo async_spawn(\"g\");").contains("captura"));
        // Isolamento: global do pai nao existe no worker.
        assert!(run_err_with_program(
            "let x = 1; fn f() { return x; } let h = async_spawn(\"f\"); echo async_join(h);"
        )
        .contains("nao definida"));
    }

    #[test]
    fn runs_stdlib_strings() {
        assert_eq!(
            run_src("echo contains(\"ola mundo\", \"mundo\");").expect("ok"),
            "true\n"
        );
        assert_eq!(
            run_src("echo starts_with(\"ola\", \"o\");").expect("ok"),
            "true\n"
        );
        assert_eq!(
            run_src("echo ends_with(\"ola\", \"a\");").expect("ok"),
            "true\n"
        );
        assert_eq!(
            run_src("echo split(\"a,b,c\", \",\");").expect("ok"),
            "[a, b, c]\n"
        );
        assert_eq!(
            run_src("echo join([\"a\", \"b\"], \"-\");").expect("ok"),
            "a-b\n"
        );
        assert_eq!(run_src("echo trim(\"  x  \");").expect("ok"), "x\n");
        assert_eq!(run_src("echo upper(\"ola\");").expect("ok"), "OLA\n");
        assert_eq!(run_src("echo lower(\"OLA\");").expect("ok"), "ola\n");
        assert_eq!(
            run_src("echo replace(\"aaa\", \"a\", \"b\");").expect("ok"),
            "bbb\n"
        );
        assert!(run_err("echo join([1], \",\");").contains("strings"));
    }

    #[test]
    fn runs_stdlib_math() {
        assert_eq!(run_src("echo abs(-3);").expect("ok"), "3\n");
        assert_eq!(run_src("echo abs(-2.5);").expect("ok"), "2.5\n");
        assert_eq!(run_src("echo min(3, 7);").expect("ok"), "3\n");
        assert_eq!(run_src("echo max(3, 7.5);").expect("ok"), "7.5\n");
        assert_eq!(run_src("echo sqrt(9);").expect("ok"), "3\n");
        assert_eq!(run_src("echo pow(2, 10);").expect("ok"), "1024\n");
        assert_eq!(run_src("echo floor(2.9);").expect("ok"), "2\n");
        assert_eq!(run_src("echo ceil(2.1);").expect("ok"), "3\n");
        assert!(run_err("echo sqrt(-1);").contains("negativo"));
        assert!(run_err("echo 1 + true;").contains("incompativeis"));
    }

    #[test]
    fn runs_stdlib_maps() {
        assert_eq!(
            run_src("echo values({b: 1, a: 2});").expect("ok"),
            "[2, 1]\n"
        );
        assert_eq!(
            run_src("echo entries({b: 1, a: 2});").expect("ok"),
            "[[a, 2], [b, 1]]\n"
        );
        assert_eq!(
            run_src("echo merge({a: 1}, {a: 2, b: 3});").expect("ok"),
            "{a: 2, b: 3}\n"
        );
        assert_eq!(run_src("echo has({a: 1}, \"a\");").expect("ok"), "true\n");
        assert_eq!(run_src("echo has({a: 1}, \"z\");").expect("ok"), "false\n");
    }

    #[test]
    fn runs_bitwise_ops() {
        assert_eq!(run_src("echo 6 & 3;").expect("ok"), "2\n");
        assert_eq!(run_src("echo 6 | 3;").expect("ok"), "7\n");
        assert_eq!(run_src("echo 6 ^ 3;").expect("ok"), "5\n");
        assert_eq!(run_src("echo ~5;").expect("ok"), "-6\n");
        assert_eq!(run_src("echo 1 << 4;").expect("ok"), "16\n");
        assert_eq!(run_src("echo 256 >> 4;").expect("ok"), "16\n");
        assert!(run_err("echo 1 & true;").contains("exige int"));
        assert!(run_err("echo 1 << -1;").contains("deslocamento"));
        assert!(run_err("echo 1 << 64;").contains("deslocamento"));
        assert!(run_err("echo ~1.5;").contains("exige int"));
    }

    #[test]
    fn runs_ternary_and_null_coalescing() {
        assert_eq!(run_src("echo true ? 1 : 2;").expect("ok"), "1\n");
        assert_eq!(run_src("echo false ? 1 : 2;").expect("ok"), "2\n");
        assert_eq!(run_src("echo null ?? \"x\";").expect("ok"), "x\n");
        assert_eq!(run_src("echo 0 ?? \"x\";").expect("ok"), "0\n");
        assert_eq!(run_src("echo false ?? \"x\";").expect("ok"), "false\n");
        // `??` avalia o lado esquerdo uma unica vez.
        assert_eq!(
            run_src("let i = 0; fn inc() { i = i + 1; return i; } echo inc() ?? 99; echo i;")
                .expect("ok"),
            "1\n1\n"
        );
    }

    #[test]
    fn runs_break_continue_and_const() {
        // Pares sao pulados pelo `continue` (inclusive o 6); o `break`
        // acontece no 7.
        assert_eq!(
            run_src("let i = 0; while i < 10 { i = i + 1; if i % 2 == 0 { continue; } if i > 5 { break; } } echo i;").expect("ok"),
            "7\n"
        );
        assert_eq!(run_src("const N = 40 + 2; echo N;").expect("ok"), "42\n");
        // Erro de compilacao: usa compile() direto (run_src so transporta erros de runtime).
        let program = rhp_parser::parse_source("const N = 1; N = 2;").expect("parse deve passar");
        assert!(rhp_bytecode::compile(&program)
            .expect_err("deve falhar")
            .message
            .contains("constante"));
        // `break`/`continue` fora de laco e erro de compilacao.
        let program = rhp_parser::parse_source("break;").expect("parse deve passar");
        assert!(rhp_bytecode::compile(&program)
            .expect_err("deve falhar")
            .message
            .contains("fora de laco"));
    }

    #[test]
    fn runs_time_builtins() {
        // `now()` = epoch Int nao-negativo e nao-decrescente.
        let t1 = run_src("echo type(now());").expect("ok");
        assert_eq!(t1, "int\n");
        // `date_str` com vetores conhecidos (UTC, bissextos, negativos).
        assert_eq!(
            run_src("echo date_str(0);").expect("ok"),
            "1970-01-01T00:00:00Z\n"
        );
        assert_eq!(
            run_src("echo date_str(-1);").expect("ok"),
            "1969-12-31T23:59:59Z\n"
        );
        assert_eq!(
            run_src("echo date_str(951782400);").expect("ok"),
            "2000-02-29T00:00:00Z\n"
        );
        assert_eq!(
            run_src("echo date_str(1763123303);").expect("ok"),
            "2025-11-14T12:28:23Z\n"
        );
        assert_eq!(
            run_src("echo date_str(253402300799);").expect("ok"),
            "9999-12-31T23:59:59Z\n"
        );
        assert_eq!(
            run_src("echo date_str(-2208988800);").expect("ok"),
            "1900-01-01T00:00:00Z\n"
        );
        // Tipos errados.
        let err = run_err("echo date_str(\"x\");");
        assert!(err.contains("epoch"), "obteve: {err}");
        let err = run_err("echo sleep(\"x\");");
        assert!(err.contains("segundos"), "obteve: {err}");
        let err = run_err("echo sleep(-1);");
        assert!(err.contains(">= 0"), "obteve: {err}");
        let err = run_err("echo sleep(3601);");
        assert!(err.contains("teto"), "obteve: {err}");
    }

    #[test]
    fn sleep_zero_returns_quickly() {
        use std::time::Instant;
        let start = Instant::now();
        assert_eq!(run_src("echo sleep(0);").expect("ok"), "null\n");
        assert!(start.elapsed().as_secs() < 5, "sleep(0) demorou demais");
    }

    #[test]
    fn runs_template_builtin() {
        assert_eq!(
            run_src("echo template(\"ola, {{nome}}!\", {\"nome\": \"mundo\"});").expect("ok"),
            "ola, mundo!\n"
        );
        // Escapa HTML (& primeiro); tipos viram texto.
        assert_eq!(
            run_src("echo template(\"{{a}}|{{b}}|{{c}}\", {\"a\": \"<b>&\\\"'\", \"b\": 42, \"c\": true});").expect("ok"),
            "&lt;b&gt;&amp;&quot;&#39;|42|true\n"
        );
        // Sem placeholder: passa direto; chave faltando/aberta = erro.
        assert_eq!(
            run_src("echo template(\"sem nada\", {});").expect("ok"),
            "sem nada\n"
        );
        let err = run_err("echo template(\"oi {{x}}\", {});");
        assert!(err.contains("ausente"), "obteve: {err}");
        let err = run_err("echo template(\"oi {{x\", {\"x\": 1});");
        assert!(err.contains("fechamento"), "obteve: {err}");
        let err = run_err("echo template(42, {});");
        assert!(err.contains("texto"), "obteve: {err}");
    }

    #[test]
    fn rejects_bad_pg_conninfo() {
        // Sem servidor: formato invalido, porta invalida, recusa rapida.
        for bad in [
            "echo db_open(\"pg:no-equals\");",
            "echo db_open(\"pg:port=abc\");",
            "echo db_open(\"pg:host=127.0.0.1 port=1 user=rhp dbname=rhp\");",
        ] {
            let err = run_err(bad);
            assert!(err.contains("`db`"), "obteve: {err} em {bad}");
        }
    }

    #[test]
    fn runs_sqlite_crud() {
        let dir = std::env::temp_dir().join(format!("rhp-db-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir temporario");
        let path = dir.join("t.db").to_string_lossy().into_owned();
        let setup = format!(
            "let h = db_open(\"{path}\");\necho db_exec(h, \"CREATE TABLE t (id, nome, preco)\", []);\necho db_exec(h, \"INSERT INTO t VALUES (?, ?, ?)\", [1, \"a\", 2.5]);\necho db_exec(h, \"INSERT INTO t VALUES (2, null, 3.0)\", []);\necho db_query(h, \"SELECT id, nome, preco FROM t ORDER BY id\", []);\necho db_close(h);\n"
        );
        let out = run_src(&setup).expect("ok");
        assert_eq!(
            out,
            "0\n1\n1\n[{id: 1, nome: a, preco: 2.5}, {id: 2, nome: null, preco: 3}]\nnull\n"
        );
        // Arquivo criado de verdade.
        assert!(std::fs::metadata(&path).is_ok(), "banco nao foi criado");
        // Tipos e erros.
        let err = run_err(&format!(
            "let h = db_open(\"{path}\"); echo db_exec(h, \"SELEC 1\", []);"
        ));
        assert!(err.contains("preparar"), "obteve: {err}");
        let err = run_err(&format!(
            "let h = db_open(\"{path}\"); echo db_exec(h, \"SELECT ?\", [1, 2]);"
        ));
        assert!(err.contains("parametro"), "obteve: {err}");
        let err = run_err(&format!("let h = db_open(\"{path}\"); echo db_query(h, \"SELECT * FROM t\", []); echo db_close(h); echo db_query(h, \"SELECT 1\", []);"));
        assert!(err.contains("fechado"), "obteve: {err}");
    }

    #[test]
    fn rejects_bad_fetch_opts() {
        // Nao-map, chave desconhecida, tipos errados, limites.
        for bad in [
            "echo http_fetch(\"http://x/\", 42);",
            "echo http_fetch(\"http://x/\", {x: 1});",
            "echo http_fetch(\"http://x/\", {seguir: 1});",
            "echo http_fetch(\"http://x/\", {max_redirecionamentos: 99});",
            "echo http_fetch(\"http://x/\", {max_redirecionamentos: -1});",
            "echo http_fetch(\"http://x/\", {timeout: 0});",
            "echo http_fetch(\"http://x/\", {timeout: 61});",
            "echo http_fetch(\"http://x/\", {timeout: \"x\"});",
            "echo http_fetch(\"http://x/\", {headers: [1]});",
            "echo http_fetch(\"http://x/\", {headers: {\"X\": 1}});",
            "echo http_fetch(\"http://x/\", {metodo: \"POST\"});",
            "echo http_fetch(42, {});",
        ] {
            let err = run_err(bad);
            assert!(err.contains("http_fetch"), "obteve: {err} em {bad}");
        }
        // `https://` aceito como esquema (falha so na conexao, nao no parse).
        let err = run_err("echo http_fetch(\"https://127.0.0.1:1/\", {});");
        assert!(err.contains("conectar"), "obteve: {err}");
        // `http_get` estrito nao mudou (redirect = erro `retornou status`).
    }

    #[test]
    fn while_body_scope_does_not_leak_on_break() {
        // Regressao: `let` no corpo do `while` + `break` vazava para fora.
        let err = run_err("while true { let vazou = 1; break; } echo vazou;");
        assert!(err.contains("nao definida"), "obteve: {err}");
        // E o caminho normal continua fechando o escopo a cada iteracao.
        let err = run_err("let i = 0; while i < 2 { let tmp = i; i = i + 1; } echo tmp;");
        assert!(err.contains("nao definida"), "obteve: {err}");
    }

    #[test]
    fn runs_throw_catch_in_same_function() {
        assert_eq!(
            run_src("try { throw \"boom\"; echo \"nunca\"; } catch e { echo e; }").expect("ok"),
            "boom\n"
        );
        // `catch` sem variavel descarta; corpo vazio consome o erro.
        assert_eq!(
            run_src("try { throw 1; } catch { echo \"pegou\"; }").expect("ok"),
            "pegou\n"
        );
        assert_eq!(
            run_src("try { throw 1; } catch e { } echo \"seguiu\";").expect("ok"),
            "seguiu\n"
        );
        // Erro interno vira mapa `{tipo, mensagem}`.
        assert_eq!(
            run_src("try { echo 1 / 0; } catch e { echo e[\"mensagem\"]; echo e[\"tipo\"]; }")
                .expect("ok"),
            "divisao por zero\nruntime\n"
        );
        // Erro de builtin e capturavel.
        assert_eq!(
            run_src("try { echo json_parse(\"{oops}\"); } catch e { echo \"json falhou\"; }")
                .expect("ok"),
            "json falhou\n"
        );
    }

    #[test]
    fn runs_exception_across_frames() {
        // Propaga por 2 frames ate o handler.
        assert_eq!(
            run_src("fn dentro() { throw \"fundo\"; } fn meio() { dentro(); } try { meio(); } catch e { echo e; }").expect("ok"),
            "fundo\n"
        );
        // `throw` dentro de `catch` propaga ao handler externo.
        assert_eq!(
            run_src("try { try { throw \"a\"; } catch e { throw \"b\"; } } catch e { echo e; }")
                .expect("ok"),
            "b\n"
        );
        // Sem handler em nenhum frame: falha como antes (mensagem intacta).
        let err = run_err("echo 1 / 0;");
        assert!(err.contains("divisao por zero"), "obteve: {err}");
        let err = run_err("fn f() { echo 1 / 0; } f();");
        assert!(err.contains("divisao por zero"), "obteve: {err}");
        let err = run_err("throw \"boom\";");
        assert!(err.contains("excecao nao capturada: boom"), "obteve: {err}");
    }

    #[test]
    fn runs_return_inside_try_catches_operand_error() {
        // Caso 1 — retorno de chamada invalida.
        assert_eq!(
            run_src("fn test() { try { return unknown_fn(); } catch e { return \"capturado\"; } } echo test();").expect("ok"),
            "capturado\n"
        );
        // Caso 2 — JSON invalido no operando do `return`.
        assert_eq!(
            run_src("fn test(s) { try { return json_parse(s); } catch e { return \"json-capturado\"; } } echo test(\"invalido\");").expect("ok"),
            "json-capturado\n"
        );
        // Caso 3 — retorno normal nao aciona o `catch`.
        assert_eq!(
            run_src("fn test() { try { return 123; } catch e { return 999; } } echo test();")
                .expect("ok"),
            "123\n"
        );
        // Caso 4 — erro em `let` seguido de `return` (ja funcionava; trava).
        assert_eq!(
            run_src("fn test() { try { let x = json_parse(\"invalido\"); return x; } catch e { return \"capturado\"; } } echo test();").expect("ok"),
            "capturado\n"
        );
        // Caso 5 — erro em funcao aninhada chamada no operando.
        assert_eq!(
            run_src("fn dentro() { throw \"fundo\"; } fn test() { try { return dentro(); } catch e { return \"pegou:\" + e; } } echo test();").expect("ok"),
            "pegou:fundo\n"
        );
        // Caso 6 — `try` aninhado: handler mais interno vence.
        assert_eq!(
            run_src("fn test() { try { try { return json_parse(\"x\"); } catch e { return \"interno\"; } } catch e { return \"externo\"; } } echo test();").expect("ok"),
            "interno\n"
        );
        // Caso 7 — `return` com operando que falha dentro de estruturas.
        assert_eq!(
            run_src("fn test(x) { if x { try { return json_parse(\"!\"); } catch e { return \"no-if\"; } } return \"fora\"; } echo test(true);").expect("ok"),
            "no-if\n"
        );
        assert_eq!(
            run_src("fn f(n) { if n <= 0 { return 0; } try { return f(n - 1) + json_parse(\"!\"); } catch e { return -1; } } echo f(3);").expect("ok"),
            "-1\n"
        );
        assert_eq!(
            run_src("fn test() { let i = 0; while i < 3 { i = i + 1; try { return json_parse(\"!\"); } catch e { return \"no-while\"; } } return \"fim\"; } echo test();").expect("ok"),
            "no-while\n"
        );
        // Operando avaliado ANTES do `finally` (semantica padrao).
        assert_eq!(
            run_src("let x = 1; fn f() { try { return x; } finally { x = 2; } } echo f(); echo x;")
                .expect("ok"),
            "1\n2\n"
        );
        // Caso 8 — sem handler, o erro do operando ainda propaga.
        let err = run_err("fn test() { return json_parse(\"invalido\"); } echo test();");
        assert!(err.contains("json_parse"), "obteve: {err}");
        let err = run_err(
            "fn test() { try { return json_parse(\"x\"); } catch e { throw e; } } echo test();",
        );
        assert!(err.contains("json_parse"), "obteve: {err}");
    }

    #[test]
    fn runs_finally_on_all_paths() {
        // Caminho normal.
        assert_eq!(
            run_src("try { echo \"corpo\"; } finally { echo \"fim\"; }").expect("ok"),
            "corpo\nfim\n"
        );
        // Caminho do `catch`.
        assert_eq!(
            run_src("try { throw 1; } catch e { echo \"pegou\"; } finally { echo \"fim\"; }")
                .expect("ok"),
            "pegou\nfim\n"
        );
        // Propagacao: `finally` roda e o erro segue.
        let err = run_err("try { echo 1 / 0; } finally { echo \"fim\"; }");
        assert!(err.contains("divisao por zero"), "obteve: {err}");
        // `return` dentro de `try` roda `finally` antes de sair.
        assert_eq!(
            run_src("fn f() { try { return 7; } finally { echo \"fim\"; } } echo f();")
                .expect("ok"),
            "fim\n7\n"
        );
        // `return` dentro de `finally` substitui o fluxo (valor e erro).
        assert_eq!(
            run_src("fn f() { try { return 1; } finally { return 2; } } echo f();").expect("ok"),
            "2\n"
        );
        // `break`/`continue` cruzando o `try` rodam `finally`.
        assert_eq!(
            run_src("while true { try { break; } finally { echo \"fim\"; } } echo \"fora\";")
                .expect("ok"),
            "fim\nfora\n"
        );
        assert_eq!(
            run_src(
                "let i = 0; while i < 2 { i = i + 1; try { continue; } finally { echo \"fim\"; } }"
            )
            .expect("ok"),
            "fim\nfim\n"
        );
    }

    #[test]
    fn exit_skips_finally() {
        // `exit` e abrupto: `finally` nao roda; stdout anterior preservado.
        let chunk = compile_src("try { echo \"antes\"; exit(3); } finally { echo \"fim\"; }");
        let mut vm = Vm::new();
        let mut out = Vec::new();
        let err = vm.run(&chunk, &mut out).expect_err("exit deve interromper");
        assert_eq!(err.exit_code(), Some(3));
        assert_eq!(out, b"antes\n");
    }

    #[test]
    fn try_scopes_do_not_leak() {
        // `let` no corpo/catch nao vaza; erro no `try` nao deixa sujeira.
        let err = run_err("try { let x = 1; throw 2; } catch e { } echo x;");
        assert!(err.contains("nao definida"), "obteve: {err}");
        let err = run_err("try { throw 1; } catch y { } echo y;");
        assert!(err.contains("nao definida"), "obteve: {err}");
    }

    #[test]
    fn runs_for_over_array_and_map() {
        assert_eq!(
            run_src("for x in [3, 1, 2] { echo x; }").expect("ok"),
            "3\n1\n2\n"
        );
        // Mapa: itera chaves ordenadas (mesma ordem de `keys`).
        assert_eq!(
            run_src("for k in {b: 1, a: 2} { echo k; }").expect("ok"),
            "a\nb\n"
        );
        // Vazio: corpo nunca executa.
        assert_eq!(
            run_src("for x in [] { echo x; } echo \"fim\";").expect("ok"),
            "fim\n"
        );
        assert_eq!(
            run_src("for k in {} { echo k; } echo \"fim\";").expect("ok"),
            "fim\n"
        );
        // Fora do laco, a variavel nao existe.
        let err = run_err("for x in [1] { } echo x;");
        assert!(err.contains("nao definida"), "obteve: {err}");
        // Nao iteravel e erro com tipo.
        let err = run_err("for x in 42 { echo x; }");
        assert!(err.contains("array, mapa ou gerador"), "obteve: {err}");
        let err = run_err("for x in \"ab\" { echo x; }");
        assert!(err.contains("array, mapa ou gerador"), "obteve: {err}");
    }

    #[test]
    fn runs_for_with_break_continue_and_nesting() {
        assert_eq!(
            run_src(
                "for i in [1, 2, 3, 4] { if i % 2 == 0 { continue; } if i > 2 { break; } echo i; }"
            )
            .expect("ok"),
            "1\n"
        );
        // `continue` pula para a proxima iteracao (nao trava no incremento).
        assert_eq!(
            run_src("let s = 0; for i in [1, 2, 3] { if i == 2 { continue; } s = s + i; } echo s;")
                .expect("ok"),
            "4\n"
        );
        // Aninhado com soma de pares.
        assert_eq!(
            run_src("let n = 0; for a in [1, 2] { for b in [10, 20] { n = n + a + b; } } echo n;")
                .expect("ok"),
            "66\n"
        );
        // Dentro de funcao e com `while` externo.
        assert_eq!(
            run_src("fn soma(xs) { let t = 0; for x in xs { t = t + x; } return t; } echo soma([1, 2, 3]);").expect("ok"),
            "6\n"
        );
        // Iteravel avaliado uma vez (mutar a origem nao afeta a iteracao).
        assert_eq!(
            run_src("let a = [1, 2]; for x in a { a = push(a, 9); echo x; }").expect("ok"),
            "1\n2\n"
        );
    }

    #[test]
    fn runs_sort_builtin() {
        assert_eq!(run_src("echo sort([3, 1, 2]);").expect("ok"), "[1, 2, 3]\n");
        assert_eq!(
            run_src("echo sort([2.5, 1, 2]);").expect("ok"),
            "[1, 2, 2.5]\n"
        );
        assert_eq!(
            run_src("echo sort([\"b\", \"a\", \"c\"]);").expect("ok"),
            "[a, b, c]\n"
        );
        assert_eq!(
            run_src("echo sort([true, false, true]);").expect("ok"),
            "[false, true, true]\n"
        );
        assert_eq!(run_src("echo sort([]);").expect("ok"), "[]\n");
        // Devolve copia: o original nao muda.
        assert_eq!(
            run_src("let a = [2, 1]; let b = sort(a); echo a; echo b;").expect("ok"),
            "[2, 1]\n[1, 2]\n"
        );
        // Heterogeneo e erro; nao-array e erro.
        let err = run_err("echo sort([1, \"a\"]);");
        assert!(err.contains("homogeneo"), "obteve: {err}");
        let err = run_err("echo sort({a: 1});");
        assert!(err.contains("exige array"), "obteve: {err}");
    }

    #[test]
    fn reports_exit_code_and_unwinds() {
        let chunk = compile_src("echo 1; exit(3); echo 2;");
        let mut vm = Vm::new();
        let mut out = Vec::new();
        let err = vm.run(&chunk, &mut out).expect_err("exit deve interromper");
        assert_eq!(err.exit_code(), Some(3));
        assert_eq!(out, b"1\n");
        // `exit(0)` tambem e pedido de saida (codigo 0).
        let chunk = compile_src("exit(0);");
        let mut vm = Vm::new();
        let mut out = Vec::new();
        let err = vm.run(&chunk, &mut out).expect_err("exit deve interromper");
        assert_eq!(err.exit_code(), Some(0));
        // Desenrola chamadas: sai de dentro de funcao.
        let chunk = compile_src("fn f() { exit(7); } f(); echo \"nunca\";");
        let mut vm = Vm::new();
        let mut out = Vec::new();
        let err = vm.run(&chunk, &mut out).expect_err("exit deve interromper");
        assert_eq!(err.exit_code(), Some(7));
        assert!(out.is_empty());
        // Codigo fora de 0-255 e erro comum (sem codigo de saida).
        let err = run_err("exit(256);");
        assert!(err.contains("0 e 255"), "obteve: {err}");
        let no_code = compile_src("exit(256);");
        let mut vm = Vm::new();
        let mut out = Vec::new();
        let err = vm.run(&no_code, &mut out).expect_err("deve falhar");
        assert_eq!(err.exit_code(), None);
    }

    #[test]
    fn runs_json_roundtrip() {
        // Tipos basicos.
        assert_eq!(run_src("echo json_parse(\"null\");").expect("ok"), "null\n");
        assert_eq!(run_src("echo json_parse(\"true\");").expect("ok"), "true\n");
        assert_eq!(run_src("echo json_parse(\"42\");").expect("ok"), "42\n");
        assert_eq!(run_src("echo json_parse(\"4.5\");").expect("ok"), "4.5\n");
        assert_eq!(
            run_src("echo json_parse(\"\\\"oi\\\"\");").expect("ok"),
            "oi\n"
        );
        assert_eq!(
            run_src("echo json_parse(\"[1, true]\");").expect("ok"),
            "[1, true]\n"
        );
        assert_eq!(
            run_src("echo json_parse(\"{\\\"b\\\": 1, \\\"a\\\": 2}\");").expect("ok"),
            "{a: 2, b: 1}\n"
        );
        // Serializacao canonica (chaves ordenadas, sem espacos).
        assert_eq!(
            run_src("echo json_stringify({b: [1, 2], a: \"x\"});").expect("ok"),
            "{\"a\":\"x\",\"b\":[1,2]}\n"
        );
        assert_eq!(run_src("echo json_stringify(2.0);").expect("ok"), "2.0\n");
        assert_eq!(
            run_src("echo json_stringify(\"a\\nb\");").expect("ok"),
            "\"a\\nb\"\n"
        );
        // Idempotencia: parse(stringify(v)) preserva o valor.
        assert_eq!(
            run_src(
                "let v = {a: [1, {b: null}], s: \"x\"}; echo json_parse(json_stringify(v)) == v;"
            )
            .expect("ok"),
            "true\n"
        );
        // Numero inteiro grande alem de i64 vira Float (perda documentada).
        assert_eq!(
            run_src("echo type(json_parse(\"99999999999999999999\"));").expect("ok"),
            "float\n"
        );
    }

    #[test]
    fn reports_json_errors() {
        // Malformados com posicao.
        for bad in [
            "echo json_parse(\"{oops}\");",
            "echo json_parse(\"[1,]\");",
            "echo json_parse(\"{\\\"a\\\":}\");",
            "echo json_parse(\"01\");",
            "echo json_parse(\"05\");",
            "echo json_parse(\"\\\\uD800\");",
            "echo json_parse(\"trailing x\");",
        ] {
            let err = run_err(bad);
            assert!(err.contains("json_parse"), "obteve: {err} em {bad}");
        }
        // Nao-string e erro de tipo.
        let err = run_err("echo json_parse(42);");
        assert!(err.contains("exige string"), "obteve: {err}");
        // Funcao, NaN e infinito nao serializam.
        let err = run_err("fn f() { } echo json_stringify(f);");
        assert!(err.contains("nao representa"), "obteve: {err}");
        let err = run_err("echo json_stringify(pow(-1.0, 0.5));");
        assert!(err.contains("nao representa"), "obteve: {err}");
        let err = run_err("echo json_stringify(pow(10.0, 309.0));");
        assert!(err.contains("nao representa"), "obteve: {err}");
    }

    #[test]
    fn reports_json_depth_limit() {
        // Aninhamento alem de 128 e erro, nunca estouro de pilha.
        let deep_in: String = "[".repeat(200) + &"]".repeat(200);
        let err = run_err(&format!("echo json_parse(\"{deep_in}\");"));
        assert!(err.contains("profundidade"), "obteve: {err}");
        // Valor aninhado construido em runtime (literal fundo nao parseia).
        let err = run_err(
            "let v = 0; let i = 0; while i < 200 { v = [v]; i = i + 1; } echo json_stringify(v);",
        );
        assert!(err.contains("profundidade"), "obteve: {err}");
    }

    #[test]
    fn runs_fs_and_env_builtins() {
        let dir = std::env::temp_dir().join(format!("rhp-fs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir temporario");
        let path = dir.join("dado.txt").to_string_lossy().into_owned();
        assert_eq!(
            run_src(&format!("echo fs_write(\"{path}\", \"ola\");")).expect("ok"),
            "3\n"
        );
        assert_eq!(
            run_src(&format!("echo fs_append(\"{path}\", \"!\");")).expect("ok"),
            "1\n"
        );
        assert_eq!(
            run_src(&format!("echo fs_read(\"{path}\");")).expect("ok"),
            "ola!\n"
        );
        assert_eq!(
            run_src(&format!("echo fs_exists(\"{path}\");")).expect("ok"),
            "true\n"
        );
        assert_eq!(
            run_src(&format!(
                "echo fs_exists(\"{}/nao-existe.rhp\");",
                dir.display()
            ))
            .expect("ok"),
            "false\n"
        );
        std::env::set_var("RHP_TESTE_UNICO_XYZ", "123");
        assert_eq!(
            run_src("echo env(\"RHP_TESTE_UNICO_XYZ\");").expect("ok"),
            "123\n"
        );
        assert_eq!(
            run_src("echo env(\"RHP_INEXISTENTE_XYZ\");").expect("ok"),
            "null\n"
        );
        assert_eq!(run_src("echo len(argv());").expect("ok"), "0\n");
    }

    #[test]
    fn reports_http_errors() {
        assert!(run_err("echo http_get(\"ftp://x/y\");").contains("apenas urls `http://`"));
        assert!(run_err("echo http_get(42);").contains("exige string"));
        assert!(run_err("echo http_get(\"http://127.0.0.1:1/\");").contains("falhou ao conectar"));
        // `.invalid` nunca resolve (RFC 2606): falha rapida, sem travar.
        assert!(run_err("echo http_get(\"http://nao-existe.invalid/\");")
            .contains("falhou ao resolver"));
    }
}
