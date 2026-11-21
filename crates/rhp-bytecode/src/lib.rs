//! Bytecode e geracao de codigo do RHP (Fase 3).
//!
//! Modelo: chunk com pool de constantes, pool de nomes, tabela de funcoes e
//! vetor de instrucoes, mais builtins da biblioteca-padrao nucleo. A VM e
//! baseada em pilha e valida o chunk (recursivamente) antes de executar.

use rhp_parser::{
    AssignTarget, BinOp, CallArg, CallData, DestructureTarget, Expr, GenericParam, MatchArm,
    Program, Stmt, StmtNode, SwitchCase, TypeTag, UnaryOp,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

/// Valor de runtime do RHP (Fase 3: + `Array` e `Map`).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Array(Vec<Value>),
    Map(HashMap<String, Value>),
    /// Funcao de primeira classe (RFC 0003): definicao compilada +
    /// ambiente capturado por valor no momento da criacao (`MakeFunc`).
    Func(FuncValue),
    /// Classe (tabela de metodos; RFC 0017, Etapa 5). Valor de `class Nome`.
    Class(Rc<ClassDef>),
    /// Instancia de classe (`new`): classe + campos mutaveis.
    Instance {
        class: Rc<ClassDef>,
        fields: HashMap<String, Value>,
    },
    /// Gerador lazy (`fn` com `yield`; RFC 0018, Etapa 5). Igualdade por
    /// identidade (mesmo objeto + mesmo `done`).
    Generator(GenHandle),
}

/// Frame de execucao: uma ativacao de chunk (principal ou funcao).
///
/// Mora aqui (e nao na VM) para que `GenState` possa suspender e retomar
/// frames por valor, sem registros nem vazamentos (RFC 0018). O chunk e
/// compartilhado por `Rc`; somente leitura durante a execucao.
#[derive(Debug, Clone)]
pub struct Frame {
    pub chunk: Rc<Chunk>,
    pub ip: usize,
    pub stack: Vec<Value>,
    /// Pilha de escopos locais; `scopes[0]` e o escopo da ativacao
    /// (parametros no caso de funcao).
    pub scopes: Vec<HashMap<String, Value>>,
    /// Pilha de handlers de `try` (RFC 0001); o mais interno por ultimo.
    pub handlers: Vec<Handler>,
    /// `new` em voo (RFC 0017): nome da classe enquanto o construtor
    /// `novo` executa; vive no frame DO CONSTRUTOR.
    pub pending_new: Option<String>,
    /// Metodo em voo (`var.metodo()`; RFC 0017): nome da variavel para
    /// write-back do `this` final. Vive no frame DO METODO.
    pub pending_method: Option<String>,
    /// `super.metodo()` em voo (Fase Next-Gen 1): write-back do `this`
    /// final no `this` do chamador (escopo de ativacao). Vive no frame DO
    /// METODO DO PAI.
    pub pending_super: bool,
    /// Gerador em voo (RFC 0018): de quem e este resume; o `Yield`/`Return`
    /// do frame salva/marca o gerador e devolve o valor ao chamador.
    /// Vive no frame DO GERADOR (morre com ele no unwind, sem stale).
    pub pending_gen: Option<GenHandle>,
    /// HOF com callback em voo (RFC 0005): progresso da iteracao enquanto
    /// as ativacoes do callback executam e retornam. `None` no comum.
    pub pending: Option<PendingHof>,
    /// Indice do frame do callback em voo do HOF acima (`None` = nenhum).
    /// Amarra o `pending` a ativacao: se o unwinding truncar o callback,
    /// o `pending` vira obsoleto e e descartado (RHP-AUD-IND-001) em vez
    /// de ser consumido por um `Return` posterior nao relacionado.
    pub pending_cb: Option<usize>,
    /// Definicao da funcao desta ativacao (`None` no programa principal).
    pub func: Option<Rc<FuncDef>>,
}

/// Estado de um gerador (RFC 0018, Etapa 5): funcao, frame suspenso
/// (`None` = ainda nao iniciado) e flag de fim. O frame e movido para
/// dentro/fora a cada `next`/`yield` (dono unico, sem aliasing).
#[derive(Debug, Clone)]
pub struct GenState {
    pub func: FuncValue,
    pub frame: Option<Frame>,
    pub done: bool,
}

/// Identidade de gerador (handle compartilhado; ver RFC 0018).
/// `PartialEq` por identidade (`Rc::ptr_eq` + `done`): estrutural
/// recursaria para sempre em ciclos (frame que captura o gerador).
#[derive(Debug, Clone)]
pub struct GenHandle(pub Rc<RefCell<GenState>>);

impl PartialEq for GenHandle {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0) && self.0.borrow().done == other.0.borrow().done
    }
}

/// Estado de `map`/`filter`/`usort` entre chamadas do callback (RFC 0005).
/// O frame chamador guarda o progresso; cada retorno do callback avanca
/// (`advance_hof`) ate finalizar com o resultado empilhado.
#[derive(Debug, Clone)]
pub enum PendingHof {
    /// `map`: aplica `func` a cada item, acumulando resultados.
    Map {
        func: FuncValue,
        items: Vec<Value>,
        /// Indice do item cuja chamada esta em voo.
        next: usize,
        acc: Vec<Value>,
    },
    /// `filter`: como `map`, mas so acumula quando o retorno e `Bool(true)`.
    Filter {
        func: FuncValue,
        items: Vec<Value>,
        next: usize,
        acc: Vec<Value>,
    },
    /// `usort`: insertion sort estavel dirigido por comparacoes.
    Sort {
        func: FuncValue,
        sorted: Vec<Value>,
        /// Proximo par a comparar: `(sorted[j], sorted[j-1])`, `j = max(1, ..)`.
        i: usize,
        j: usize,
    },
}

/// Handler de `try`: para onde saltar e com que escopos/pilha.
#[derive(Debug, Clone, Copy)]
pub struct Handler {
    /// Instrucao do `catch` ou do bloco `finally`+`Rethrow`.
    pub target: usize,
    /// `scopes.len()` ao registrar (restaura no unwinding).
    pub scope_len: usize,
}

/// Definicao de classe compilada (RFC 0017, Etapa 5): nome + tabela de
/// metodos (cada um um `FuncValue` com ambiente capturado na definicao;
/// `this` e vinculado na chamada).
/// Fase Next-Gen 1: `parent` guarda o nome da classe pai (para `super` e
/// captura tipada); os metodos ja vao achatados (herdados + proprios).
#[derive(Debug, Clone, PartialEq)]
pub struct ClassDef {
    pub name: String,
    pub parent: Option<String>,
    pub methods: HashMap<String, FuncValue>,
}

/// Valor funcional de primeira classe: definicao + ambiente capturado.
#[derive(Debug, Clone, PartialEq)]
pub struct FuncValue {
    pub def: Rc<FuncDef>,
    pub env: HashMap<String, Value>,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Nunca aborta (VM-001/SEC-001): alem do teto escreve o marcador.
        // Caminhos de usuario (`echo`, `str()`, `throw`, template) usam
        // `to_display_string`, que devolve erro tratavel em vez de truncar.
        if self.fmt_limited(f, 0).is_err() {
            write!(f, "<profundidade maxima excedida>")?;
        }
        Ok(())
    }
}

impl Value {
    /// Formata com teto de profundidade (`MAX_VALUE_DEPTH`, 128): cada
    /// nivel de aninhamento consome 1 frame Rust, 128 sao seguros.
    /// `Err(())` = teto excedido (o chamador decide: marcador ou erro).
    fn fmt_limited(&self, w: &mut dyn fmt::Write, depth: usize) -> Result<(), ()> {
        if depth > MAX_VALUE_DEPTH {
            return Err(());
        }
        match self {
            Self::Null => write!(w, "null").map_err(|_| ())?,
            Self::Bool(b) => write!(w, "{b}").map_err(|_| ())?,
            Self::Int(i) => write!(w, "{i}").map_err(|_| ())?,
            Self::Float(x) => write!(w, "{x}").map_err(|_| ())?,
            Self::Str(s) => write!(w, "{s}").map_err(|_| ())?,
            Self::Array(items) => {
                write!(w, "[").map_err(|_| ())?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(w, ", ").map_err(|_| ())?;
                    }
                    item.fmt_limited(w, depth + 1)?;
                }
                write!(w, "]").map_err(|_| ())?;
            }
            Self::Map(entries) => {
                Self::fmt_map_limited(w, depth, entries.iter())?;
            }
            Self::Class(def) => write!(w, "<classe {}>", def.name).map_err(|_| ())?,
            Self::Instance { fields, .. } => {
                Self::fmt_map_limited(w, depth, fields.iter())?;
            }
            Self::Func(func) => write!(w, "<fn {}>", func.def.name).map_err(|_| ())?,
            Self::Generator(_) => {
                let state = self.generator_state_or_fail()?;
                write!(w, "<gerador {}>", state.func.def.name).map_err(|_| ())?
            }
        }
        Ok(())
    }

    /// Exibicao estrita para caminhos de usuario: `Ok(string)` ou
    /// `Err` tratavel (capturavel por `try`), nunca abort.
    pub fn to_display_string(&self) -> Result<String, String> {
        let mut out = String::new();
        self.fmt_limited(&mut out, 0).map_err(|_| {
            format!("valor excede profundidade maxima de exibicao ({MAX_VALUE_DEPTH})")
        })?;
        Ok(out)
    }

    fn generator_state_or_fail(&self) -> Result<std::cell::Ref<'_, crate::GenState>, ()> {
        match self {
            Self::Generator(gen) => gen.0.try_borrow().map_err(|_| ()),
            _ => Err(()),
        }
    }

    fn fmt_map_limited<'a>(
        w: &mut dyn fmt::Write,
        depth: usize,
        entries: impl Iterator<Item = (&'a String, &'a Value)>,
    ) -> Result<(), ()> {
        let mut keys: Vec<&String> = Vec::new();
        let mut values: Vec<&Value> = Vec::new();
        for (k, v) in entries {
            keys.push(k);
            values.push(v);
        }
        let mut order: Vec<usize> = (0..keys.len()).collect();
        order.sort_by(|&a, &b| keys[a].cmp(keys[b]));
        write!(w, "{{").map_err(|_| ())?;
        for (i, &pos) in order.iter().enumerate() {
            if i > 0 {
                write!(w, ", ").map_err(|_| ())?;
            }
            write!(w, "{}: ", keys[pos]).map_err(|_| ())?;
            values[pos].fmt_limited(w, depth + 1)?;
        }
        write!(w, "}}").map_err(|_| ())?;
        Ok(())
    }
}

/// Definicao de funcao compilada.
///
/// `body` e `Rc` (o mesmo chunk e referenciado pelo `Frame` da ativacao;
/// sem clonagem profunda por chamada; Etapa 3).
#[derive(Debug, Clone, PartialEq)]
pub struct FuncDef {
    pub name: String,
    pub params: Vec<String>,
    /// Padrao por posicao (`None` = obrigatorio; RFC 0006). Alinhado a `params`.
    pub defaults: Vec<Option<Value>>,
    /// `...rest` (no maximo um, por ultimo; RFC 0006).
    pub variadic: Option<String>,
    /// Tipos dos parametros, alinhado a `params` (`None` = sem anotacao;
    /// RFC 0016). Checados no `activate` da VM (apos defaults).
    pub param_types: Vec<Option<TypeTag>>,
    /// Tipo de retorno (`: tipo`; RFC 0016). Checado no `Return` da VM.
    pub return_type: Option<TypeTag>,
    /// `true` se o corpo contem `yield` (funcao geradora; RFC 0018).
    /// Chamar NAO executa: devolve `Value::Generator` lazy.
    pub is_generator: bool,
    /// `true` se `async fn` (Fase Next-Gen 3): a chamada gera tarefa
    /// (nao bloqueia o chamador; `await` junta o resultado).
    pub is_async: bool,
    /// Nativa (Fase Next-Gen 6): `Some("ext::func")` executa Rust em vez de
    /// bytecode (sem frame; chamada sincrona com os mesmos erros). `None` = RHP.
    pub native_id: Option<String>,
    pub body: Rc<Chunk>,
}

/// Builtin da biblioteca-padrao nucleo (Fases 3 e 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Builtin {
    Len,
    Push,
    Pop,
    Keys,
    Str,
    Int,
    Float,
    Type,
    Assert,
    /// `http_get(url)` — cliente HTTP minimo (somente `http://`), Fase 6.
    HttpGet,
    // Biblioteca-padrao: strings, matematica e mapas (prox. evolucao).
    Contains,
    StartsWith,
    EndsWith,
    Split,
    Join,
    Trim,
    Upper,
    Lower,
    Replace,
    Abs,
    Min,
    Max,
    Sqrt,
    Pow,
    Floor,
    Ceil,
    Values,
    Entries,
    Merge,
    Has,
    FsRead,
    FsWrite,
    FsAppend,
    FsExists,
    Argv,
    Env,
    /// `sort(array)` — copia ordenada (homogeneo: numeros, strings ou bools).
    Sort,
    /// `exit(codigo)` — encerra com codigo 0–255 (via erro especial).
    Exit,
    /// `json_parse(texto)` — JSON => valor (nulo/bool/numero/string/array/mapa).
    JsonParse,
    /// `json_stringify(valor)` — valor => texto JSON.
    JsonStringify,
    /// `now()` — segundos desde 1970-01-01T00:00:00Z (`Int`).
    Now,
    /// `sleep(segundos)` — bloqueia a thread (`Int`/`Float`, teto 1h).
    Sleep,
    /// `date_str(ts)` — `Int` (epoch) => `"AAAA-MM-DDTHH:MM:SSZ"` (UTC).
    DateStr,
    /// `http_fetch(url, opts)` — GET com redirects/headers/timeout (Fase A5).
    HttpFetch,
    /// `db_open(caminho)` — abre/cria SQLite (`Int` = handle).
    DbOpen,
    /// `db_exec(h, sql, params)` — executa; devolve linhas afetadas.
    DbExec,
    /// `db_query(h, sql, params)` — consulta; devolve array de mapas.
    DbQuery,
    /// `db_close(h)` — fecha handle (invalida para reuso).
    DbClose,
    /// `http_serve(porta, handler, opts)` — servidor HTTP (Fase C).
    HttpServe,
    /// `template(texto, valores)` — interpola `{{chave}}` com escape HTML.
    Template,
    /// `map(arr, f)` — aplica `f(elem)` e empilha array de resultados (RFC 0005).
    Map,
    /// `filter(arr, f)` — `f(elem)` deve ser `Bool`; empilha filtrados (RFC 0005).
    Filter,
    /// `usort(arr, cmp)` — insertion sort estavel via `cmp(a, b): Int` (RFC 0005).
    Usort,
    /// `hash_password(senha)` — Argon2id (`$argon2id$v=19$...`; RFC 0013).
    HashPassword,
    /// `verify_password(senha, hash)` — `Bool`; `$argon2id$` ou bcrypt `$2*$` (RFC 0013).
    VerifyPassword,
    /// `regex_match(s, padrao)` — `Bool`; tempo linear, sem lookaround/backrefs (RFC 0014).
    RegexMatch,
    /// `regex_replace(s, padrao, subst)` — `$1`/`$name`; `Str` (RFC 0014).
    RegexReplace,
    /// `regex_split(s, padrao)` — `Array` de `Str` (RFC 0014).
    RegexSplit,
    /// `sha256(s)` — hex; puro Rust (RFC 0014).
    Sha256,
    /// `hmac_sha256(chave, msg)` — tag hex, tempo constante na verificacao (RFC 0014).
    HmacSha256,
    /// `hmac_verify(chave, msg, tag)` — `Bool` (RFC 0014).
    HmacVerify,
    /// `rand()` — `Int` em `[0, i64::MAX]` (xorshift64*; NAO criptografico; RFC 0012).
    Rand,
    /// `rand_range(min, max)` — `Int` em `[min, max]` (inclusive; RFC 0012).
    RandRange,
    /// `fs_list(dir)` — nomes de entradas ordenados (`[]` se vazio; RFC 0012).
    FsList,
    /// `next(gen)` — proximo valor do gerador (erro se esgotado; RFC 0018).
    Next,
    /// `exhausted(gen)` — `Bool`: gerador terminou (RFC 0018).
    Exhausted,
    /// `async_spawn("f")` — `Int` (handle); roda `f` em thread (RFC 0019).
    AsyncSpawn,
    /// `async_join(h)` — bloqueia e devolve o retorno (RFC 0019).
    AsyncJoin,
    /// `await_timeout(h, s)` — junta com prazo em segundos; estouro = erro
    /// capturavel, tarefa segue em voo (item RFC 0030).
    AwaitTimeout,
    /// `async_cancel(h)` — sinaliza cancelamento cooperativo (`Null`).
    AsyncCancel,
    /// `async_cancelled()` — `Bool`: a tarefa atual foi cancelada.
    AsyncCancelled,
    /// `await_all([h, ...])` — junta em ordem, devolve array (falha rapida).
    AwaitAll,
    /// `await_race([h, ...])` — devolve o primeiro pronto (demais seguem).
    AwaitRace,
    /// `tcp_connect(host, porta)` — `Int` (handle; RFC 0020).
    TcpConnect,
    /// `tcp_send(h, dados)` — bytes escritos (`Int`; RFC 0020).
    TcpSend,
    /// `tcp_recv(h, max)` — ate `max` bytes (`Str`; `Null` em EOF; RFC 0020).
    TcpRecv,
    /// `tcp_close(h)` — fecha (`Null`; RFC 0020).
    TcpClose,
    /// `redis_open(host, porta)` — `Int` (handle; sem AUTH na v1; RFC 0020).
    RedisOpen,
    /// `redis_exec(h, [cmd, args...])` — valor RESP (RFC 0020).
    RedisExec,
    /// `redis_close(h)` — fecha (`Null`; RFC 0020).
    RedisClose,
    /// `fs_rename(de, para)` — renomeia/move (`Null`; erro limpo).
    FsRename,
    /// `fs_remove(caminho)` — remove arquivo ou diretorio vazio (`Null`).
    FsRemove,
    /// `fs_mkdir(caminho)` — cria diretorio (inclui pais; `Null`).
    FsMkdir,
    /// `fs_chmod(caminho, modo)` — permissao Unix (`Int` octal; `Null`).
    FsChmod,
    /// `fs_stat(caminho)` — mapa `{tamanho, e_diretorio, e_arquivo,
    /// somente_leitura, modificado}` (epoch `Int`; erro se inexistente).
    FsStat,
}

/// Retorna o builtin para um nome, se existir.
#[must_use]
pub fn builtin_by_name(name: &str) -> Option<Builtin> {
    match name {
        "len" => Some(Builtin::Len),
        "push" => Some(Builtin::Push),
        "pop" => Some(Builtin::Pop),
        "keys" => Some(Builtin::Keys),
        "str" => Some(Builtin::Str),
        "int" => Some(Builtin::Int),
        "float" => Some(Builtin::Float),
        "type" => Some(Builtin::Type),
        "assert" => Some(Builtin::Assert),
        "http_get" => Some(Builtin::HttpGet),
        "contains" => Some(Builtin::Contains),
        "starts_with" => Some(Builtin::StartsWith),
        "ends_with" => Some(Builtin::EndsWith),
        "split" => Some(Builtin::Split),
        "join" => Some(Builtin::Join),
        "trim" => Some(Builtin::Trim),
        "upper" => Some(Builtin::Upper),
        "lower" => Some(Builtin::Lower),
        "replace" => Some(Builtin::Replace),
        "abs" => Some(Builtin::Abs),
        "min" => Some(Builtin::Min),
        "max" => Some(Builtin::Max),
        "sqrt" => Some(Builtin::Sqrt),
        "pow" => Some(Builtin::Pow),
        "floor" => Some(Builtin::Floor),
        "ceil" => Some(Builtin::Ceil),
        "values" => Some(Builtin::Values),
        "entries" => Some(Builtin::Entries),
        "merge" => Some(Builtin::Merge),
        "has" => Some(Builtin::Has),
        "fs_read" => Some(Builtin::FsRead),
        "fs_write" => Some(Builtin::FsWrite),
        "fs_append" => Some(Builtin::FsAppend),
        "fs_exists" => Some(Builtin::FsExists),
        "argv" => Some(Builtin::Argv),
        "env" => Some(Builtin::Env),
        "sort" => Some(Builtin::Sort),
        "exit" => Some(Builtin::Exit),
        "json_parse" => Some(Builtin::JsonParse),
        "json_stringify" => Some(Builtin::JsonStringify),
        "now" => Some(Builtin::Now),
        "sleep" => Some(Builtin::Sleep),
        "date_str" => Some(Builtin::DateStr),
        "http_fetch" => Some(Builtin::HttpFetch),
        "db_open" => Some(Builtin::DbOpen),
        "db_exec" => Some(Builtin::DbExec),
        "db_query" => Some(Builtin::DbQuery),
        "db_close" => Some(Builtin::DbClose),
        "http_serve" => Some(Builtin::HttpServe),
        "template" => Some(Builtin::Template),
        "map" => Some(Builtin::Map),
        "filter" => Some(Builtin::Filter),
        "usort" => Some(Builtin::Usort),
        "rand" => Some(Builtin::Rand),
        "rand_range" => Some(Builtin::RandRange),
        "fs_list" => Some(Builtin::FsList),
        "hash_password" => Some(Builtin::HashPassword),
        "verify_password" => Some(Builtin::VerifyPassword),
        "regex_match" => Some(Builtin::RegexMatch),
        "regex_replace" => Some(Builtin::RegexReplace),
        "regex_split" => Some(Builtin::RegexSplit),
        "sha256" => Some(Builtin::Sha256),
        "hmac_sha256" => Some(Builtin::HmacSha256),
        "hmac_verify" => Some(Builtin::HmacVerify),
        "next" => Some(Builtin::Next),
        "exhausted" => Some(Builtin::Exhausted),
        "async_spawn" => Some(Builtin::AsyncSpawn),
        "async_join" => Some(Builtin::AsyncJoin),
        "await_timeout" => Some(Builtin::AwaitTimeout),
        "async_cancel" => Some(Builtin::AsyncCancel),
        "async_cancelled" => Some(Builtin::AsyncCancelled),
        "await_all" => Some(Builtin::AwaitAll),
        "await_race" => Some(Builtin::AwaitRace),
        "tcp_connect" => Some(Builtin::TcpConnect),
        "tcp_send" => Some(Builtin::TcpSend),
        "tcp_recv" => Some(Builtin::TcpRecv),
        "tcp_close" => Some(Builtin::TcpClose),
        "redis_open" => Some(Builtin::RedisOpen),
        "redis_exec" => Some(Builtin::RedisExec),
        "redis_close" => Some(Builtin::RedisClose),
        "fs_rename" => Some(Builtin::FsRename),
        "fs_remove" => Some(Builtin::FsRemove),
        "fs_mkdir" => Some(Builtin::FsMkdir),
        "fs_chmod" => Some(Builtin::FsChmod),
        "fs_stat" => Some(Builtin::FsStat),
        _ => None,
    }
}

#[must_use]
pub const fn builtin_arity(builtin: Builtin) -> usize {
    match builtin {
        Builtin::Argv | Builtin::Now | Builtin::Rand | Builtin::AsyncCancelled => 0,
        Builtin::Len
        | Builtin::Pop
        | Builtin::Keys
        | Builtin::Str
        | Builtin::Int
        | Builtin::Float
        | Builtin::Type
        | Builtin::Assert
        | Builtin::HttpGet
        | Builtin::Trim
        | Builtin::Upper
        | Builtin::Lower
        | Builtin::Abs
        | Builtin::Sqrt
        | Builtin::Floor
        | Builtin::Ceil
        | Builtin::Values
        | Builtin::Entries
        | Builtin::FsRead
        | Builtin::FsExists
        | Builtin::Env
        | Builtin::Sort
        | Builtin::Exit
        | Builtin::JsonParse
        | Builtin::JsonStringify
        | Builtin::Sleep
        | Builtin::DateStr
        | Builtin::DbOpen
        | Builtin::DbClose
        | Builtin::FsList
        | Builtin::HashPassword
        | Builtin::Next
        | Builtin::Exhausted
        | Builtin::AsyncSpawn
        | Builtin::AsyncJoin
        | Builtin::AsyncCancel
        | Builtin::AwaitAll
        | Builtin::AwaitRace
        | Builtin::TcpClose
        | Builtin::RedisClose
        | Builtin::FsRemove
        | Builtin::FsMkdir
        | Builtin::FsStat
        | Builtin::Sha256 => 1,
        Builtin::Push
        | Builtin::Contains
        | Builtin::StartsWith
        | Builtin::EndsWith
        | Builtin::Split
        | Builtin::Join
        | Builtin::Min
        | Builtin::Max
        | Builtin::Pow
        | Builtin::Merge
        | Builtin::Has
        | Builtin::FsWrite
        | Builtin::FsAppend
        | Builtin::HttpFetch
        | Builtin::Template
        | Builtin::Map
        | Builtin::Filter
        | Builtin::Usort
        | Builtin::RandRange
        | Builtin::VerifyPassword
        | Builtin::RegexMatch
        | Builtin::RegexSplit
        | Builtin::HmacSha256
        | Builtin::TcpConnect
        | Builtin::TcpSend
        | Builtin::TcpRecv
        | Builtin::RedisOpen
        | Builtin::RedisExec
        | Builtin::AwaitTimeout
        | Builtin::FsRename
        | Builtin::FsChmod => 2,
        Builtin::DbExec
        | Builtin::DbQuery
        | Builtin::HttpServe
        | Builtin::RegexReplace
        | Builtin::HmacVerify => 3,
        Builtin::Replace => 3,
    }
}

/// Opcodes da VM (Fase 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// Empilha `constants[idx]`.
    Constant(u32),
    /// Desempilha um valor e escreve sua representacao + `\n` na saida.
    Print,
    /// Encerra a execucao com sucesso. Todo chunk valido termina com `Halt`.
    Halt,
    // Aritmetica e logica: operandos na pilha, resultado empilhado.
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Neg,
    Not,
    BitNot,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    /// Duplica o topo da pilha.
    Dup,
    /// Desempilha; se nao for nulo, salta para `ip = alvo`.
    JumpIfNotNull(u32),
    // Comparacao: dois operandos, empilha `Bool`.
    Eq,
    /// Igualdade estrita (RFC 0008, `match`): mesmo tipo + valor
    /// (`Int`/`Float` comparam numericamente); resto exige variante igual.
    StrictEq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    /// Desempilha `Bool`; se falso, salta para `ip = alvo`.
    JumpIfFalse(u32),
    /// Desempilha `Bool`; se verdadeiro, salta para `ip = alvo` (RFC 0018).
    JumpIfTrue(u32),
    /// Salta para `ip = alvo`.
    Jump(u32),
    /// Empilha o valor atual de `names[idx]` (erro se indefinido).
    LoadName(u32),
    /// Desempilha e atribui a variavel existente `names[idx]`.
    StoreName(u32),
    /// Desempilha e declara `names[idx]` no escopo atual.
    DeclareName(u32),
    /// Desempilha e declara a estatica `names[idx]` da funcao atual
    /// (primeira atribuicao vence; RFC 0010). Erro fora de funcao.
    /// Leituras/escritas usam `LoadName`/`StoreName` com fallback em runtime
    /// (sem ops dedicados — sem codigo morto).
    DeclareStatic(u32),
    /// Abre um escopo de bloco.
    EnterScope,
    /// Fecha o escopo de bloco atual.
    ExitScope,
    /// Cria `Func` de `funcs[idx]` e declara no escopo atual.
    DefineFunc(u32),
    /// Cria `Func` de `funcs[idx]` capturando o ambiente atual por valor
    /// (RFC 0003: snapshot dos escopos internos para os externos) e o
    /// empilha como valor de primeira classe.
    MakeFunc(u32),
    /// Chama a funcao no topo (apos `aridade` argumentos); empilha retorno.
    Call(u32),
    /// Chamada mista (RFC 0009): callee + valores em ordem do fonte; o
    /// descritor `constants[idx]` (`Array` de `Int(0)`=posicional,
    /// `Int(1)`=spread, `Str`=nomeado) guia a ligacao em runtime.
    CallMixed(u32),
    /// Retorna da funcao com o valor no topo da pilha.
    Return,
    /// Descarta o valor no topo da pilha.
    Pop,
    // Compostos e builtins (Fase 3).
    /// Desempilha `n` valores e empilha `Array` (ordem preservada).
    BuildArray(u32),
    /// Desempilha `n` pares chave/valor e empilha `Map` (chaves `Str`).
    BuildMap(u32),
    /// Desempilha indice e base; empilha `base[indice]`.
    Index,
    /// Desempilha valor e indice; grava em `names[idx][indice]`.
    StoreIndex(u32),
    /// Desempilha iteravel e empilha lista de iteracao: array vira copia;
    /// mapa vira array de chaves `Str` ordenadas; outro tipo e erro.
    IterList,
    /// Registra handler de `try` (alvo do `catch`/`finally`); metadados
    /// fora da pilha de operandos (altura inalterada).
    TryBegin(u32),
    /// Remove o handler do `try` (caminho normal).
    TryEnd,
    /// Desempilha valor e o lanca como erro (unwinding).
    Throw,
    /// Relanca o erro em propagacao (fim de `finally` sem `catch`).
    Rethrow,
    /// Chama o builtin `id` (`n` = aridade fixa do builtin).
    CallBuiltin(BuiltinId),
    /// Checa o topo da pilha contra a tag (anotacao `: tipo`, RFC 0016;
    /// sem desempilhar; erro `tipos incompativeis: esperava ...`).
    CheckType(TypeTag),
    /// Constroi classe (`BuildClass(nome_idx, n)`): desempilha `n` pares
    /// `[nome(Str), metodo(Func)]` e empilha `Value::Class` (RFC 0017).
    BuildClass(u32, u32),
    /// Constroi classe com heranca (`BuildClassEx(nome, pai, n)`; Fase
    /// Next-Gen 1): como `BuildClass`, mas herda os metodos do pai (a VM
    /// busca o pai nas globais; erro se inexistente ou nao-classe) e guarda
    /// o nome do pai para `super` e captura tipada. Metodos proprios
    /// sobrescrevem os herdados.
    BuildClassEx(u32, u32, u32),
    /// Checa instancia contra classe (`IsInstanceOf(nome_idx)`; Fase
    /// Next-Gen 4): desempilha o valor e empilha `Bool` (`true` se for
    /// instancia da classe ou de subclasse via cadeia `parent`).
    IsInstanceOf(u32),
    /// Suspende ate a tarefa concluir (`Await`; Fase Next-Gen 3):
    /// desempilha o valor; se for handle de tarefa (`Int` valido), bloqueia
    /// ate o fim e empilha o retorno (erros propagam); senao, empilha o
    /// proprio valor (identidade para nao-tarefas).
    Await,
    /// Instancia (`NewMixed(desc_idx)`, como `CallMixed`): desempilha os
    /// valores + a classe, ativa `novo` com `this` e entrega a
    /// instancia (retorno do construtor ignorado; RFC 0017).
    NewMixed(u32),
    /// Chamada de metodo sobre variavel (`CallMethod(var, campo, desc)`;
    /// RFC 0017): como `Index` + chamada, mas com write-back do `this`
    /// final na variavel (mutacoes via `obj.metodo()` persistem).
    CallMethod(u32, u32, u32),
    /// Chamada de metodo do pai (`CallSuper(campo, desc)`; Fase Next-Gen 1):
    /// como `CallMethod`, mas o `this` e o da ativacao atual e o metodo vem
    /// da classe pai (erro se sem pai ou sem metodo). O `this` final volta
    /// ao escopo (write-back local, coerente com metodos normais).
    CallSuper(u32, u32),
    /// Carrega extensao nativa (`LoadNative(nome_idx)`; Fase Next-Gen 6):
    /// registra as funcoes da extensao como globais (erro se inexistente
    /// ou ABI incompativel). Idempotente por extensao (segunda carga = erro
    /// claro para evitar sombra silenciosa).
    LoadNative(u32),
    /// Suspender gerador (`Yield`; RFC 0018): desempilha o valor e o
    /// entrega ao retomador, guardando o frame para continuar depois.
    Yield,
    /// Exige `Bool` no topo (RFC 0031): `&&`/`||` verificam o operando
    /// direito (o esquerdo ja passa por `JumpIfFalse`/`JumpIfTrue`).
    /// Neutro na pilha (espia); erro E3003 fora de `Bool`.
    AssertBool,
}

/// Identificador numerico estavel de builtin (para o bytecode).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinId(pub u32);

impl BuiltinId {
    /// Converte para [`Builtin`]; ids desconhecidos sao erro de validacao.
    pub fn resolve(self) -> Result<Builtin, CompileError> {
        match self.0 {
            0 => Ok(Builtin::Len),
            1 => Ok(Builtin::Push),
            2 => Ok(Builtin::Pop),
            3 => Ok(Builtin::Keys),
            4 => Ok(Builtin::Str),
            5 => Ok(Builtin::Int),
            6 => Ok(Builtin::Float),
            7 => Ok(Builtin::Type),
            8 => Ok(Builtin::Assert),
            9 => Ok(Builtin::HttpGet),
            10 => Ok(Builtin::Contains),
            11 => Ok(Builtin::StartsWith),
            12 => Ok(Builtin::EndsWith),
            13 => Ok(Builtin::Split),
            14 => Ok(Builtin::Join),
            15 => Ok(Builtin::Trim),
            16 => Ok(Builtin::Upper),
            17 => Ok(Builtin::Lower),
            18 => Ok(Builtin::Replace),
            19 => Ok(Builtin::Abs),
            20 => Ok(Builtin::Min),
            21 => Ok(Builtin::Max),
            22 => Ok(Builtin::Sqrt),
            23 => Ok(Builtin::Pow),
            24 => Ok(Builtin::Floor),
            25 => Ok(Builtin::Ceil),
            26 => Ok(Builtin::Values),
            27 => Ok(Builtin::Entries),
            28 => Ok(Builtin::Merge),
            29 => Ok(Builtin::Has),
            30 => Ok(Builtin::FsRead),
            31 => Ok(Builtin::FsWrite),
            32 => Ok(Builtin::FsAppend),
            33 => Ok(Builtin::FsExists),
            34 => Ok(Builtin::Argv),
            35 => Ok(Builtin::Env),
            36 => Ok(Builtin::Sort),
            37 => Ok(Builtin::Exit),
            38 => Ok(Builtin::JsonParse),
            39 => Ok(Builtin::JsonStringify),
            40 => Ok(Builtin::Now),
            41 => Ok(Builtin::Sleep),
            42 => Ok(Builtin::DateStr),
            43 => Ok(Builtin::HttpFetch),
            44 => Ok(Builtin::DbOpen),
            45 => Ok(Builtin::DbExec),
            46 => Ok(Builtin::DbQuery),
            47 => Ok(Builtin::DbClose),
            48 => Ok(Builtin::HttpServe),
            49 => Ok(Builtin::Template),
            50 => Ok(Builtin::Map),
            51 => Ok(Builtin::Filter),
            52 => Ok(Builtin::Usort),
            53 => Ok(Builtin::Rand),
            54 => Ok(Builtin::RandRange),
            55 => Ok(Builtin::FsList),
            56 => Ok(Builtin::HashPassword),
            57 => Ok(Builtin::VerifyPassword),
            58 => Ok(Builtin::RegexMatch),
            59 => Ok(Builtin::RegexReplace),
            60 => Ok(Builtin::RegexSplit),
            61 => Ok(Builtin::Sha256),
            62 => Ok(Builtin::HmacSha256),
            63 => Ok(Builtin::HmacVerify),
            64 => Ok(Builtin::Next),
            65 => Ok(Builtin::Exhausted),
            66 => Ok(Builtin::AsyncSpawn),
            67 => Ok(Builtin::AsyncJoin),
            68 => Ok(Builtin::TcpConnect),
            69 => Ok(Builtin::TcpSend),
            70 => Ok(Builtin::TcpRecv),
            71 => Ok(Builtin::TcpClose),
            72 => Ok(Builtin::RedisOpen),
            73 => Ok(Builtin::RedisExec),
            74 => Ok(Builtin::RedisClose),
            75 => Ok(Builtin::AwaitTimeout),
            76 => Ok(Builtin::AsyncCancel),
            77 => Ok(Builtin::AsyncCancelled),
            78 => Ok(Builtin::AwaitAll),
            79 => Ok(Builtin::AwaitRace),
            80 => Ok(Builtin::FsRename),
            81 => Ok(Builtin::FsRemove),
            82 => Ok(Builtin::FsMkdir),
            83 => Ok(Builtin::FsChmod),
            84 => Ok(Builtin::FsStat),
            other => Err(CompileError::new(format!("builtin desconhecido: {other}"))),
        }
    }
}

impl From<Builtin> for BuiltinId {
    fn from(builtin: Builtin) -> Self {
        let id = match builtin {
            Builtin::Len => 0,
            Builtin::Push => 1,
            Builtin::Pop => 2,
            Builtin::Keys => 3,
            Builtin::Str => 4,
            Builtin::Int => 5,
            Builtin::Float => 6,
            Builtin::Type => 7,
            Builtin::Assert => 8,
            Builtin::HttpGet => 9,
            Builtin::Contains => 10,
            Builtin::StartsWith => 11,
            Builtin::EndsWith => 12,
            Builtin::Split => 13,
            Builtin::Join => 14,
            Builtin::Trim => 15,
            Builtin::Upper => 16,
            Builtin::Lower => 17,
            Builtin::Replace => 18,
            Builtin::Abs => 19,
            Builtin::Min => 20,
            Builtin::Max => 21,
            Builtin::Sqrt => 22,
            Builtin::Pow => 23,
            Builtin::Floor => 24,
            Builtin::Ceil => 25,
            Builtin::Values => 26,
            Builtin::Entries => 27,
            Builtin::Merge => 28,
            Builtin::Has => 29,
            Builtin::FsRead => 30,
            Builtin::FsWrite => 31,
            Builtin::FsAppend => 32,
            Builtin::FsExists => 33,
            Builtin::Argv => 34,
            Builtin::Env => 35,
            Builtin::Sort => 36,
            Builtin::Exit => 37,
            Builtin::JsonParse => 38,
            Builtin::JsonStringify => 39,
            Builtin::Now => 40,
            Builtin::Sleep => 41,
            Builtin::DateStr => 42,
            Builtin::HttpFetch => 43,
            Builtin::DbOpen => 44,
            Builtin::DbExec => 45,
            Builtin::DbQuery => 46,
            Builtin::DbClose => 47,
            Builtin::HttpServe => 48,
            Builtin::Template => 49,
            Builtin::Map => 50,
            Builtin::Filter => 51,
            Builtin::Usort => 52,
            Builtin::Rand => 53,
            Builtin::RandRange => 54,
            Builtin::FsList => 55,
            Builtin::HashPassword => 56,
            Builtin::VerifyPassword => 57,
            Builtin::RegexMatch => 58,
            Builtin::RegexReplace => 59,
            Builtin::RegexSplit => 60,
            Builtin::Sha256 => 61,
            Builtin::HmacSha256 => 62,
            Builtin::HmacVerify => 63,
            Builtin::Next => 64,
            Builtin::Exhausted => 65,
            Builtin::AsyncSpawn => 66,
            Builtin::AsyncJoin => 67,
            Builtin::TcpConnect => 68,
            Builtin::TcpSend => 69,
            Builtin::TcpRecv => 70,
            Builtin::TcpClose => 71,
            Builtin::RedisOpen => 72,
            Builtin::RedisExec => 73,
            Builtin::RedisClose => 74,
            Builtin::AwaitTimeout => 75,
            Builtin::AsyncCancel => 76,
            Builtin::AsyncCancelled => 77,
            Builtin::AwaitAll => 78,
            Builtin::AwaitRace => 79,
            Builtin::FsRename => 80,
            Builtin::FsRemove => 81,
            Builtin::FsMkdir => 82,
            Builtin::FsChmod => 83,
            Builtin::FsStat => 84,
        };
        Self(id)
    }
}

/// Chunk de bytecode: constantes, nomes, funcoes e codigo.
///
/// `funcs` guarda `Rc` (compartilhado entre a tabela do chunk e os frames
/// da VM sem clonar por chamada; Etapa 3).
#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub constants: Vec<Value>,
    pub names: Vec<String>,
    pub funcs: Vec<Rc<FuncDef>>,
    pub code: Vec<Op>,
}

/// Profundidade maxima de valores compostos aninhados em constantes
/// (protecao contra estouro de pilha em `Display`/execucao; ver AUD-001).
pub const MAX_VALUE_DEPTH: usize = 128;

impl Chunk {
    #[must_use]
    pub fn new() -> Self {
        Self {
            constants: Vec::new(),
            names: Vec::new(),
            funcs: Vec::new(),
            code: Vec::new(),
        }
    }

    /// Valida o chunk e, iterativamente, os corpos das funcoes.
    ///
    /// Iterativo de proposito: chunks artesanais (fora do compilador) podem
    /// aninhar definicoes sem limite, e a validacao nao deve estourar a
    /// pilha antes de rejeita-los.
    ///
    /// # Erros
    ///
    /// Retorna [`CompileError`] se o codigo estiver vazio, nao terminar com
    /// [`Op::Halt`], referenciar constante/nome/funcao/builtin inexistente,
    /// salto fora do codigo ou constante alem da profundidade maxima.
    pub fn validate(&self) -> Result<(), CompileError> {
        let mut pending: Vec<(&Chunk, String)> = vec![(self, "principal".to_string())];
        while let Some((chunk, context)) = pending.pop() {
            chunk.validate_shallow(&context)?;
            for func in &chunk.funcs {
                pending.push((&func.body, format!("funcao `{}`", func.name)));
            }
        }
        Ok(())
    }

    fn validate_shallow(&self, context: &str) -> Result<(), CompileError> {
        if self.code.is_empty() {
            return Err(CompileError::new(format!(
                "chunk vazio em `{context}`: sem instrucoes"
            )));
        }
        if self.code.last() != Some(&Op::Halt) {
            return Err(CompileError::new(format!(
                "chunk invalido em `{context}`: sem `Halt` final"
            )));
        }
        for value in &self.constants {
            check_constant_depth(value)?;
        }
        let len = self.code.len();
        for op in &self.code {
            match *op {
                Op::Constant(idx) if (idx as usize) >= self.constants.len() => {
                    return Err(CompileError::new(format!(
                        "chunk invalido em `{context}`: constante {idx} fora do pool"
                    )));
                }
                Op::LoadName(idx)
                | Op::StoreName(idx)
                | Op::DeclareName(idx)
                | Op::DeclareStatic(idx)
                | Op::StoreIndex(idx)
                    if (idx as usize) >= self.names.len() =>
                {
                    return Err(CompileError::new(format!(
                        "chunk invalido em `{context}`: nome {idx} fora do pool"
                    )));
                }
                Op::BuildClass(name_idx, _) if (name_idx as usize) >= self.names.len() => {
                    return Err(CompileError::new(format!(
                        "chunk invalido em `{context}`: nome {name_idx} fora do pool"
                    )));
                }
                Op::BuildClassEx(name_idx, parent_idx, _)
                    if (name_idx as usize) >= self.names.len()
                        || (parent_idx as usize) >= self.names.len() =>
                {
                    return Err(CompileError::new(format!(
                        "chunk invalido em `{context}`: nome fora do pool em BuildClassEx"
                    )));
                }
                Op::IsInstanceOf(name_idx) if (name_idx as usize) >= self.names.len() => {
                    return Err(CompileError::new(format!(
                        "chunk invalido em `{context}`: nome {name_idx} fora do pool"
                    )));
                }
                Op::LoadNative(name_idx) if (name_idx as usize) >= self.names.len() => {
                    return Err(CompileError::new(format!(
                        "chunk invalido em `{context}`: nome {name_idx} fora do pool"
                    )));
                }
                Op::DefineFunc(idx) | Op::MakeFunc(idx) if (idx as usize) >= self.funcs.len() => {
                    return Err(CompileError::new(format!(
                        "chunk invalido em `{context}`: funcao {idx} inexistente"
                    )));
                }
                Op::JumpIfFalse(target)
                | Op::JumpIfTrue(target)
                | Op::Jump(target)
                | Op::JumpIfNotNull(target)
                | Op::TryBegin(target)
                    if (target as usize) >= len =>
                {
                    return Err(CompileError::new(format!(
                        "chunk invalido em `{context}`: salto para {target} fora do codigo"
                    )));
                }
                Op::CallBuiltin(id) => {
                    id.resolve()?;
                }
                Op::CallMixed(desc) | Op::NewMixed(desc) => {
                    // Descritor existe; o tipo e checado em `call_mixed_slots`.
                    call_mixed_slots(&self.constants, desc, context)?;
                }
                Op::CallMethod(var_idx, field_idx, desc) => {
                    if (var_idx as usize) >= self.names.len() {
                        return Err(CompileError::new(format!(
                            "chunk invalido em `{context}`: nome {var_idx} fora do pool"
                        )));
                    }
                    match self.constants.get(field_idx as usize) {
                        Some(Value::Str(_)) => {}
                        _ => {
                            return Err(CompileError::new(format!(
                                "chunk invalido em `{context}`: campo de metodo {field_idx} invalido"
                            )));
                        }
                    }
                    call_mixed_slots(&self.constants, desc, context)?;
                }
                Op::CallSuper(field_idx, desc) => {
                    match self.constants.get(field_idx as usize) {
                        Some(Value::Str(_)) => {}
                        _ => {
                            return Err(CompileError::new(format!(
                                "chunk invalido em `{context}`: campo de super {field_idx} invalido"
                            )));
                        }
                    }
                    call_mixed_slots(&self.constants, desc, context)?;
                }
                _ => {}
            }
        }
        check_stack_flow(&self.code, &self.constants, context)?;
        Ok(())
    }
}

/// Numero de slots do descritor de `CallMixed` (RFC 0009): `constants[idx]`
/// deve ser `Array` de `Int(0|1)` (posicional/spread) ou `Str` (nomeado).
fn call_mixed_slots(constants: &[Value], desc: u32, context: &str) -> Result<usize, CompileError> {
    match constants.get(desc as usize) {
        Some(Value::Array(items))
            if items
                .iter()
                .all(|m| matches!(m, Value::Int(0) | Value::Int(1) | Value::Str(_))) =>
        {
            Ok(items.len())
        }
        _ => Err(CompileError::new(format!(
            "chunk invalido em `{context}`: descritor de chamada mista {desc} invalido"
        ))),
    }
}

/// Checa profundidade de uma constante sem recursao (pilha explicita).
fn check_constant_depth(value: &Value) -> Result<(), CompileError> {
    let mut stack: Vec<(&Value, usize)> = vec![(value, 0)];
    while let Some((current, depth)) = stack.pop() {
        if depth > MAX_VALUE_DEPTH {
            return Err(CompileError::new(format!(
                "constante excede profundidade maxima ({MAX_VALUE_DEPTH})"
            )));
        }
        match current {
            Value::Array(items) => {
                for item in items {
                    stack.push((item, depth + 1));
                }
            }
            Value::Map(entries) => {
                for item in entries.values() {
                    stack.push((item, depth + 1));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Analise estatica de altura da pilha de operandos (NG-202).
///
/// Prova que nenhum caminho de execucao sofre underflow e que juncoes de
/// fluxo concordam na altura. Independente da execucao; roda dentro do
/// `validate`, para o chunk raiz e cada corpo de funcao (pilha zerada).
/// Codigo inalancavel apos `Return`/`Jump` e ignorado (mas continua sujeito
/// as checagens estruturais de `validate_shallow`).
fn check_stack_flow(code: &[Op], constants: &[Value], context: &str) -> Result<(), CompileError> {
    let mut entry: Vec<Option<i64>> = vec![None; code.len()];
    entry[0] = Some(0);
    let mut worklist = vec![0usize];
    while let Some(ip) = worklist.pop() {
        let height = entry[ip].expect("posicao alcancavel tem altura");
        let op = code[ip];
        // Retorna (altura apos a instrucao, alvos; `None` = terminal).
        let (after, targets): (i64, Option<Vec<usize>>) = match op {
            Op::Constant(_) | Op::LoadName(_) => (height + 1, Some(vec![ip + 1])),
            Op::Print | Op::Pop | Op::StoreName(_) | Op::DeclareName(_) | Op::DeclareStatic(_) => {
                check_depth(height, 1, op, context)?;
                (height - 1, Some(vec![ip + 1]))
            }
            Op::Neg | Op::Not | Op::BitNot => {
                check_depth(height, 1, op, context)?;
                (height, Some(vec![ip + 1]))
            }
            // `CheckType` so espia o topo (anotacao `: tipo`, RFC 0016).
            // `AssertBool` idem (RFC 0031).
            Op::CheckType(_) | Op::AssertBool => {
                check_depth(height, 1, op, context)?;
                (height, Some(vec![ip + 1]))
            }
            Op::IterList => {
                check_depth(height, 1, op, context)?;
                (height, Some(vec![ip + 1]))
            }
            Op::Add
            | Op::Sub
            | Op::Mul
            | Op::Div
            | Op::Mod
            | Op::Eq
            | Op::StrictEq
            | Op::Ne
            | Op::Lt
            | Op::Le
            | Op::Gt
            | Op::Ge
            | Op::BitAnd
            | Op::BitOr
            | Op::BitXor
            | Op::Shl
            | Op::Shr
            | Op::Index => {
                check_depth(height, 2, op, context)?;
                (height - 1, Some(vec![ip + 1]))
            }
            Op::Dup => {
                check_depth(height, 1, op, context)?;
                (height + 1, Some(vec![ip + 1]))
            }
            Op::StoreIndex(_) => {
                check_depth(height, 2, op, context)?;
                (height - 2, Some(vec![ip + 1]))
            }
            Op::JumpIfFalse(target) | Op::JumpIfNotNull(target) | Op::JumpIfTrue(target) => {
                check_depth(height, 1, op, context)?;
                (height - 1, Some(vec![target as usize, ip + 1]))
            }
            Op::TryBegin(_) => (height, Some(vec![ip + 1])),
            Op::TryEnd => (height, Some(vec![ip + 1])),
            Op::Throw | Op::Rethrow => {
                check_depth(height, 1, op, context)?;
                (height, None)
            }
            Op::Jump(target) => (height, Some(vec![target as usize])),
            Op::DefineFunc(_) | Op::EnterScope | Op::ExitScope => (height, Some(vec![ip + 1])),
            // `MakeFunc` captura o ambiente sem tocar na pilha de operandos.
            Op::MakeFunc(_) => (height + 1, Some(vec![ip + 1])),
            Op::Halt | Op::Return => (height, None),
            // `Yield` desempilha e sai do frame (como `Return`); o
            // validador estatico nao acompanha o salto dinamico.
            Op::Yield => (height, None),
            Op::Call(arity) => {
                let arity = arity as usize;
                check_depth(height, arity as i64 + 1, op, context)?;
                (height - arity as i64, Some(vec![ip + 1]))
            }
            Op::CallMixed(desc) => {
                let slots = call_mixed_slots(constants, desc, context)? as i64;
                check_depth(height, slots + 1, op, context)?;
                (height - slots, Some(vec![ip + 1]))
            }
            Op::NewMixed(desc) => {
                // Como `CallMixed` (desempilha valores + classe, empilha
                // a instancia): mesma aritmetica de pilha.
                let slots = call_mixed_slots(constants, desc, context)? as i64;
                check_depth(height, slots + 1, op, context)?;
                (height - slots, Some(vec![ip + 1]))
            }
            Op::CallMethod(_, _, desc) => {
                // Desempilha so os argumentos (a variavel e lida pelo nome);
                // empilha o retorno.
                let slots = call_mixed_slots(constants, desc, context)? as i64;
                check_depth(height, slots, op, context)?;
                (height - slots + 1, Some(vec![ip + 1]))
            }
            Op::BuildClass(_, count) => {
                let pairs = count as i64;
                check_depth(height, pairs * 2, op, context)?;
                (height - pairs * 2 + 1, Some(vec![ip + 1]))
            }
            Op::BuildClassEx(_, _, count) => {
                let pairs = count as i64;
                check_depth(height, pairs * 2, op, context)?;
                (height - pairs * 2 + 1, Some(vec![ip + 1]))
            }
            Op::IsInstanceOf(_) => {
                check_depth(height, 1, op, context)?;
                (height, Some(vec![ip + 1]))
            }
            Op::LoadNative(_) => (height, Some(vec![ip + 1])),
            Op::Await => {
                check_depth(height, 1, op, context)?;
                (height, Some(vec![ip + 1]))
            }
            Op::CallSuper(_, desc) => {
                let slots = call_mixed_slots(constants, desc, context)? as i64;
                check_depth(height, slots, op, context)?;
                (height - slots + 1, Some(vec![ip + 1]))
            }
            Op::CallBuiltin(id) => {
                // `validate_shallow` ja rejeitou ids desconhecidos.
                // Builtins nao recebem callee: desempilha so os argumentos.
                let arity = builtin_arity(id.resolve()?) as i64;
                check_depth(height, arity, op, context)?;
                (height - arity + 1, Some(vec![ip + 1]))
            }
            Op::BuildArray(count) => {
                check_depth(height, count as i64, op, context)?;
                (height - count as i64 + 1, Some(vec![ip + 1]))
            }
            Op::BuildMap(count) => {
                check_depth(height, count as i64 * 2, op, context)?;
                (height - count as i64 * 2 + 1, Some(vec![ip + 1]))
            }
        };
        if let Some(targets) = targets {
            for target in targets {
                if target == code.len() {
                    continue; // fim do codigo apos `Halt` final
                }
                match entry[target] {
                    None => {
                        entry[target] = Some(after);
                        worklist.push(target);
                    }
                    Some(previous) if previous != after => {
                        return Err(CompileError::new(format!(
                            "chunk invalido em `{context}`: altura de pilha inconsistente em {target} ({previous} vs {after})"
                        )));
                    }
                    Some(_) => {}
                }
            }
        }
        // Alvo do handler: caminho de erro entra com altura +1 (o valor).
        if let Op::TryBegin(target) = op {
            let target = target as usize;
            if target != code.len() {
                match entry[target] {
                    None => {
                        entry[target] = Some(height + 1);
                        worklist.push(target);
                    }
                    Some(previous) if previous != height + 1 => {
                        return Err(CompileError::new(format!(
                            "chunk invalido em `{context}`: altura de pilha inconsistente em {target} ({previous} vs {})",
                            height + 1
                        )));
                    }
                    Some(_) => {}
                }
            }
        }
    }
    Ok(())
}

fn check_depth(height: i64, need: i64, op: Op, context: &str) -> Result<(), CompileError> {
    if height < need {
        return Err(CompileError::new(format!(
            "chunk invalido em `{context}`: pilha insuficiente para `{op:?}` (altura {height})"
        )));
    }
    Ok(())
}

impl Default for Chunk {
    fn default() -> Self {
        Self::new()
    }
}

/// Erro de compilacao/validacao de bytecode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileError {
    pub message: String,
}

impl CompileError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CompileError {}

struct Compiler {
    chunk: Chunk,
    in_function: bool,
    names: std::collections::HashMap<String, u32>,
    constants: std::collections::HashMap<ConstKey, u32>,
    /// Pilha de escopos lexicos (nomes de `const` por nivel); espelha os
    /// `EnterScope`/`ExitScope` emitidos. Usada para `break`/`continue` com
    /// desenrolamento correto e para proibir atribuicao a constantes.
    scopes: Vec<std::collections::HashSet<String>>,
    loops: Vec<LoopCtx>,
    /// Pilha de `try`s com `finally` em compilacao (o mais interno por
    /// ultimo); usada para fechar handlers e duplicar `finally` em
    /// `break`/`continue`/`return` que cruzam o `try`.
    pending_trys: Vec<PendingTry>,
    /// Reentrancia na emissao de copias de `finally`: dentro da copia,
    /// desvios nao disparam nova duplicacao (substituem o fluxo em curso).
    emitting_finally: u32,
    /// Contador de lacos `for` (nomes ocultos unicos por laco).
    for_counter: u32,
    /// Sequencia de closures anonimas (`{closure:N}`; item 1): nomes unicos
    /// e deterministicos (ordem de travessia), para resolucao precisa de
    /// tarefas async no worker (antes, todas chamavam-se `{closure}`).
    closure_seq: u32,
    /// Funcoes de topo (nome qualificado => generics; item bounds): para
    /// checar `f<T>(...)` no uso. So topo (`!in_function` ao definir).
    functions: std::collections::HashMap<String, Vec<GenericParam>>,
    /// Parametros de tipo visiveis aqui (item bounds): proprio + ancestrais.
    /// Herdado pelos compiladores aninhados; uso como argumento dispensa a
    /// checagem do bound (sera verificado na instanciacao concreta).
    type_params: Vec<String>,
    /// Namespace atual (`namespace A\B;`; Fase Next-Gen 2): prefixo aplicado
    /// as definicoes seguintes no mesmo arquivo.
    current_namespace: Option<String>,
    /// Classes ja definidas (nome => info): para heranca/interfaces/traits
    /// (Fase Next-Gen 1). Metodos achatados nao ficam aqui (viva no chunk);
    /// guardamos assinaturas para checagem de `implements` e aridade.
    classes: std::collections::HashMap<String, ClassInfo>,
    /// Interfaces (nome => metodos): so assinaturas (Fase Next-Gen 1).
    interfaces: std::collections::HashMap<String, Vec<SigInfo>>,
    /// Traits (nome => metodos com corpo): para composicao `use` (Fase Next-Gen 1).
    traits: std::collections::HashMap<String, Vec<TraitMethodInfo>>,
}

/// Assinatura de metodo para checagem de interface (nome + aridade).
#[derive(Debug, Clone)]
struct SigInfo {
    name: String,
    params: usize,
    variadic: bool,
}

/// Metodo de trait guardado para composicao (assinatura + indice da funcao).
#[derive(Debug, Clone)]
struct TraitMethodInfo {
    name: String,
    params: usize,
    variadic: bool,
    func_idx: u32,
}

/// Info de classe para heranca (Fase Next-Gen 1) + generics (item bounds).
#[derive(Debug, Clone)]
struct ClassInfo {
    parent: Option<String>,
    methods: std::collections::HashMap<String, SigInfoEx>,
    generics: Vec<GenericParam>,
}

/// Assinatura estendida (com indice da funcao para heranca/traits).
#[derive(Debug, Clone)]
struct SigInfoEx {
    params: usize,
    variadic: bool,
    func_idx: u32,
}

/// Contexto de um `try` em compilacao (para `break`/`continue`/`return`
/// que o cruzam: fechar handler e rodar `finally` antes do desvio).
struct PendingTry {
    /// Corpo do `finally` (clonado) para duplicar nas saidas.
    finally_body: Option<Vec<StmtNode>>,
    /// `true` enquanto o corpo protegido compila (handler ativo no
    /// runtime); `false` nas regioes de `catch`/`finally` (handler ja
    /// consumido pelo unwinding ou pelo `TryEnd` do caminho normal).
    handler_live: bool,
    /// `loops.len()` ao abrir o `try`: desvios miram o laco do topo, logo
    /// so cruzam `try`s abertos dentro dele.
    loop_depth: usize,
}

/// Contexto de compilacao de um laco ou `switch` (para `break`/`continue`).
/// `switch` empilha com `is_loop: false`: `break` mira o topo (seja laco
/// ou `switch`); `continue` ignora `switch` e mira o laco mais interno.
struct LoopCtx {
    /// Verdadeiro para `while`/`for`; falso para `switch`.
    is_loop: bool,
    /// Profundidade de escopo a manter ao sair com `break`.
    /// (`while`: fecha tambem o escopo do corpo; `for`: mantem o escopo
    /// do laco, fechado na saida, pois a condicao vive dentro dele.)
    break_depth: usize,
    /// Profundidade de escopo a manter ao voltar com `continue`.
    continue_depth: usize,
    /// Inicio do laco (`while`: salto direto do `continue`; `for`: `None`,
    /// pois o `continue` salta ao incremento — remendado depois do corpo).
    start: Option<u32>,
    /// Posicoes de salto a remendar para a saida.
    breaks: Vec<usize>,
    /// Posicoes de salto a remendar para o ponto de continuacao.
    continues: Vec<usize>,
}

/// Chave de deduplicacao de constantes (somente escalares; `Float` por bits
/// para `NaN`s distintos nao colidirem). `Array`/`Map`/`Func` nao entram
/// (custo de comparacao profunda sem beneficio tipico).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ConstKey {
    Int(i64),
    Float(u64),
    Str(String),
    Bool(bool),
    Null,
}

/// Avalia expressao constante (defaults de parametros, RFC 0006) para
/// `Value` em tempo de compilacao. Espelha as regras de `LANGUAGE_SPEC`
/// §4 para os casos suportados; resto (variaveis, chamadas, closures,
/// indice, ternario) e erro: padrao deve ser constante.
fn eval_const_expr(expr: &Expr) -> Result<Value, CompileError> {
    let not_const = || {
        CompileError::new(
            "padrao de parametro deve ser expressao constante (literal, operacao entre constantes ou composto de constantes)".to_string(),
        )
    };
    match expr {
        Expr::StringLiteral(s) => Ok(Value::Str(s.clone())),
        Expr::Int(n) => Ok(Value::Int(*n)),
        Expr::Float(x) => Ok(Value::Float(*x)),
        Expr::Bool(b) => Ok(Value::Bool(*b)),
        Expr::Null => Ok(Value::Null),
        Expr::Array(items) => items
            .iter()
            .map(eval_const_expr)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        Expr::Map(entries) => entries
            .iter()
            .map(|(k, v)| eval_const_expr(v).map(|folded| (k.clone(), folded)))
            .collect::<Result<std::collections::HashMap<_, _>, _>>()
            .map(Value::Map),
        Expr::Unary { op, expr } => {
            let v = eval_const_expr(expr)?;
            match (op, v) {
                (UnaryOp::Neg, Value::Int(n)) => n.checked_neg().map(Value::Int).ok_or_else(|| {
                    CompileError::new("estouro de `Int` em expressao constante".to_string())
                }),
                (UnaryOp::Neg, Value::Float(x)) => Ok(Value::Float(-x)),
                (UnaryOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
                (UnaryOp::BitNot, Value::Int(n)) => Ok(Value::Int(!n)),
                _ => Err(CompileError::new(
                    "operacao unaria invalida em expressao constante".to_string(),
                )),
            }
        }
        Expr::Binary { op, left, right } => {
            let l = eval_const_expr(left)?;
            let r = eval_const_expr(right)?;
            eval_const_binary(*op, l, r)
        }
        _ => Err(not_const()),
    }
}

/// Aplica operador binario a valores constantes (regras de `LANGUAGE_SPEC` §4).
fn eval_const_binary(op: BinOp, l: Value, r: Value) -> Result<Value, CompileError> {
    use Value::{Bool as B, Float as F, Int as I, Str as S};
    let invalid = || {
        CompileError::new(
            "operacao invalida em expressao constante (tipos incompativeis)".to_string(),
        )
    };
    match op {
        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
            let num_err =
                || CompileError::new("aritmetica em expressao constante exige numeros".to_string());
            match (l, r) {
                (I(a), I(b)) => {
                    let res = match op {
                        BinOp::Add => a.checked_add(b),
                        BinOp::Sub => a.checked_sub(b),
                        BinOp::Mul => a.checked_mul(b),
                        BinOp::Div => (b != 0).then(|| a / b),
                        _ => (b != 0).then(|| a % b),
                    };
                    res.map(I).ok_or_else(|| {
                        CompileError::new(
                            "estouro de `Int` ou divisao por zero em expressao constante"
                                .to_string(),
                        )
                    })
                }
                (F(a), F(b)) => {
                    if matches!(op, BinOp::Div | BinOp::Mod) && b == 0.0 {
                        return Err(CompileError::new(
                            "divisao por zero em expressao constante".to_string(),
                        ));
                    }
                    let res = match op {
                        BinOp::Add => a + b,
                        BinOp::Sub => a - b,
                        BinOp::Mul => a * b,
                        BinOp::Div => a / b,
                        _ => a % b,
                    };
                    Ok(F(res))
                }
                (I(a), F(b)) => eval_const_binary(op, F(a as f64), F(b)),
                (F(a), I(b)) => eval_const_binary(op, F(a), F(b as f64)),
                (S(a), S(b)) if matches!(op, BinOp::Add) => Ok(S(a + &b)),
                _ => Err(num_err()),
            }
        }
        BinOp::Eq => Ok(B(const_eq(&l, &r))),
        BinOp::Ne => Ok(B(!const_eq(&l, &r))),
        BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
            let ord = match (&l, &r) {
                (I(a), I(b)) => a.partial_cmp(b),
                (F(a), F(b)) => a.partial_cmp(b),
                (I(a), F(b)) => (*a as f64).partial_cmp(b),
                (F(a), I(b)) => a.partial_cmp(&(*b as f64)),
                (S(a), S(b)) => Some(a.cmp(b)),
                _ => None,
            };
            match ord {
                Some(o) => Ok(B(match op {
                    BinOp::Lt => o.is_lt(),
                    BinOp::Le => o.is_le(),
                    BinOp::Gt => o.is_gt(),
                    _ => o.is_ge(),
                })),
                None => Err(invalid()),
            }
        }
        BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => match (l, r) {
            (I(a), I(b)) => Ok(I(match op {
                BinOp::BitAnd => a & b,
                BinOp::BitOr => a | b,
                _ => a ^ b,
            })),
            _ => Err(invalid()),
        },
        // `&&`/`||` em constantes: so literais `Bool` (mesma regra estrita
        // do runtime; curto-circuito irrelevante sem efeitos colaterais).
        BinOp::LogicalAnd | BinOp::LogicalOr => match (l, r) {
            (B(a), B(b)) => Ok(B(match op {
                BinOp::LogicalAnd => a && b,
                _ => a || b,
            })),
            _ => Err(invalid()),
        },
        BinOp::Shl | BinOp::Shr => match (l, r) {
            (I(a), I(b)) if (0..64).contains(&b) => Ok(I(match op {
                BinOp::Shl => a << b,
                _ => a >> b,
            })),
            _ => Err(CompileError::new(
                "deslocamento invalido em expressao constante (0-63)".to_string(),
            )),
        },
        BinOp::NullCoalesce => Ok(match l {
            Value::Null => r,
            v => v,
        }),
    }
}

/// Igualdade de constantes: numeros com promocao, resto por tipo/valor.
fn const_eq(l: &Value, r: &Value) -> bool {
    match (l, r) {
        (Value::Int(a), Value::Int(b)) => a == b,
        (Value::Float(a), Value::Float(b)) => a == b,
        (Value::Int(a), Value::Float(b)) => *a as f64 == *b,
        (Value::Float(a), Value::Int(b)) => *a == *b as f64,
        _ => l == r,
    }
}

impl Compiler {
    fn new(in_function: bool) -> Self {
        Self {
            chunk: Chunk::new(),
            in_function,
            names: std::collections::HashMap::new(),
            constants: std::collections::HashMap::new(),
            scopes: vec![std::collections::HashSet::new()],
            loops: Vec::new(),
            pending_trys: Vec::new(),
            emitting_finally: 0,
            for_counter: 0,
            closure_seq: 0,
            functions: std::collections::HashMap::new(),
            type_params: Vec::new(),
            current_namespace: None,
            classes: std::collections::HashMap::new(),
            interfaces: std::collections::HashMap::new(),
            traits: std::collections::HashMap::new(),
        }
    }

    /// Aplica o namespace atual ao nome (se nao-qualificado e namespace ativo).
    fn qualify(&self, name: &str) -> String {
        if name.contains('\\') {
            return name.to_string();
        }
        match self.current_namespace.as_ref() {
            Some(ns) => format!("{ns}\\{name}"),
            None => name.to_string(),
        }
    }

    fn emit_scope_open(&mut self) {
        self.emit(Op::EnterScope);
        self.scopes.push(std::collections::HashSet::new());
    }

    fn emit_scope_close(&mut self) -> Result<(), CompileError> {
        if self.scopes.len() <= 1 {
            return Err(CompileError::new(
                "desequilibrio interno de escopos".to_string(),
            ));
        }
        self.emit(Op::ExitScope);
        self.scopes.pop();
        Ok(())
    }

    fn is_const(&self, name: &str) -> bool {
        self.scopes.iter().any(|scope| scope.contains(name))
    }

    fn declare_const(&mut self, name: &str) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string());
        }
    }

    fn const_key(value: &Value) -> Option<ConstKey> {
        match value {
            Value::Int(v) => Some(ConstKey::Int(*v)),
            Value::Float(v) => Some(ConstKey::Float(v.to_bits())),
            Value::Str(s) => Some(ConstKey::Str(s.clone())),
            Value::Bool(b) => Some(ConstKey::Bool(*b)),
            Value::Null => Some(ConstKey::Null),
            // Compostos, funcoes, classes, instancias e geradores: sem
            // dedup (comparacao profunda cara, beneficio atipico).
            Value::Array(_)
            | Value::Map(_)
            | Value::Func(_)
            | Value::Class(_)
            | Value::Instance { .. }
            | Value::Generator(_) => None,
        }
    }

    fn add_constant(&mut self, value: Value) -> Result<u32, CompileError> {
        if let Some(key) = Self::const_key(&value) {
            if let Some(&idx) = self.constants.get(&key) {
                return Ok(idx);
            }
        }
        if self.chunk.constants.len() >= u32::MAX as usize {
            return Err(CompileError::new("limite de constantes excedido"));
        }
        self.chunk.constants.push(value.clone());
        let idx = (self.chunk.constants.len() - 1) as u32;
        if let Some(key) = Self::const_key(&value) {
            self.constants.insert(key, idx);
        }
        Ok(idx)
    }

    fn intern_name(&mut self, name: &str) -> Result<u32, CompileError> {
        if let Some(&idx) = self.names.get(name) {
            return Ok(idx);
        }
        if self.chunk.names.len() >= u32::MAX as usize {
            return Err(CompileError::new("limite de nomes excedido"));
        }
        self.chunk.names.push(name.to_string());
        let idx = (self.chunk.names.len() - 1) as u32;
        self.names.insert(name.to_string(), idx);
        Ok(idx)
    }

    fn emit(&mut self, op: Op) -> usize {
        self.chunk.code.push(op);
        self.chunk.code.len() - 1
    }

    fn patch_jump(&mut self, pos: usize, target: u32) {
        let len = self.chunk.code.len() as u32;
        debug_assert!((target as usize) <= len as usize);
        match self.chunk.code[pos] {
            Op::JumpIfFalse(_) => self.chunk.code[pos] = Op::JumpIfFalse(target),
            Op::JumpIfTrue(_) => self.chunk.code[pos] = Op::JumpIfTrue(target),
            Op::JumpIfNotNull(_) => self.chunk.code[pos] = Op::JumpIfNotNull(target),
            Op::Jump(_) => self.chunk.code[pos] = Op::Jump(target),
            Op::TryBegin(_) => self.chunk.code[pos] = Op::TryBegin(target),
            _ => debug_assert!(false, "patch_jump em posicao sem salto"),
        }
    }

    fn compile_stmts(&mut self, stmts: &[StmtNode]) -> Result<(), CompileError> {
        for node in stmts {
            self.compile_stmt(&node.stmt)?;
        }
        Ok(())
    }

    /// Compila o corpo de uma funcao (nomeada ou closure) em compilador
    /// aninhado, registra o `FuncDef` na tabela e devolve o indice.
    /// Usado por `Stmt::FnDef` e `Expr::Closure` (RFC 0003); defaults e
    /// `...rest` pela RFC 0006 (defaults dobrados para `Value` aqui).
    /// `own_types` sao os parametros de tipo declarados aqui (o resto do
    /// escopo vem herdado); 8 args por convergencia de flags, como em
    /// `call_builtin`/`resolve_dep` (ver `#[allow]` abaixo).
    #[allow(clippy::too_many_arguments)]
    fn compile_func_body(
        &mut self,
        name: &str,
        params: &[rhp_parser::Param],
        variadic: Option<&str>,
        return_ann: Option<TypeTag>,
        body: &[StmtNode],
        is_async: bool,
        own_types: &[String],
    ) -> Result<u32, CompileError> {
        let mut seen = std::collections::HashSet::new();
        let mut names = Vec::with_capacity(params.len());
        let mut defaults = Vec::with_capacity(params.len());
        let mut param_types = Vec::with_capacity(params.len());
        for param in params {
            if !seen.insert(param.name.clone()) {
                return Err(CompileError::new(format!(
                    "parametro duplicado: {}",
                    param.name
                )));
            }
            names.push(param.name.clone());
            defaults.push(param.default.as_ref().map(eval_const_expr).transpose()?);
            param_types.push(param.ann);
        }
        if let Some(rest) = variadic {
            if !seen.insert(rest.to_string()) {
                return Err(CompileError::new(format!("parametro duplicado: {rest}")));
            }
        }
        let mut fn_compiler = Compiler::new(true);
        // Herda contexto de namespaces e tipos (Fase Next-Gen): metodos e
        // closures enxergam as mesmas classes/interfaces/traits. A sequencia
        // de closures e compartilhada (ida e volta) para nomes deterministicos.
        fn_compiler.current_namespace = self.current_namespace.clone();
        fn_compiler.classes = self.classes.clone();
        fn_compiler.interfaces = self.interfaces.clone();
        fn_compiler.traits = self.traits.clone();
        fn_compiler.closure_seq = self.closure_seq;
        // Escopo de tipos: ancestrais + proprios da definicao (item bounds).
        fn_compiler.type_params = self
            .type_params
            .iter()
            .chain(own_types.iter())
            .cloned()
            .collect();
        fn_compiler.compile_stmts(body)?;
        self.closure_seq = fn_compiler.closure_seq;
        let null_idx = fn_compiler.add_constant(Value::Null)?;
        fn_compiler.emit(Op::Constant(null_idx));
        fn_compiler.emit(Op::Return);
        fn_compiler.emit(Op::Halt);
        let body_chunk = fn_compiler.chunk;
        body_chunk.validate()?;
        let is_generator = body_chunk.code.iter().any(|op| matches!(op, Op::Yield));
        if self.chunk.funcs.len() >= u32::MAX as usize {
            return Err(CompileError::new("limite de funcoes excedido"));
        }
        self.chunk.funcs.push(Rc::new(FuncDef {
            name: name.to_string(),
            params: names,
            defaults,
            variadic: variadic.map(str::to_string),
            param_types,
            return_type: return_ann,
            is_generator,
            is_async,
            native_id: None,
            body: Rc::new(body_chunk),
        }));
        Ok((self.chunk.funcs.len() - 1) as u32)
    }

    /// Nome oculto do laco `for` (lista ou indice): contem NUL, impossivel
    /// de escrever no fonte, logo nunca colide com variaveis do usuario.
    fn hidden_for_name(&mut self, what: &str) -> String {
        let id = self.for_counter;
        self.for_counter += 1;
        format!("\0for:{what}:{id}")
    }

    /// Nome oculto generico para temporarios (ex.: RHS do destructuring):
    /// contem NUL, impossivel no fonte (contador compartilhado com o `for`).
    fn hidden_name(&mut self, what: &str) -> String {
        let id = self.for_counter;
        self.for_counter += 1;
        format!("\0tmp:{what}:{id}")
    }

    /// Nome de closure anonima (`{closure:N}`; item 1): unico por programa e
    /// deterministico (ordem de travessia, com ida-e-volta em corpos
    /// aninhados), para o worker async resolver a definicao exata pelo nome.
    fn fresh_closure_name(&mut self) -> String {
        let id = self.closure_seq;
        self.closure_seq += 1;
        format!("{{closure:{id}}}")
    }

    /// Compila `for VAR in ITER BLOCO`.
    ///
    /// Estrategia (sem runtime novo): avalia `ITER` uma vez, normaliza com
    /// `IterList` (array = copia; mapa = chaves ordenadas) e itera por
    /// indice, como um `while` com incremento. `VAR` e declarada a cada
    /// iteracao no escopo do corpo. `break` fecha corpo + laco; `continue`
    /// salta ao incremento.
    fn compile_for(
        &mut self,
        var: &str,
        iter: &Expr,
        body: &[StmtNode],
    ) -> Result<(), CompileError> {
        self.emit_scope_open();
        let for_depth = self.scopes.len();
        // Avalia o iteravel UMA vez e guarda em H_iter (RFC 0018: gerador
        // ou colecao; o teste de tipo abaixo escolhe a forma do laco).
        let iter_name = self.hidden_for_name("iter");
        self.compile_expr(iter)?;
        let iter_idx = self.intern_name(&iter_name)?;
        self.emit(Op::DeclareName(iter_idx));
        // Marca da forma per-iteracao (false = lazy, true = eager),
        // declarada no escopo do laco: o corpo e compilado UMA vez e o
        // `continue` despacha em runtime (correcao COMP-001: antes o corpo
        // era compilado duas vezes, custo 2^N em `for` aninhados).
        let eager_flag = self.hidden_for_name("eager");
        let flag_off = self.add_constant(Value::Bool(false))?;
        self.emit(Op::Constant(flag_off));
        let flag_idx = self.intern_name(&eager_flag)?;
        self.emit(Op::DeclareName(flag_idx));
        // if type(H_iter) == "gerador": forma lazy; senao, eager.
        let iter_ref = self.intern_name(&iter_name)?;
        self.emit(Op::LoadName(iter_ref));
        self.emit(Op::CallBuiltin(BuiltinId::from(Builtin::Type)));
        let gerador_idx = self.add_constant(Value::Str("gerador".to_string()))?;
        self.emit(Op::Constant(gerador_idx));
        self.emit(Op::Eq);
        let eager_jump = self.emit(Op::JumpIfFalse(u32::MAX));
        // ---- Forma lazy (gerador).
        // while true { tmp = next; if exhausted { break }; x = tmp; BODY }
        // O `exhausted` e testado ANTES (gerador ja esgotado = zero
        // iteracoes) e DEPOIS de cada `next` (o gerador so termina
        // *durante* um resume; o valor final e descartado). `tmp` usa
        // `StoreName` (sem redeclare) para nao exigir segundo `close`
        // (o rastreamento estatico de escopos e linear).
        let tmp_name = self.hidden_for_name("prox");
        let tmp_idx = self.intern_name(&tmp_name)?;
        let tmp_null = self.add_constant(Value::Null)?;
        self.emit(Op::Constant(tmp_null));
        self.emit(Op::DeclareName(tmp_idx));
        let iter_is_done = self.intern_name(&iter_name)?;
        self.emit(Op::LoadName(iter_is_done));
        self.emit(Op::CallBuiltin(BuiltinId::from(Builtin::Exhausted)));
        let lazy_exit = self.emit(Op::JumpIfTrue(u32::MAX));
        let lazy_start = self.chunk.code.len() as u32;
        let iter_next = self.intern_name(&iter_name)?;
        self.emit(Op::LoadName(iter_next));
        self.emit(Op::CallBuiltin(BuiltinId::from(Builtin::Next)));
        let tmp_store = self.intern_name(&tmp_name)?;
        self.emit(Op::StoreName(tmp_store));
        let iter_done = self.intern_name(&iter_name)?;
        self.emit(Op::LoadName(iter_done));
        self.emit(Op::CallBuiltin(BuiltinId::from(Builtin::Exhausted)));
        let lazy_done_jump = self.emit(Op::JumpIfTrue(u32::MAX));
        // Valor da variavel + marca da forma (lazy): o corpo unico abaixo
        // declara `x` e executa; `continue` volta aqui via despacho.
        let tmp_load = self.intern_name(&tmp_name)?;
        self.emit(Op::LoadName(tmp_load));
        let flag_lazy = self.add_constant(Value::Bool(false))?;
        self.emit(Op::Constant(flag_lazy));
        let flag_lazy_store = self.intern_name(&eager_flag)?;
        self.emit(Op::StoreName(flag_lazy_store));
        let to_shared_lazy = self.emit(Op::Jump(u32::MAX));
        // ---- Forma eager (array/mapa): a de antes, sobre H_iter.
        // (JumpIfTrue nao existe; o salto condicional acima ja desviou.)
        let eager_label = self.chunk.code.len() as u32;
        self.patch_jump(eager_jump, eager_label);
        let list_name = self.hidden_for_name("lista");
        let idx_name = self.hidden_for_name("indice");
        let iter_eager = self.intern_name(&iter_name)?;
        self.emit(Op::LoadName(iter_eager));
        self.emit(Op::IterList);
        let list_idx = self.intern_name(&list_name)?;
        self.emit(Op::DeclareName(list_idx));
        let zero = self.add_constant(Value::Int(0))?;
        self.emit(Op::Constant(zero));
        let idx_pos = self.intern_name(&idx_name)?;
        self.emit(Op::DeclareName(idx_pos));
        let loop_start = self.chunk.code.len() as u32;
        // Condicao: H_idx < len(H_list).
        let idx_ref = self.intern_name(&idx_name)?;
        self.emit(Op::LoadName(idx_ref));
        let list_ref = self.intern_name(&list_name)?;
        self.emit(Op::LoadName(list_ref));
        self.emit(Op::CallBuiltin(BuiltinId::from(Builtin::Len)));
        self.emit(Op::Lt);
        let exit_jump = self.emit(Op::JumpIfFalse(u32::MAX));
        // VAR = H_list[H_idx].
        let list_elem = self.intern_name(&list_name)?;
        self.emit(Op::LoadName(list_elem));
        let idx_elem = self.intern_name(&idx_name)?;
        self.emit(Op::LoadName(idx_elem));
        self.emit(Op::Index);
        // Valor da variavel + marca da forma (eager): corpo unico abaixo.
        let flag_eager = self.add_constant(Value::Bool(true))?;
        self.emit(Op::Constant(flag_eager));
        let flag_eager_store = self.intern_name(&eager_flag)?;
        self.emit(Op::StoreName(flag_eager_store));
        let to_shared_eager = self.emit(Op::Jump(u32::MAX));
        // ---- Corpo unico: as duas formas convergem aqui (COMP-001).
        // Escopo do corpo: o `DeclareName` de `x` cai dentro dele, nao no
        // escopo do laco/global. Um so LoopCtx: `break` (das duas formas)
        // converge no fim; `continue` vai ao despacho abaixo.
        let shared_label = self.chunk.code.len() as u32;
        self.patch_jump(to_shared_lazy, shared_label);
        self.patch_jump(to_shared_eager, shared_label);
        self.emit_scope_open();
        let var_idx = self.intern_name(var)?;
        self.emit(Op::DeclareName(var_idx));
        let shared_open = self.scopes.clone();
        self.loops.push(LoopCtx {
            is_loop: true,
            break_depth: for_depth,
            continue_depth: self.scopes.len(),
            start: None,
            breaks: Vec::new(),
            continues: Vec::new(),
        });
        self.compile_stmts(body)?;
        self.scopes = shared_open.clone();
        // Saida normal do corpo: fecha o escopo e pede a proxima iteracao
        // (despacho sem fechar de novo: o fechamento ja foi feito aqui).
        self.emit_scope_close()?;
        let to_next = self.emit(Op::Jump(u32::MAX));
        // Trampolim do `continue`: fecha o escopo do corpo e despacha pela
        // marca (eager incrementa e volta ao teste; lazy volta ao `next`).
        let continue_dispatch = self.chunk.code.len() as u32;
        self.scopes = shared_open;
        self.emit_scope_close()?;
        let flag_load = self.intern_name(&eager_flag)?;
        self.emit(Op::LoadName(flag_load));
        let to_eager_cont = self.emit(Op::JumpIfTrue(u32::MAX));
        self.emit(Op::Jump(lazy_start));
        // Proxima iteracao (fallthrough): mesma decisao, sem fechar.
        let shared_next = self.chunk.code.len() as u32;
        self.patch_jump(to_next, shared_next);
        let flag_again = self.intern_name(&eager_flag)?;
        self.emit(Op::LoadName(flag_again));
        let to_eager_inc = self.emit(Op::JumpIfTrue(u32::MAX));
        self.emit(Op::Jump(lazy_start));
        // Ramo eager (`continue` ou fallthrough): H_idx++; volta ao teste.
        let eager_inc = self.chunk.code.len() as u32;
        self.patch_jump(to_eager_cont, eager_inc);
        self.patch_jump(to_eager_inc, eager_inc);
        let idx_inc = self.intern_name(&idx_name)?;
        self.emit(Op::LoadName(idx_inc));
        let one = self.add_constant(Value::Int(1))?;
        self.emit(Op::Constant(one));
        self.emit(Op::Add);
        let idx_store = self.intern_name(&idx_name)?;
        self.emit(Op::StoreName(idx_store));
        self.emit(Op::Jump(loop_start));
        // Fim: saidas das formas (`lazy_exit`, `exit_jump`, gerador
        // esgotado no resume) e `break`s convergem aqui; fecha o laco.
        let post_label = self.chunk.code.len() as u32;
        self.patch_jump(lazy_exit, post_label);
        self.patch_jump(exit_jump, post_label);
        self.patch_jump(lazy_done_jump, post_label);
        let Some(loop_ctx) = self.loops.pop() else {
            return Err(CompileError::new(
                "desequilibrio interno de lacos".to_string(),
            ));
        };
        for pos in &loop_ctx.breaks {
            self.patch_jump(*pos, post_label);
        }
        for pos in &loop_ctx.continues {
            self.patch_jump(*pos, continue_dispatch);
        }
        self.emit_scope_close()?;
        Ok(())
    }

    /// Emite copia do `finally` em escopo proprio (codigo de saida de
    /// `try`: caminho normal ou desvio abrupto). Codigo de corpo e neutro
    /// na pilha de operandos; `break`/`continue`/`return` dentro da copia
    /// nao disparam nova duplicacao (substituem o fluxo em curso).
    fn emit_finally_copy(&mut self, body: &[StmtNode]) -> Result<(), CompileError> {
        self.emit_scope_open();
        self.emitting_finally += 1;
        let saved = self.scopes.clone();
        self.compile_stmts(body)?;
        self.scopes = saved;
        self.emitting_finally -= 1;
        self.emit_scope_close()?;
        Ok(())
    }

    /// Fecha handlers e roda `finally`s de `try`s cruzados por um desvio
    /// abrupto (`break`/`continue`/`return`), do mais interno para fora.
    /// `TryEnd` so e emitido para `try`s com handler ativo (no caminho de
    /// `catch`, o unwinding ja o consumiu); copias de `finally` sempre
    /// (exceto dentro de copia, onde o desvio substitui o fluxo).
    /// `crossing_loop`: filtra `try`s abertos dentro do laco-alvo
    /// (`break`/`continue`); `return` cruza todos (`None` = sem filtro).
    fn emit_abrupt_exits(&mut self, crossing_loop: Option<usize>) -> Result<(), CompileError> {
        let guarded = self.emitting_finally > 0;
        // Do mais interno para fora. `TryEnd` marca o handler como
        // consumido NESTE caminho: copias de `finally` emitidas a seguir
        // (que podem conter novos desvios) nao o repetem.
        let mut copies: Vec<Vec<StmtNode>> = Vec::new();
        let mut i = self.pending_trys.len();
        while i > 0 {
            i -= 1;
            // `depth` = indice do laco-alvo + 1: cruzam os `try`s abertos
            // dentro dele (profundidade maior). Para desvios que miram o
            // topo sem `switch` no caminho, equivale ao `==` anterior.
            let apply = match crossing_loop {
                Some(depth) => self.pending_trys[i].loop_depth >= depth,
                None => true,
            };
            if !apply {
                continue;
            }
            if self.pending_trys[i].handler_live {
                self.emit(Op::TryEnd);
                self.pending_trys[i].handler_live = false;
            }
            if !guarded {
                if let Some(body) = self.pending_trys[i].finally_body.clone() {
                    copies.push(body);
                }
            }
        }
        for body in copies {
            self.emit_finally_copy(&body)?;
        }
        Ok(())
    }

    /// `try` com multiplos `catch` tipados (Fase Next-Gen 4).
    /// Sem tipo = captura tudo (legado). Com tipo = `IsInstanceOf` + salto.
    /// Ordem: primeiro match vence; `finally` sempre respeitado.
    fn compile_try_multi(
        &mut self,
        body: &[StmtNode],
        catches: &[rhp_parser::CatchClause],
        finally_body: &Option<Vec<StmtNode>>,
    ) -> Result<(), CompileError> {
        let handler_jump = self.emit(Op::TryBegin(u32::MAX));
        self.emit_scope_open();
        self.pending_trys.push(PendingTry {
            finally_body: finally_body.clone(),
            handler_live: true,
            loop_depth: self.loops.len(),
        });
        let saved = self.scopes.clone();
        self.compile_stmts(body)?;
        self.scopes = saved;
        self.emit_scope_close()?;
        self.emit(Op::TryEnd);
        if let Some(top) = self.pending_trys.last_mut() {
            top.handler_live = false;
        }
        if let Some(fin) = finally_body {
            self.emit_finally_copy(fin)?;
        }
        let end_jump = self.emit(Op::Jump(u32::MAX));
        let handler_target = self.chunk.code.len() as u32;
        self.patch_jump(handler_jump, handler_target);
        if catches.is_empty() {
            if let Some(fin) = finally_body {
                self.emit_finally_copy(fin)?;
            }
            self.emit(Op::Rethrow);
        } else {
            // Cadeia de catches: cada um testa (se tipado) e consome ou repassa.
            let mut next_jumps: Vec<usize> = Vec::new();
            for (i, catch) in catches.iter().enumerate() {
                let is_last = i + 1 == catches.len();
                // Pilha aqui: [erro]. Cada catch nao-final deixa [erro] para o proximo.
                if let Some(tname) = catch.type_name.as_ref() {
                    let qtype = self.resolve_type_name(tname);
                    self.emit(Op::Dup);
                    let type_idx = self.intern_name(&qtype)?;
                    self.emit(Op::IsInstanceOf(type_idx));
                    let to_next = self.emit(Op::JumpIfFalse(u32::MAX));
                    // Match: consome a copia? Dup+IsInstanceOf consumiu uma copia
                    // e empilhou Bool; JumpIfFalse desempilhou o Bool. Resta [erro].
                    // Vincula var ou dispensa e executa o corpo.
                    self.emit_catch_body(catch, finally_body)?;
                    let to_end = self.emit(Op::Jump(u32::MAX));
                    let next_target = self.chunk.code.len() as u32;
                    self.patch_jump(to_next, next_target);
                    // Nao-match: [erro] permanece para o proximo catch.
                    // Se for o ultimo e nao casou, repassa (rethrow).
                    if is_last {
                        // [erro] -> finally + rethrow.
                        if let Some(fin) = finally_body {
                            // `finally` em caminho de erro: precisa do valor?
                            // `emit_finally_copy` nao toca na pilha de operandos
                            // (neutra); o [erro] segue abaixo dela.
                            self.emit_finally_copy(fin)?;
                        }
                        self.emit(Op::Rethrow);
                    }
                    // Remenda o salto do corpo para o fim (apos toda a cadeia).
                    next_jumps.push(to_end);
                } else {
                    // Sem tipo: captura tudo (legado). Consome [erro].
                    self.emit_catch_body(catch, finally_body)?;
                    let to_end = self.emit(Op::Jump(u32::MAX));
                    next_jumps.push(to_end);
                    // Catches apos um catch sem tipo sao inalancaveis: erro claro.
                    if !is_last {
                        return Err(CompileError::new(
                            "`catch` sem tipo deve ser o ultimo (ele captura tudo)".to_string(),
                        ));
                    }
                }
            }
            let end_target = self.chunk.code.len() as u32;
            for pos in next_jumps {
                self.patch_jump(pos, end_target);
            }
        }
        let end_target = self.chunk.code.len() as u32;
        self.patch_jump(end_jump, end_target);
        self.pending_trys.pop();
        Ok(())
    }

    /// Corpo de um `catch` (vinculo + stmts + finally + fechamento).
    fn emit_catch_body(
        &mut self,
        catch: &rhp_parser::CatchClause,
        finally_body: &Option<Vec<StmtNode>>,
    ) -> Result<(), CompileError> {
        self.emit_scope_open();
        match catch.var.as_ref() {
            Some(name) => {
                let name_idx = self.intern_name(name)?;
                self.emit(Op::DeclareName(name_idx));
            }
            None => {
                self.emit(Op::Pop);
            }
        }
        let saved_catch = self.scopes.clone();
        self.compile_stmts(&catch.body)?;
        self.scopes = saved_catch;
        if let Some(fin) = finally_body {
            self.emit_finally_copy(fin)?;
        }
        self.emit_scope_close()?;
        Ok(())
    }

    /// Resolve nome de tipo para forma qualificada (namespace atual).
    fn resolve_type_name(&self, name: &str) -> String {
        self.qualify(name)
    }

    /// Resolve nome de valor (variavel/funcao/classe) para forma qualificada.
    /// Se nao-qualificado, namespace ativo e a forma qualificada ja foi
    /// definida antes neste chunk, usa a qualificada; senao mantem.
    fn resolve_name(&self, name: &str) -> String {
        if name.contains('\\') {
            return name.to_string();
        }
        if builtin_by_name(name).is_some() {
            return name.to_string();
        }
        if let Some(ns) = self.current_namespace.as_ref() {
            let q = format!("{ns}\\{name}");
            if self.classes.contains_key(&q)
                || self.interfaces.contains_key(&q)
                || self.traits.contains_key(&q)
            {
                return q;
            }
            // Funcoes definidas? Rastreamos via `names`? `names` tem todas as
            // globais, inclusive qualificadas. Se `q` ja foi internado, e forte
            // indicio de que a definicao prefixada veio antes.
            if self.names.contains_key(&q) {
                return q;
            }
        }
        name.to_string()
    }

    /// Bound deve nomear classe ou interface conhecida (definida antes).
    /// Falha rapida na definicao generica (item bounds, RFC 0029).
    fn check_bound_known(&self, bound: &str, owner: &str) -> Result<(), CompileError> {
        if self.classes.contains_key(bound) || self.interfaces.contains_key(bound) {
            return Ok(());
        }
        Err(CompileError::new(format!(
            "bound `{bound}` desconhecido em `{owner}` (use classe ou interface definida antes)"
        )))
    }

    /// `true` se `child` e `base` ou subclasse (cadeia `parent`).
    /// Limite anti-ciclo para bytecode artesanal (hierarquia valida nao tem).
    fn is_subclass_of(&self, child: &str, base: &str) -> bool {
        // VM-006: sem teto de 256 (ver copia da VM): cadeia de `extends` e
        // aciclica por construcao; revisita = ciclo, tratada como fim.
        let mut current = Some(child.to_string());
        let mut visited: Vec<String> = Vec::new();
        loop {
            match current {
                Some(name) => {
                    if name == base {
                        return true;
                    }
                    if visited.iter().any(|n| n == &name) {
                        return false;
                    }
                    visited.push(name.clone());
                    current = self.classes.get(&name).and_then(|i| i.parent.clone());
                }
                None => return false,
            }
        }
    }

    /// `true` se a classe (com herdados) implementa a interface com aridade.
    /// Mesma regra do `implements` (reuso na checagem de bounds).
    fn class_implements(&self, class: &str, iface: &str) -> bool {
        let Some(sigs) = self.interfaces.get(iface) else {
            return false;
        };
        let Some(info) = self.classes.get(class) else {
            return false;
        };
        sigs.iter().all(|sig| match info.methods.get(&sig.name) {
            Some(m) => m.params == sig.params && m.variadic == sig.variadic,
            None => false,
        })
    }

    /// Checa argumentos de tipo no uso (`new Caixa<Sub>`, `f<T>(...)`).
    /// Apagamento mantido: so valida, nao emite nada (item bounds).
    /// Sem argumentos de tipo = chamada apagada (legado, sem checagem).
    fn check_type_args(
        &mut self,
        target: &str,
        declared: &[GenericParam],
        args: &[String],
        kind: &str,
    ) -> Result<(), CompileError> {
        if args.is_empty() {
            return Ok(());
        }
        if declared.is_empty() {
            return Err(CompileError::new(format!(
                "`{target}` nao e {kind} generica mas recebeu {} argumento(s) de tipo",
                args.len(),
            )));
        }
        if args.len() != declared.len() {
            return Err(CompileError::new(format!(
                "`{target}` espera {} argumento(s) de tipo, recebeu {}",
                declared.len(),
                args.len(),
            )));
        }
        for (param, arg) in declared.iter().zip(args.iter()) {
            // Parametro de tipo visivel = adiado para a instanciacao concreta.
            if self.type_params.iter().any(|t| t == arg) {
                continue;
            }
            let Some(bound) = param.bound.as_ref() else {
                continue; // sem bound: qualquer tipo serve (apagado).
            };
            let qarg = self.resolve_name(arg);
            let known_class = self.classes.contains_key(&qarg);
            let satisfied = if self.classes.contains_key(bound) {
                // Bound de classe: mesmo tipo ou subclasse.
                known_class && self.is_subclass_of(&qarg, bound)
            } else if self.interfaces.contains_key(bound) {
                // Bound de interface: classe que a implementa.
                known_class && self.class_implements(&qarg, bound)
            } else {
                false // bound sumiu da tabela (nao deveria ocorrer).
            };
            if !satisfied {
                return Err(CompileError::new(format!(
                    "`{arg}` nao satisfaz `{}: {bound}` em `{target}`",
                    param.name,
                )));
            }
        }
        Ok(())
    }

    /// `class Nome<T> extends Pai implements I... { use T...; fn ... }`.
    /// Precedencia: propria > trait > pai. Valida interfaces e conflitos.
    fn compile_class(
        &mut self,
        name: &str,
        parent: &Option<String>,
        interfaces: &[String],
        uses: &[String],
        generic_params: &[GenericParam],
        methods: &[rhp_parser::ClassMethod],
    ) -> Result<(), CompileError> {
        let qname = self.qualify(name);
        // Bounds: cada um deve nomear classe ou interface conhecida.
        // Armazena qualificados (namespace da definicao).
        let mut stored: Vec<GenericParam> = Vec::with_capacity(generic_params.len());
        for gp in generic_params.iter() {
            let bound = match gp.bound.as_ref() {
                Some(b) => {
                    let qb = self.qualify(b);
                    self.check_bound_known(&qb, &qname)?;
                    Some(qb)
                }
                None => None,
            };
            stored.push(GenericParam {
                name: gp.name.clone(),
                bound,
            });
        }
        // Pai: deve existir (definido antes) e nao formar ciclo.
        let qparent: Option<String> = match parent.as_ref() {
            Some(p) => {
                let qp = self.qualify(p);
                if qp == qname {
                    return Err(CompileError::new(format!(
                        "classe `{qname}` nao pode herdar de si mesma"
                    )));
                }
                let pinfo = self.classes.get(&qp).ok_or_else(|| {
                    CompileError::new(format!(
                        "classe pai `{qp}` desconhecida (defina antes de `{qname}`)"
                    ))
                })?;
                // Ciclo A->B->A: como o pai deve existir antes, basta checar se
                // o pai (transitivamente) ja e o filho (impossivel sem forward,
                // mas checa a cadeia por seguranca).
                let mut chain = pinfo.parent.clone();
                while let Some(cur) = chain {
                    if cur == qname {
                        return Err(CompileError::new(format!(
                            "hierarquia circular envolvendo `{qname}`"
                        )));
                    }
                    chain = self.classes.get(&cur).and_then(|i| i.parent.clone());
                }
                Some(qp)
            }
            None => None,
        };
        // Metodos proprios: compila corpos, detecta duplicata (parse ja fez).
        let mut own: std::collections::HashMap<String, (u32, usize, bool)> =
            std::collections::HashMap::new();
        let class_types: Vec<String> = stored.iter().map(|g| g.name.clone()).collect();
        for method in methods {
            let func_idx = self.compile_func_body(
                &format!("{qname}::{}", method.name),
                &method.params,
                method.variadic.as_deref(),
                method.return_ann,
                &method.body,
                method.is_async,
                &class_types,
            )?;
            if own
                .insert(
                    method.name.clone(),
                    (func_idx, method.params.len(), method.variadic.is_some()),
                )
                .is_some()
            {
                return Err(CompileError::new(format!(
                    "metodo duplicado `{}` na classe `{qname}`",
                    method.name
                )));
            }
        }
        // Traits: `use` compoe; conflito trait-trait sem override = erro.
        let mut from_traits: std::collections::HashMap<String, (u32, usize, bool, String)> =
            std::collections::HashMap::new();
        for tname in uses {
            let qt = self.qualify(tname);
            let tmethods = self.traits.get(&qt).cloned().ok_or_else(|| {
                CompileError::new(format!(
                    "trait `{qt}` desconhecido (defina antes de `{qname}`)"
                ))
            })?;
            for tm in tmethods {
                if own.contains_key(&tm.name) {
                    continue; // propria vence trait.
                }
                if let Some((_, _, _, other)) = from_traits.get(&tm.name) {
                    if other != &qt {
                        return Err(CompileError::new(format!(
                            "conflito de trait: metodo `{}` vem de `{}` e `{}` (resolva com override em `{qname}`)",
                            tm.name, other, qt
                        )));
                    }
                    continue;
                }
                from_traits.insert(
                    tm.name.clone(),
                    (tm.func_idx, tm.params, tm.variadic, qt.clone()),
                );
            }
        }
        // Pai: herda metodos nao-sobrescritos (propria > trait > pai).
        let mut inherited: Vec<(String, u32)> = Vec::new();
        if let Some(qp) = qparent.as_ref() {
            let pinfo = self.classes.get(qp).expect("pai verificado");
            for (mname, sig) in pinfo.methods.iter() {
                if own.contains_key(mname) || from_traits.contains_key(mname) {
                    continue;
                }
                inherited.push((mname.clone(), sig.func_idx));
            }
        }
        // Interfaces: todos os metodos exigidos devem existir (proprio/trait/pai)
        // com aridade compativel.
        for iname in interfaces {
            let qi = self.qualify(iname);
            let sigs = self.interfaces.get(&qi).cloned().ok_or_else(|| {
                CompileError::new(format!(
                    "interface `{qi}` desconhecida (defina antes de `{qname}`)"
                ))
            })?;
            for sig in sigs {
                let found = own
                    .get(&sig.name)
                    .map(|(_, p, v)| (*p, *v))
                    .or_else(|| from_traits.get(&sig.name).map(|(_, p, v, _)| (*p, *v)))
                    .or_else(|| {
                        qparent.as_ref().and_then(|qp| {
                            self.classes
                                .get(qp)
                                .and_then(|pi| pi.methods.get(&sig.name))
                                .map(|s| (s.params, s.variadic))
                        })
                    });
                match found {
                    Some((p, v)) => {
                        if p != sig.params || v != sig.variadic {
                            return Err(CompileError::new(format!(
                                "classe `{qname}` nao implementa `{qi}::{}` com aridade compativel",
                                sig.name
                            )));
                        }
                    }
                    None => {
                        return Err(CompileError::new(format!(
                            "classe `{qname}` nao implementa metodo `{}` de `{qi}`",
                            sig.name
                        )));
                    }
                }
            }
        }
        // Emite: herdados (pai) + traits + proprios. Ordem irrelevante (mapa).
        let mut total = 0u32;
        for (mname, fidx) in &inherited {
            let midx = self.add_constant(Value::Str(mname.clone()))?;
            self.emit(Op::Constant(midx));
            self.emit(Op::MakeFunc(*fidx));
            total += 1;
        }
        // `from_traits`/`own`: itera por referencia para reusar abaixo.
        let trait_list: Vec<(String, u32, usize, bool)> = from_traits
            .iter()
            .map(|(k, (f, p, v, _))| (k.clone(), *f, *p, *v))
            .collect();
        for (mname, fidx, _, _) in &trait_list {
            let midx = self.add_constant(Value::Str(mname.clone()))?;
            self.emit(Op::Constant(midx));
            self.emit(Op::MakeFunc(*fidx));
            total += 1;
        }
        let own_list: Vec<(String, u32, usize, bool)> = own
            .iter()
            .map(|(k, (f, p, v))| (k.clone(), *f, *p, *v))
            .collect();
        for (mname, fidx, _, _) in &own_list {
            let midx = self.add_constant(Value::Str(mname.clone()))?;
            self.emit(Op::Constant(midx));
            self.emit(Op::MakeFunc(*fidx));
            total += 1;
        }
        let count = total;
        // Registra a classe achatada para filhos/interfaces/catch.
        let mut flat: std::collections::HashMap<String, SigInfoEx> =
            std::collections::HashMap::new();
        if let Some(qp) = qparent.as_ref() {
            if let Some(pi) = self.classes.get(qp) {
                for (k, v) in pi.methods.iter() {
                    flat.insert(k.clone(), v.clone());
                }
            }
        }
        for (mname, fidx, p, v) in trait_list {
            flat.insert(
                mname,
                SigInfoEx {
                    params: p,
                    variadic: v,
                    func_idx: fidx,
                },
            );
        }
        for (mname, fidx, p, v) in own_list {
            flat.insert(
                mname,
                SigInfoEx {
                    params: p,
                    variadic: v,
                    func_idx: fidx,
                },
            );
        }
        if self.classes.contains_key(&qname) {
            return Err(CompileError::new(format!("classe `{qname}` ja declarada")));
        }
        self.classes.insert(
            qname.clone(),
            ClassInfo {
                parent: qparent.clone(),
                methods: flat,
                generics: stored,
            },
        );
        // Emite construcao: com pai usa `BuildClassEx`, senao `BuildClass`.
        let name_idx = self.intern_name(&qname)?;
        if let Some(qp) = qparent.as_ref() {
            let parent_idx = self.intern_name(qp)?;
            self.emit(Op::BuildClassEx(name_idx, parent_idx, count));
        } else {
            self.emit(Op::BuildClass(name_idx, count));
        }
        self.emit(Op::DeclareName(name_idx));
        Ok(())
    }

    /// `interface Nome<T> { fn m(params); ... }`: so assinaturas, sem runtime.
    /// Registra para checagem de `implements`; redefinicao = erro.
    fn compile_interface(
        &mut self,
        name: &str,
        generic_params: &[GenericParam],
        methods: &[rhp_parser::InterfaceMethod],
    ) -> Result<(), CompileError> {
        let qname = self.qualify(name);
        if self.interfaces.contains_key(&qname)
            || self.classes.contains_key(&qname)
            || self.traits.contains_key(&qname)
        {
            return Err(CompileError::new(format!(
                "interface `{qname}` ja declarada"
            )));
        }
        for gp in generic_params.iter() {
            if let Some(bound) = gp.bound.as_ref() {
                let qb = self.qualify(bound);
                self.check_bound_known(&qb, &qname)?;
            }
        }
        let mut sigs = Vec::with_capacity(methods.len());
        for m in methods {
            sigs.push(SigInfo {
                name: m.name.clone(),
                params: m.params.len(),
                variadic: m.variadic.is_some(),
            });
        }
        self.interfaces.insert(qname, sigs);
        Ok(())
    }

    /// `trait Nome { fn m() { } }`: compila corpos para reuso via `use`.
    /// Sem runtime proprio (so registro); redefinicao = erro.
    fn compile_trait(
        &mut self,
        name: &str,
        methods: &[rhp_parser::ClassMethod],
    ) -> Result<(), CompileError> {
        let qname = self.qualify(name);
        if self.traits.contains_key(&qname)
            || self.classes.contains_key(&qname)
            || self.interfaces.contains_key(&qname)
        {
            return Err(CompileError::new(format!("trait `{qname}` ja declarado")));
        }
        let mut infos = Vec::with_capacity(methods.len());
        for m in methods {
            let func_idx = self.compile_func_body(
                &format!("{qname}::{}", m.name),
                &m.params,
                m.variadic.as_deref(),
                m.return_ann,
                &m.body,
                m.is_async,
                &[],
            )?;
            infos.push(TraitMethodInfo {
                name: m.name.clone(),
                params: m.params.len(),
                variadic: m.variadic.is_some(),
                func_idx,
            });
        }
        self.traits.insert(qname, infos);
        Ok(())
    }

    fn compile_if(
        &mut self,
        cond: &Expr,
        then_branch: &[StmtNode],
        else_branch: &Option<Vec<StmtNode>>,
    ) -> Result<(), CompileError> {
        self.compile_expr(cond)?;
        let else_jump = self.emit(Op::JumpIfFalse(u32::MAX));
        self.emit_scope_open();
        self.compile_stmts(then_branch)?;
        // Ramo que sempre diverge (break/continue/return) nao precisa do
        // fechamento: o desenrolamento ja removeu o escopo e o controle
        // jamais alcanca este ponto por fall-through.
        let end_jump = if diverges(then_branch) {
            None
        } else {
            self.emit_scope_close()?;
            Some(self.emit(Op::Jump(u32::MAX)))
        };
        let else_target = self.chunk.code.len() as u32;
        self.patch_jump(else_jump, else_target);
        if let Some(else_stmts) = else_branch {
            self.emit_scope_open();
            self.compile_stmts(else_stmts)?;
            if !diverges(else_stmts) {
                self.emit_scope_close()?;
            }
        }
        let end_target = self.chunk.code.len() as u32;
        if let Some(end_jump) = end_jump {
            self.patch_jump(end_jump, end_target);
        }
        Ok(())
    }

    /// Compila `break [N];` (RFC 0008): sai dos N breakables mais internos
    /// (laco ou `switch`). N alem do aninhamento = erro de compilacao.
    /// Fecha escopos e `try`s cruzados; dispensa um discriminante por
    /// `switch` cruzado (o alvo espera a altura sem eles); o dono do
    /// contexto remenda o salto (lacos: saida; `switch`: apos o `Pop`).
    fn compile_break(&mut self, n: u32) -> Result<(), CompileError> {
        let n = n as usize;
        if n == 0 || n > self.loops.len() {
            return Err(CompileError::new(if self.loops.is_empty() {
                "`break` fora de laco".to_string()
            } else {
                format!(
                    "`break {n}` alem do aninhamento (so ha {} niveis)",
                    self.loops.len()
                )
            }));
        }
        self.compile_break_at(self.loops.len() - n)
    }

    /// Nucleo do desvio de saida para o contexto `p` (laco ou `switch`):
    /// fecha escopos/`try`s, dispensa discriminantes de `switch` cruzados
    /// (incluindo o do proprio alvo se for `switch`) e remenda no dono.
    fn compile_break_at(&mut self, p: usize) -> Result<(), CompileError> {
        let break_depth = self.loops[p].break_depth;
        self.emit_abrupt_exits(Some(p + 1))?;
        while self.scopes.len() > break_depth {
            self.emit_scope_close()?;
        }
        let crossed = self.loops[p..].iter().filter(|ctx| !ctx.is_loop).count();
        for _ in 0..crossed {
            self.emit(Op::Pop);
        }
        let pos = self.emit(Op::Jump(u32::MAX));
        self.loops[p].breaks.push(pos);
        Ok(())
    }

    /// Compila `continue [N];` (RFC 0008): conta N breakables para fora
    /// (laco ou `switch`, como no PHP). Caindo em laco: continua-o (salto
    /// ao ponto de continuacao, com `Pop` por `switch` cruzado). Caindo em
    /// `switch`: equivale a `break` dele. N alem do aninhamento = erro.
    fn compile_continue(&mut self, n: u32) -> Result<(), CompileError> {
        let mut target = None;
        let mut seen = 0u32;
        for (i, _) in self.loops.iter().enumerate().rev() {
            seen += 1;
            if seen == n {
                target = Some(i);
                break;
            }
        }
        let Some(pos) = target else {
            return Err(CompileError::new(if self.loops.is_empty() {
                "`continue` fora de laco".to_string()
            } else {
                format!(
                    "`continue {n}` alem do aninhamento (so ha {} niveis)",
                    self.loops.len()
                )
            }));
        };
        if !self.loops[pos].is_loop {
            // Caiu em `switch`: `continue` age como `break` (PHP).
            return self.compile_break_at(pos);
        }
        let (continue_depth, start) = {
            let loop_ctx = &self.loops[pos];
            (loop_ctx.continue_depth, loop_ctx.start)
        };
        self.emit_abrupt_exits(Some(pos + 1))?;
        while self.scopes.len() > continue_depth {
            self.emit_scope_close()?;
        }
        let crossed = self.loops[pos + 1..]
            .iter()
            .filter(|ctx| !ctx.is_loop)
            .count();
        for _ in 0..crossed {
            self.emit(Op::Pop);
        }
        match start {
            Some(target) => {
                self.emit(Op::Jump(target));
            }
            None => {
                let jump = self.emit(Op::Jump(u32::MAX));
                self.loops[pos].continues.push(jump);
            }
        }
        Ok(())
    }

    /// Compila `switch` (RFC 0004): discriminante avaliado uma vez;
    /// testes em ordem (`Dup + case + Eq`, `JumpIfFalse` para o proximo;
    /// `default` desvia incondicional), corpos em sequencia com fallthrough
    /// natural; `break` e o caminho sem-match convergem no `Pop` final
    /// (dispensa o discriminante, pilha balanceada). Nenhum op novo.
    fn compile_switch(
        &mut self,
        discriminant: &Expr,
        cases: &[SwitchCase],
    ) -> Result<(), CompileError> {
        self.compile_expr(discriminant)?;
        // Salva o rastreamento de escopos: `break`/`continue`/`return` nos
        // cases o alteram; restaurado no fim (pools e saltos mantidos).
        let saved_scopes = self.scopes.clone();
        // Contexto "breakable": `break` mira o `switch`; `continue` o ignora.
        self.loops.push(LoopCtx {
            is_loop: false,
            break_depth: self.scopes.len(),
            continue_depth: self.scopes.len(),
            start: None,
            breaks: Vec::new(),
            continues: Vec::new(),
        });
        // Bloco de testes: cada match desvia para o corpo (remendado apos).
        let mut body_jumps: Vec<usize> = Vec::with_capacity(cases.len());
        let mut next_test: Option<usize> = None;
        for case in cases {
            if let Some(pos) = next_test.take() {
                let here = self.chunk.code.len() as u32;
                self.patch_jump(pos, here);
            }
            match &case.cond {
                Some(cond) => {
                    self.emit(Op::Dup);
                    self.compile_expr(cond)?;
                    self.emit(Op::Eq);
                    next_test = Some(self.emit(Op::JumpIfFalse(u32::MAX)));
                    body_jumps.push(self.emit(Op::Jump(u32::MAX)));
                }
                // `default` casa quando alcancado (sem teste).
                None => {
                    body_jumps.push(self.emit(Op::Jump(u32::MAX)));
                }
            }
        }
        // Sem-match: remendado para o `Pop` apos os corpos (abaixo).
        // O bloco de testes nunca cai em sequencia (todo teste desvia).
        let no_match_jump = next_test.take();
        // Corpos em sequencia (fallthrough); cada teste remendado p/ o seu.
        for (case, jump) in cases.iter().zip(body_jumps.iter()) {
            let here = self.chunk.code.len() as u32;
            self.patch_jump(*jump, here);
            self.emit_scope_open();
            // Desvios no meio (`break; echo x`) baguncam o rastreamento;
            // corpo divergente nao cai aqui (volta a entrada, sem emitir).
            let case_saved = self.scopes.clone();
            self.compile_stmts(&case.body)?;
            if diverges(&case.body) {
                self.scopes = saved_scopes.clone();
            } else {
                self.scopes = case_saved;
                self.emit_scope_close()?;
            }
        }
        // `break` (com `Pop` proprio) chega sem discriminante e converge
        // no fim; fallthrough/sem-match o dispensam aqui. Cada salto mira
        // o ponto de altura correspondente.
        let pop_pos = self.chunk.code.len() as u32;
        if let Some(pos) = no_match_jump {
            self.patch_jump(pos, pop_pos);
        }
        self.emit(Op::Pop);
        let end_target = self.chunk.code.len() as u32;
        let Some(switch_ctx) = self.loops.pop() else {
            return Err(CompileError::new(
                "desequilibrio interno de lacos".to_string(),
            ));
        };
        for pos in &switch_ctx.breaks {
            self.patch_jump(*pos, end_target);
        }
        debug_assert!(
            switch_ctx.continues.is_empty(),
            "`continue` nunca mira `switch`"
        );
        self.scopes = saved_scopes;
        Ok(())
    }

    /// Compila chamada com nomeados/spread (RFC 0009): callee, valores em
    /// ordem do fonte e descritor nas constantes (`Int(0)` = posicional,
    /// `Int(1)` = spread, `Str` = nomeado); ligacao em runtime via
    /// `Op::CallMixed`. Chamadas so-posicionais usam o `Call` classico.
    fn compile_call_mixed(&mut self, callee: &Expr, args: &[CallArg]) -> Result<(), CompileError> {
        match callee {
            Expr::Var(name) => {
                let rname = self.resolve_name(name);
                let name_idx = self.intern_name(&rname)?;
                self.emit(Op::LoadName(name_idx));
            }
            other => {
                self.compile_expr(other)?;
            }
        }
        let desc_idx = self.emit_call_args(args)?;
        self.emit(Op::CallMixed(desc_idx));
        Ok(())
    }

    /// Emite os valores dos argumentos + descritor da chamada mista
    /// (`0` = posicional, `1` = spread, `Str` = nomeado); devolve o indice
    /// do descritor. Compartilhado por `CallMixed` e `NewMixed` (RFC 0017).
    fn emit_call_args(&mut self, args: &[CallArg]) -> Result<u32, CompileError> {
        for arg in args {
            self.compile_expr(arg.value())?;
        }
        if args.len() > u32::MAX as usize {
            return Err(CompileError::new("numero de argumentos excedido"));
        }
        let desc: Vec<Value> = args
            .iter()
            .map(|arg| match arg {
                CallArg::Pos(_) => Value::Int(0),
                CallArg::Spread(_) => Value::Int(1),
                CallArg::Named(name, _) => Value::Str(name.clone()),
            })
            .collect();
        self.add_constant(Value::Array(desc))
    }

    /// Compila `new Nome(args)` (RFC 0017): avalia a classe e os argumentos
    /// e emite `NewMixed` (mesmo descritor da chamada mista).
    fn compile_new(&mut self, class: &Expr, args: &[CallArg]) -> Result<(), CompileError> {
        self.compile_expr(class)?;
        let desc_idx = self.emit_call_args(args)?;
        self.emit(Op::NewMixed(desc_idx));
        Ok(())
    }

    /// Compila `match` (RFC 0008): sujeito avaliado uma vez; testes em
    /// ordem (`Dup + braco + StrictEq`, `JumpIfFalse` para o proximo;
    /// `default` desvia incondicional) e corpos-expressao em sequencia.
    /// Sem fallthrough: cada corpo descarta o sujeito (`Pop`), avalia o
    /// valor e salta ao fim. Sem match e sem `default`: `Pop` + `Throw`
    /// com a mensagem (capturavel por `try`, como o PHP lanca
    /// `UnhandledMatchError`). Nenhum op alem de `StrictEq`.
    fn compile_match(&mut self, subject: &Expr, arms: &[MatchArm]) -> Result<(), CompileError> {
        self.compile_expr(subject)?;
        let saved_scopes = self.scopes.clone();
        let mut body_jumps: Vec<usize> = Vec::with_capacity(arms.len());
        let mut next_test: Option<usize> = None;
        for arm in arms {
            if let Some(pos) = next_test.take() {
                let here = self.chunk.code.len() as u32;
                self.patch_jump(pos, here);
            }
            match &arm.cond {
                Some(cond) => {
                    self.emit(Op::Dup);
                    self.compile_expr(cond)?;
                    self.emit(Op::StrictEq);
                    next_test = Some(self.emit(Op::JumpIfFalse(u32::MAX)));
                    body_jumps.push(self.emit(Op::Jump(u32::MAX)));
                }
                None => {
                    body_jumps.push(self.emit(Op::Jump(u32::MAX)));
                }
            }
        }
        // Sem-match: dispensa o sujeito e lanca a mensagem (caminho terminal).
        let no_match = self.chunk.code.len() as u32;
        if let Some(pos) = next_test.take() {
            self.patch_jump(pos, no_match);
        }
        self.emit(Op::Pop);
        let msg_idx =
            self.add_constant(Value::Str("match sem braco correspondente".to_string()))?;
        self.emit(Op::Constant(msg_idx));
        self.emit(Op::Throw);
        // Corpos: cada teste remendado para o seu; valor como expressao.
        let mut end_jumps: Vec<usize> = Vec::with_capacity(arms.len());
        for (arm, jump) in arms.iter().zip(body_jumps.iter()) {
            let here = self.chunk.code.len() as u32;
            self.patch_jump(*jump, here);
            self.emit(Op::Pop);
            self.compile_expr(&arm.value)?;
            end_jumps.push(self.emit(Op::Jump(u32::MAX)));
        }
        let end_target = self.chunk.code.len() as u32;
        for pos in end_jumps {
            self.patch_jump(pos, end_target);
        }
        self.scopes = saved_scopes;
        Ok(())
    }

    /// Compila `for init; cond; passo { corpo }` (for-C, RFC 0008).
    /// Espelha `compile_for`: escopo proprio do conjunto (init nao vaza),
    /// `continue` salta ao passo (nao a condicao), `break` sai ao fim.
    /// `cond` ausente = sempre verdadeiro.
    fn compile_for_c(
        &mut self,
        init: &Option<Box<StmtNode>>,
        cond: &Option<Expr>,
        step: &Option<Box<StmtNode>>,
        body: &[StmtNode],
    ) -> Result<(), CompileError> {
        self.emit_scope_open();
        if let Some(init) = init {
            self.compile_stmt(&init.stmt)?;
        }
        let loop_start = self.chunk.code.len() as u32;
        let exit_jump = match cond {
            Some(cond) => {
                self.compile_expr(cond)?;
                Some(self.emit(Op::JumpIfFalse(u32::MAX)))
            }
            None => None,
        };
        self.emit_scope_open();
        let base_depth = self.scopes.len();
        self.loops.push(LoopCtx {
            is_loop: true,
            break_depth: base_depth,
            continue_depth: base_depth,
            start: None,
            breaks: Vec::new(),
            continues: Vec::new(),
        });
        let saved_scopes = self.scopes.clone();
        self.compile_stmts(body)?;
        self.scopes = saved_scopes;
        let continue_target = self.chunk.code.len() as u32;
        self.emit_scope_close()?;
        if let Some(step) = step {
            self.compile_stmt(&step.stmt)?;
        }
        self.emit(Op::Jump(loop_start));
        let exit_target = self.chunk.code.len() as u32;
        if let Some(pos) = exit_jump {
            self.patch_jump(pos, exit_target);
        }
        let Some(loop_ctx) = self.loops.pop() else {
            return Err(CompileError::new(
                "desequilibrio interno de lacos".to_string(),
            ));
        };
        for pos in &loop_ctx.breaks {
            self.patch_jump(*pos, exit_target);
        }
        for pos in &loop_ctx.continues {
            self.patch_jump(*pos, continue_target);
        }
        self.emit_scope_close()?;
        Ok(())
    }

    fn compile_stmt(&mut self, stmt: &Stmt) -> Result<(), CompileError> {
        match stmt {
            Stmt::Echo { expr } => {
                self.compile_expr(expr)?;
                self.emit(Op::Print);
            }
            Stmt::Let { name, ann, init } => {
                match init {
                    Some(expr) => self.compile_expr(expr)?,
                    None => {
                        let idx = self.add_constant(Value::Null)?;
                        self.emit(Op::Constant(idx));
                    }
                }
                if let Some(tag) = ann {
                    self.emit(Op::CheckType(*tag));
                }
                let name_idx = self.intern_name(name)?;
                self.emit(Op::DeclareName(name_idx));
            }
            Stmt::Const { name, ann, init } => {
                self.compile_expr(init)?;
                if let Some(tag) = ann {
                    self.emit(Op::CheckType(*tag));
                }
                let name_idx = self.intern_name(name)?;
                self.emit(Op::DeclareName(name_idx));
                self.declare_const(name);
            }
            Stmt::Static { name, init } => {
                if !self.in_function {
                    return Err(CompileError::new(
                        "`static` fora de funcao (estado por definicao)".to_string(),
                    ));
                }
                self.compile_expr(init)?;
                let name_idx = self.intern_name(name)?;
                self.emit(Op::DeclareStatic(name_idx));
            }
            Stmt::Destructure {
                target,
                init,
                is_const,
            } => {
                // RHS uma vez em temporario oculto; cada alvo = `tmp[i|k]`
                // declarado (erros de indice/chave/duplicata herdados).
                self.compile_expr(init)?;
                let tmp = self.hidden_name("destructure");
                let tmp_idx = self.intern_name(&tmp)?;
                self.emit(Op::DeclareName(tmp_idx));
                match target {
                    DestructureTarget::Positional(names) => {
                        for (i, name) in names.iter().enumerate() {
                            let name_idx = self.intern_name(name)?;
                            self.emit(Op::LoadName(tmp_idx));
                            let key_idx = self.add_constant(Value::Int(i as i64))?;
                            self.emit(Op::Constant(key_idx));
                            self.emit(Op::Index);
                            self.emit(Op::DeclareName(name_idx));
                            if *is_const {
                                self.declare_const(name);
                            }
                        }
                    }
                    DestructureTarget::ByKey(pairs) => {
                        for (key, name) in pairs {
                            let name_idx = self.intern_name(name)?;
                            self.emit(Op::LoadName(tmp_idx));
                            let key_idx = self.add_constant(Value::Str(key.clone()))?;
                            self.emit(Op::Constant(key_idx));
                            self.emit(Op::Index);
                            self.emit(Op::DeclareName(name_idx));
                            if *is_const {
                                self.declare_const(name);
                            }
                        }
                    }
                }
            }
            Stmt::Break => self.compile_break(1)?,
            Stmt::BreakN(n) => self.compile_break(*n)?,
            Stmt::Continue => self.compile_continue(1)?,
            Stmt::ContinueN(n) => self.compile_continue(*n)?,
            Stmt::Assign { target, expr } => match target {
                AssignTarget::Var(name) => {
                    if self.is_const(name) {
                        return Err(CompileError::new(format!("atribuicao a constante: {name}")));
                    }
                    self.compile_expr(expr)?;
                    let name_idx = self.intern_name(name)?;
                    self.emit(Op::StoreName(name_idx));
                }
                AssignTarget::Index { name, index } => {
                    if self.is_const(name) {
                        return Err(CompileError::new(format!("atribuicao a constante: {name}")));
                    }
                    // Ordem na pilha: [indice, valor]; StoreIndex desempilha
                    // valor e depois indice.
                    self.compile_expr(index)?;
                    self.compile_expr(expr)?;
                    let name_idx = self.intern_name(name)?;
                    self.emit(Op::StoreIndex(name_idx));
                }
            },
            Stmt::If {
                cond,
                then_branch,
                else_branch,
            } => self.compile_if(cond, then_branch, else_branch)?,
            Stmt::While { cond, body } => {
                let loop_start = self.chunk.code.len() as u32;
                self.compile_expr(cond)?;
                let exit_jump = self.emit(Op::JumpIfFalse(u32::MAX));
                // Profundidade ANTES de abrir o escopo do corpo: `break` e
                // `continue` fecham tambem o escopo do corpo, sem vazar
                // `let`s internos para fora do laco.
                let base_depth = self.scopes.len();
                self.emit_scope_open();
                self.loops.push(LoopCtx {
                    is_loop: true,
                    break_depth: base_depth,
                    continue_depth: base_depth,
                    start: Some(loop_start),
                    breaks: Vec::new(),
                    continues: Vec::new(),
                });
                // `break`/`continue` no corpo fecham escopos no rastreamento
                // estatico; restaura-o para o caminho normal (corpo sem
                // desvio), que fecha o escopo do corpo logo abaixo. So o
                // rastreamento e restaurado — pools e saltos sao mantidos.
                let saved_scopes = self.scopes.clone();
                self.compile_stmts(body)?;
                self.scopes = saved_scopes;
                self.emit_scope_close()?;
                self.emit(Op::Jump(loop_start));
                let exit_target = self.chunk.code.len() as u32;
                self.patch_jump(exit_jump, exit_target);
                let Some(loop_ctx) = self.loops.pop() else {
                    return Err(CompileError::new(
                        "desequilibrio interno de lacos".to_string(),
                    ));
                };
                for pos in &loop_ctx.breaks {
                    self.patch_jump(*pos, exit_target);
                }
                for pos in &loop_ctx.continues {
                    self.patch_jump(*pos, loop_start);
                }
            }
            Stmt::For { var, iter, body } => {
                self.compile_for(var, iter, body)?;
            }
            Stmt::ForC {
                init,
                cond,
                step,
                body,
            } => {
                self.compile_for_c(init, cond, step, body)?;
            }
            Stmt::Switch {
                discriminant,
                cases,
            } => {
                self.compile_switch(discriminant, cases)?;
            }
            Stmt::FnDef {
                name,
                params,
                variadic,
                return_ann,
                body,
                is_async,
                generic_params,
            } => {
                // Bounds: cada um deve nomear classe ou interface conhecida
                // (definida antes); senao erro claro ja na definicao.
                // Armazena qualificados (namespace da definicao).
                let mut stored: Vec<GenericParam> = Vec::with_capacity(generic_params.len());
                for gp in generic_params.iter() {
                    let bound = match gp.bound.as_ref() {
                        Some(b) => {
                            let qb = self.qualify(b);
                            self.check_bound_known(&qb, name)?;
                            Some(qb)
                        }
                        None => None,
                    };
                    stored.push(GenericParam {
                        name: gp.name.clone(),
                        bound,
                    });
                }
                // Registra generics da funcao de topo (para checar o uso).
                if !self.in_function {
                    self.functions.insert(self.qualify(name), stored);
                }
                let qname = self.qualify(name);
                let own_types: Vec<String> =
                    generic_params.iter().map(|g| g.name.clone()).collect();
                let func_idx = self.compile_func_body(
                    &qname,
                    params,
                    variadic.as_deref(),
                    *return_ann,
                    body,
                    *is_async,
                    &own_types,
                )?;
                self.intern_name(&qname)?;
                self.emit(Op::DefineFunc(func_idx));
            }
            Stmt::Return { expr } => {
                if !self.in_function {
                    return Err(CompileError::new("`return` fora de funcao"));
                }
                // O operando e avaliado ANTES de cruzar os `try`s: o erro da
                // avaliacao e "erro em B" (LANGUAGE_SPEC) e deve alcancar o
                // handler ativo. `TryEnd`/`finally` rodam depois, com o valor
                // ja empilhado (copias de `finally` sao neutras na pilha).
                match expr {
                    Some(expr) => self.compile_expr(expr)?,
                    None => {
                        let idx = self.add_constant(Value::Null)?;
                        self.emit(Op::Constant(idx));
                    }
                }
                // `return` cruza todos os `try`s do frame (handler + `finally`).
                self.emit_abrupt_exits(None)?;
                self.emit(Op::Return);
            }
            Stmt::Yield { expr } => {
                if !self.in_function {
                    return Err(CompileError::new("`yield` fora de funcao"));
                }
                // Mesma ordem do `return`: operando antes de suspender.
                // Diferente do `return`, `yield` NAO cruza `try`s (VM-002):
                // o frame suspende com handlers/escopos intactos e o resume
                // continua dentro do `try`; o `finally` roda no termino
                // normal ou no `return`/`throw`, nao a cada `yield`.
                match expr {
                    Some(expr) => self.compile_expr(expr)?,
                    None => {
                        let idx = self.add_constant(Value::Null)?;
                        self.emit(Op::Constant(idx));
                    }
                }
                self.emit(Op::Yield);
            }
            Stmt::Throw { expr } => {
                self.compile_expr(expr)?;
                self.emit(Op::Throw);
            }
            Stmt::Try {
                body,
                catches,
                finally_body,
            } => self.compile_try_multi(body, catches, finally_body)?,
            Stmt::ClassDef {
                name,
                parent,
                interfaces,
                uses,
                generic_params,
                methods,
            } => self.compile_class(name, parent, interfaces, uses, generic_params, methods)?,
            Stmt::InterfaceDef {
                name,
                generic_params,
                methods,
            } => self.compile_interface(name, generic_params, methods)?,
            Stmt::TraitDef { name, methods } => self.compile_trait(name, methods)?,
            Stmt::Namespace { name } => {
                // `namespace A\B;` define o prefixo das definicoes seguintes.
                // Multiplos `namespace` (ex.: modulos ligados) = ultimo vence.
                self.current_namespace = Some(name.clone());
            }
            Stmt::ImportQualified { .. } => {
                return Err(CompileError::new(
                    "`import A\\B` sem resolucao: use o loader (`rhp run`/`rhp check`)".to_string(),
                ));
            }
            Stmt::ImportNative { name } => {
                let name_idx = self.intern_name(name)?;
                self.emit(Op::LoadNative(name_idx));
            }
            Stmt::Block(stmts) => {
                self.emit_scope_open();
                self.compile_stmts(stmts)?;
                if !diverges(stmts) {
                    self.emit_scope_close()?;
                }
            }
            Stmt::Import(_) => {
                // Resolucao de modulos e papel do carregador (`rhp run`);
                // a compilacao direta ignora (programa ja deve vir resolvido).
            }
            Stmt::ImportNames { .. } => {
                // Como `Import`: so o linker resolve (programa direto com
                // `import { } from` e erro claro, nao silencioso).
                return Err(CompileError::new(
                    "`import { } from` sem resolucao: use o loader (`rhp run`/`rhp check`)"
                        .to_string(),
                ));
            }
            Stmt::Export { .. } => {
                // Marcador consumido pelo linker; fora dele e erro.
                return Err(CompileError::new(
                    "`export` fora de modulo: use o loader (`rhp run`/`rhp check`)".to_string(),
                ));
            }
            Stmt::ReExport { .. } => {
                // Como `Export`: so o loader resolve.
                return Err(CompileError::new(
                    "`export from` fora de modulo: use o loader (`rhp run`/`rhp check`)"
                        .to_string(),
                ));
            }
            Stmt::ExprStmt { expr } => {
                self.compile_expr(expr)?;
                self.emit(Op::Pop);
            }
        }
        Ok(())
    }

    fn compile_expr(&mut self, expr: &Expr) -> Result<(), CompileError> {
        match expr {
            Expr::StringLiteral(s) => {
                let idx = self.add_constant(Value::Str(s.clone()))?;
                self.emit(Op::Constant(idx));
            }
            Expr::Int(v) => {
                let idx = self.add_constant(Value::Int(*v))?;
                self.emit(Op::Constant(idx));
            }
            Expr::Float(v) => {
                let idx = self.add_constant(Value::Float(*v))?;
                self.emit(Op::Constant(idx));
            }
            Expr::Bool(b) => {
                let idx = self.add_constant(Value::Bool(*b))?;
                self.emit(Op::Constant(idx));
            }
            Expr::Null => {
                let idx = self.add_constant(Value::Null)?;
                self.emit(Op::Constant(idx));
            }
            Expr::Var(name) => {
                let rname = self.resolve_name(name);
                let name_idx = self.intern_name(&rname)?;
                self.emit(Op::LoadName(name_idx));
            }
            Expr::Array(items) => {
                for item in items {
                    self.compile_expr(item)?;
                }
                if items.len() > u32::MAX as usize {
                    return Err(CompileError::new("array literal grande demais"));
                }
                self.emit(Op::BuildArray(items.len() as u32));
            }
            Expr::Map(entries) => {
                for (key, value) in entries {
                    let key_idx = self.add_constant(Value::Str(key.clone()))?;
                    self.emit(Op::Constant(key_idx));
                    self.compile_expr(value)?;
                }
                if entries.len() > u32::MAX as usize {
                    return Err(CompileError::new("mapa literal grande demais"));
                }
                self.emit(Op::BuildMap(entries.len() as u32));
            }
            Expr::Index { base, index } => {
                self.compile_expr(base)?;
                self.compile_expr(index)?;
                self.emit(Op::Index);
            }
            Expr::Member { obj, field } => {
                // `obj.campo` = `obj["campo"]`; em instancia resolve campo
                // ou metodo vinculado (RFC 0017).
                self.compile_expr(obj)?;
                let field_idx = self.add_constant(Value::Str(field.clone()))?;
                self.emit(Op::Constant(field_idx));
                self.emit(Op::Index);
            }
            Expr::New(data) => {
                // Checagem de generics (item bounds; apagada no runtime).
                // So classes literais `new Nome<...>` sao checadas; outras
                // formas de `class` caem no erro de runtime vigente.
                if let Expr::Var(name) = data.class.as_ref() {
                    let rname = self.resolve_name(name);
                    if let Some(info) = self.classes.get(&rname).cloned() {
                        self.check_type_args(&rname, &info.generics, &data.type_args, "classe")?;
                    }
                }
                self.compile_new(&data.class, &data.args)?;
            }
            Expr::Unary { op, expr } => {
                self.compile_expr(expr)?;
                match op {
                    UnaryOp::Neg => self.emit(Op::Neg),
                    UnaryOp::Not => self.emit(Op::Not),
                    UnaryOp::BitNot => self.emit(Op::BitNot),
                };
            }
            Expr::Binary {
                op: BinOp::NullCoalesce,
                left,
                right,
            } => {
                // Avalia `left` uma unica vez; se nao-nulo, usa o valor.
                self.compile_expr(left)?;
                self.emit(Op::Dup);
                let end = self.emit(Op::JumpIfNotNull(u32::MAX));
                self.emit(Op::Pop);
                self.compile_expr(right)?;
                let end_target = self.chunk.code.len() as u32;
                self.patch_jump(end, end_target);
            }
            Expr::Binary {
                op: BinOp::LogicalAnd,
                left,
                right,
            } => {
                // Curto-circuito: `false && _` nao avalia a direita.
                // Ambos os lados sao estritamente `Bool`: o esquerdo via
                // `JumpIfFalse`, o direito via `AssertBool` (E3003).
                self.compile_expr(left)?;
                let false_jump = self.emit(Op::JumpIfFalse(u32::MAX));
                self.compile_expr(right)?;
                self.emit(Op::AssertBool);
                let end_jump = self.emit(Op::Jump(u32::MAX));
                let false_target = self.chunk.code.len() as u32;
                self.patch_jump(false_jump, false_target);
                let idx = self.add_constant(Value::Bool(false))?;
                self.emit(Op::Constant(idx));
                let end_target = self.chunk.code.len() as u32;
                self.patch_jump(end_jump, end_target);
            }
            Expr::Binary {
                op: BinOp::LogicalOr,
                left,
                right,
            } => {
                // Curto-circuito: `true || _` nao avalia a direita.
                // Ambos os lados sao estritamente `Bool`: o esquerdo via
                // `JumpIfTrue`, o direito via `AssertBool` (E3003).
                self.compile_expr(left)?;
                let true_jump = self.emit(Op::JumpIfTrue(u32::MAX));
                self.compile_expr(right)?;
                self.emit(Op::AssertBool);
                let end_jump = self.emit(Op::Jump(u32::MAX));
                let true_target = self.chunk.code.len() as u32;
                self.patch_jump(true_jump, true_target);
                let idx = self.add_constant(Value::Bool(true))?;
                self.emit(Op::Constant(idx));
                let end_target = self.chunk.code.len() as u32;
                self.patch_jump(end_jump, end_target);
            }
            Expr::Ternary {
                cond,
                then_branch,
                else_branch,
            } => {
                self.compile_expr(cond)?;
                let else_jump = self.emit(Op::JumpIfFalse(u32::MAX));
                self.compile_expr(then_branch)?;
                let end_jump = self.emit(Op::Jump(u32::MAX));
                let else_target = self.chunk.code.len() as u32;
                self.patch_jump(else_jump, else_target);
                self.compile_expr(else_branch)?;
                let end_target = self.chunk.code.len() as u32;
                self.patch_jump(end_jump, end_target);
            }
            Expr::Binary { op, left, right } => {
                self.compile_expr(left)?;
                self.compile_expr(right)?;
                match op {
                    BinOp::Add => self.emit(Op::Add),
                    BinOp::Sub => self.emit(Op::Sub),
                    BinOp::Mul => self.emit(Op::Mul),
                    BinOp::Div => self.emit(Op::Div),
                    BinOp::Mod => self.emit(Op::Mod),
                    BinOp::Eq => self.emit(Op::Eq),
                    BinOp::Ne => self.emit(Op::Ne),
                    BinOp::Lt => self.emit(Op::Lt),
                    BinOp::Le => self.emit(Op::Le),
                    BinOp::Gt => self.emit(Op::Gt),
                    BinOp::Ge => self.emit(Op::Ge),
                    BinOp::BitAnd => self.emit(Op::BitAnd),
                    BinOp::BitOr => self.emit(Op::BitOr),
                    BinOp::BitXor => self.emit(Op::BitXor),
                    BinOp::Shl => self.emit(Op::Shl),
                    BinOp::Shr => self.emit(Op::Shr),
                    BinOp::NullCoalesce => {
                        // Inalcancavel: tratado no braco dedicado acima.
                        return Err(CompileError::new(
                            "erro interno: NullCoalesce fora do caminho dedicado".to_string(),
                        ));
                    }
                    BinOp::LogicalAnd | BinOp::LogicalOr => {
                        // Inalcancaveis: tratados nos bracos dedicados acima
                        // (curto-circuito via jumps).
                        return Err(CompileError::new(
                            "erro interno: operador logico fora do caminho dedicado".to_string(),
                        ));
                    }
                };
            }
            Expr::Call(data) => {
                let name = data.name.clone();
                let type_args = data.type_args.clone();
                let args = data.args.clone();
                let rname = self.resolve_name(&name);
                if let Some(builtin) = builtin_by_name(&rname) {
                    if !type_args.is_empty() {
                        return Err(CompileError::new(format!(
                            "builtin `{rname}` nao aceita argumentos de tipo"
                        )));
                    }
                    if args.iter().any(|a| !matches!(a, CallArg::Pos(_))) {
                        return Err(CompileError::new(format!(
                            "builtin `{rname}` nao aceita argumentos nomeados nem spread"
                        )));
                    }
                    if args.len() != builtin_arity(builtin) {
                        return Err(CompileError::new(format!(
                            "`{rname}` espera {} argumentos, recebeu {}",
                            builtin_arity(builtin),
                            args.len()
                        )));
                    }
                    for arg in &args {
                        self.compile_expr(arg.value())?;
                    }
                    self.emit(Op::CallBuiltin(BuiltinId::from(builtin)));
                } else {
                    // Checagem de generics (item bounds; apagada no runtime).
                    if let Some(declared) = self.functions.get(&rname).cloned() {
                        self.check_type_args(&rname, &declared, &type_args, "funcao")?;
                    }
                    if args.iter().all(|a| matches!(a, CallArg::Pos(_))) {
                        let name_idx = self.intern_name(&rname)?;
                        self.emit(Op::LoadName(name_idx));
                        for arg in &args {
                            self.compile_expr(arg.value())?;
                        }
                        if args.len() > u32::MAX as usize {
                            return Err(CompileError::new("numero de argumentos excedido"));
                        }
                        self.emit(Op::Call(args.len() as u32));
                    } else {
                        let rv = Expr::Var(rname);
                        self.compile_call_mixed(&rv, &args)?;
                    }
                }
            }
            Expr::CallValue { callee, args } => {
                // `f(x)` direto preserva o caminho classico (precedencia
                // de builtins e erro de aridade em compilacao).
                if let Expr::Var(name) = callee.as_ref() {
                    let rname = self.resolve_name(name);
                    self.compile_expr(&Expr::Call(Box::new(CallData {
                        name: rname,
                        type_args: Vec::new(),
                        args: args.clone(),
                    })))?;
                } else if let Expr::Member { obj, field } = callee.as_ref() {
                    // `super.metodo(args)`: chamada ao pai com `this` atual.
                    if matches!(obj.as_ref(), Expr::Super) {
                        if !self.in_function {
                            return Err(CompileError::new("`super` fora de metodo".to_string()));
                        }
                        let field_idx = self.add_constant(Value::Str(field.clone()))?;
                        let desc_idx = self.emit_call_args(args)?;
                        self.emit(Op::CallSuper(field_idx, desc_idx));
                    } else if let Expr::Var(var) = obj.as_ref() {
                        // `var.metodo(args)`: chamada de metodo com
                        // write-back do `this` (RFC 0017; coerente com
                        // `a[i] = v`, que tambem exige variavel nomeada).
                        let rvar = self.resolve_name(var);
                        let var_idx = self.intern_name(&rvar)?;
                        let field_idx = self.add_constant(Value::Str(field.clone()))?;
                        let desc_idx = self.emit_call_args(args)?;
                        self.emit(Op::CallMethod(var_idx, field_idx, desc_idx));
                    } else {
                        // `expr.metodo(args)`: via `Index`, sem write-back.
                        self.compile_call_mixed(callee, args)?;
                    }
                } else if matches!(callee.as_ref(), Expr::Super) {
                    return Err(CompileError::new(
                        "`super` so em chamada de metodo (`super.m(...)`)".to_string(),
                    ));
                } else if args.iter().all(|a| matches!(a, CallArg::Pos(_))) {
                    self.compile_expr(callee)?;
                    for arg in args {
                        self.compile_expr(arg.value())?;
                    }
                    if args.len() > u32::MAX as usize {
                        return Err(CompileError::new("numero de argumentos excedido"));
                    }
                    self.emit(Op::Call(args.len() as u32));
                } else {
                    self.compile_call_mixed(callee, args)?;
                }
            }
            Expr::Closure(data) => {
                let name = self.fresh_closure_name();
                let func_idx = self.compile_func_body(
                    &name,
                    &data.params,
                    data.variadic.as_deref(),
                    data.return_ann,
                    &data.body,
                    data.is_async,
                    &[],
                )?;
                self.emit(Op::MakeFunc(func_idx));
            }
            Expr::Super => {
                return Err(CompileError::new(
                    "`super` so em chamada de metodo (`super.m(...)`)".to_string(),
                ));
            }
            Expr::Await { expr } => {
                // `await` vale em qualquer lugar (topo ou funcao): bloqueia
                // ate a tarefa concluir; em nao-tarefa e identidade.
                self.compile_expr(expr)?;
                self.emit(Op::Await);
            }
            Expr::Match { subject, arms } => {
                self.compile_match(subject, arms)?;
            }
        }
        Ok(())
    }
}

/// Indica se a execucao nunca passa do fim desta lista de instrucoes
/// (termina em `break`, `continue` ou `return`, considerando `if`/`{}`).
/// Usado para nao emitir fechamento de escopo em codigo inalancavel.
fn diverges(nodes: &[StmtNode]) -> bool {
    match nodes.last().map(|node| &node.stmt) {
        Some(Stmt::Break)
        | Some(Stmt::BreakN(_))
        | Some(Stmt::Continue)
        | Some(Stmt::ContinueN(_))
        | Some(Stmt::Return { .. }) => true,
        Some(Stmt::If {
            then_branch,
            else_branch: Some(other),
            ..
        }) => diverges(then_branch) && diverges(other),
        Some(Stmt::If { .. }) => false,
        Some(Stmt::Block(inner)) => diverges(inner),
        _ => false,
    }
}

/// Compila a AST em bytecode.
///
/// # Erros
///
/// Retorna [`CompileError`] em `return` fora de funcao, aridade de builtin,
/// estouro dos pools ou chunk invalido.
pub fn compile(program: &Program) -> Result<Chunk, CompileError> {
    let mut compiler = Compiler::new(false);
    compiler.compile_stmts(&program.statements)?;
    compiler.emit(Op::Halt);
    compiler.chunk.validate()?;
    Ok(compiler.chunk)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rhp_parser::parse_source;

    fn compile_src(src: &str) -> Chunk {
        let program = parse_source(src).expect("parse deve passar");
        compile(&program).expect("compile deve passar")
    }

    #[test]
    fn compiles_echo_hello() {
        let program = Program {
            statements: vec![StmtNode::from(Stmt::Echo {
                expr: Expr::StringLiteral("Hello from RHP!".to_string()),
            })],
            trailing_comments: vec![],
        };
        let chunk = compile(&program).expect("compile deve passar");
        assert_eq!(
            chunk.constants,
            vec![Value::Str("Hello from RHP!".to_string())]
        );
        assert_eq!(chunk.code, vec![Op::Constant(0), Op::Print, Op::Halt]);
    }

    #[test]
    fn compiles_empty_program_to_halt_only() {
        let chunk = compile(&Program {
            statements: vec![],
            trailing_comments: vec![],
        })
        .expect("deve passar");
        assert!(chunk.constants.is_empty());
        assert_eq!(chunk.code, vec![Op::Halt]);
    }

    #[test]
    fn compiles_arithmetic_expression() {
        // `echo 1 + 2 * 3;` => 1 2 3 Mul Add Print Halt
        let chunk = compile_src("echo 1 + 2 * 3;");
        assert_eq!(
            chunk.code,
            vec![
                Op::Constant(0),
                Op::Constant(1),
                Op::Constant(2),
                Op::Mul,
                Op::Add,
                Op::Print,
                Op::Halt,
            ]
        );
    }

    #[test]
    fn compiles_let_and_assign() {
        let chunk = compile_src("let x = 1; x = 2;");
        assert!(chunk.code.contains(&Op::DeclareName(0)));
        assert!(chunk.code.contains(&Op::StoreName(0)));
        assert_eq!(chunk.names, vec!["x".to_string()]);
    }

    #[test]
    fn compiles_index_assign() {
        let chunk = compile_src("let a = [1]; a[0] = 2;");
        assert!(chunk.code.contains(&Op::BuildArray(1)));
        assert!(chunk.code.contains(&Op::StoreIndex(0)));
    }

    #[test]
    fn compiles_array_map_index() {
        let chunk = compile_src("echo [1, 2][0]; echo {a: 1}[\"a\"];");
        assert!(chunk.code.contains(&Op::BuildArray(2)));
        assert!(chunk.code.contains(&Op::BuildMap(1)));
        assert!(chunk.code.contains(&Op::Index));
    }

    #[test]
    fn compiles_builtin_call() {
        let chunk = compile_src("echo len([1, 2]);");
        assert!(chunk.code.contains(&Op::CallBuiltin(BuiltinId(0))));
    }

    #[test]
    fn compiles_hofs_with_callback() {
        // `map`/`filter`/`usort` viram `CallBuiltin` com ids estaveis; aridade 2.
        let chunk = compile_src("echo map([1], fn (x) { return x; });");
        assert!(chunk.code.contains(&Op::CallBuiltin(BuiltinId(50))));
        let chunk = compile_src("echo filter([1], fn (x) { return true; });");
        assert!(chunk.code.contains(&Op::CallBuiltin(BuiltinId(51))));
        let chunk = compile_src("echo usort([2, 1], fn (a, b) { return a - b; });");
        assert!(chunk.code.contains(&Op::CallBuiltin(BuiltinId(52))));
        // Aridade checada em compilacao (regra dos builtins).
        let program = parse_source("echo map([1]);").expect("parse deve passar");
        let err = compile(&program).expect_err("aridade deve falhar");
        assert!(err.message.contains("espera 2"));
    }

    #[test]
    fn compiles_defaults_and_variadic() {
        // Defaults dobrados para `Value`; `...rest` registrado.
        let chunk = compile_src("fn f(a, b = 2, c = 1 + 2, ...rest) { return a; }");
        assert_eq!(chunk.funcs.len(), 1);
        let def = &chunk.funcs[0];
        assert_eq!(
            def.params,
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
        assert_eq!(
            def.defaults,
            vec![None, Some(Value::Int(2)), Some(Value::Int(3))]
        );
        assert_eq!(def.variadic, Some("rest".to_string()));
        // Nao-constante rejeitada; divisao por zero em constante tambem.
        let program = parse_source("fn f(a = b) { return a; }").expect("parse deve passar");
        let err = compile(&program).expect_err("default nao-const deve falhar");
        assert!(err.message.contains("constante"));
        let program = parse_source("fn f(a = 1 / 0) { return a; }").expect("parse deve passar");
        let err = compile(&program).expect_err("divisao por zero deve falhar");
        assert!(err.message.contains("divisao por zero"));
        // Duplicata (incluindo `...rest` repetindo param) rejeitada.
        let program = parse_source("fn f(a, a) { return a; }").expect("parse deve passar");
        let err = compile(&program).expect_err("duplicata deve falhar");
        assert!(err.message.contains("duplicado"));
        let program = parse_source("fn f(a, ...a) { return a; }").expect("parse deve passar");
        let err = compile(&program).expect_err("rest duplicado deve falhar");
        assert!(err.message.contains("duplicado"));
    }

    #[test]
    fn compiles_destructuring() {
        // Temporario oculto + `Index` + `DeclareName` por alvo; valido.
        let chunk = compile_src("let [a, b] = [1, 2];");
        chunk.validate().expect("destructure deve validar");
        let declares = chunk
            .code
            .iter()
            .filter(|op| matches!(op, Op::DeclareName(_)))
            .count();
        assert_eq!(declares, 3, "temporario + 2 nomes");
        assert!(chunk.code.contains(&Op::Index));
        // `const` marca todos como imutaveis (atribuicao posterior falha).
        let program = parse_source("const {x} = {x: 1}; x = 2;").expect("parse deve passar");
        let err = compile(&program).expect_err("atribuicao a const deve falhar");
        assert!(err.message.contains("constante"));
    }

    #[test]
    fn compiles_mixed_calls() {
        // Nomeado/spread viram `CallMixed` com descritor (`[0, "x", 1]`).
        let chunk = compile_src("echo f(1, x: 2, ...r);");
        assert!(chunk.code.iter().any(|op| matches!(op, Op::CallMixed(_))));
        chunk.validate().expect("CallMixed deve validar");
        let desc_idx = chunk
            .code
            .iter()
            .find_map(|op| match op {
                Op::CallMixed(d) => Some(*d as usize),
                _ => None,
            })
            .expect("tem CallMixed");
        assert_eq!(
            chunk.constants[desc_idx],
            Value::Array(vec![
                Value::Int(0),
                Value::Str("x".to_string()),
                Value::Int(1),
            ])
        );
        // So-posicional preserva o `Call` classico.
        let chunk = compile_src("echo f(1, 2);");
        assert!(chunk.code.contains(&Op::Call(2)));
        assert!(!chunk.code.iter().any(|op| matches!(op, Op::CallMixed(_))));
        // Builtin com nomeado/spread: erro de compilacao.
        let program = parse_source("echo len(x: [1]);").expect("parse deve passar");
        let err = compile(&program).expect_err("builtin com nomeado deve falhar");
        assert!(
            err.message.contains("nao aceita"),
            "obteve: {}",
            err.message
        );
        let program = parse_source("echo len(...[1]);").expect("parse deve passar");
        let err = compile(&program).expect_err("builtin com spread deve falhar");
        assert!(
            err.message.contains("nao aceita"),
            "obteve: {}",
            err.message
        );
    }

    #[test]
    fn compiles_static_statement() {
        // 3 ops novos, validados; `static` fora de funcao = erro.
        let chunk = compile_src("fn f() { static n = 0; echo n; n = 1; }");
        chunk.validate().expect("static deve validar");
        assert!(chunk.funcs[0]
            .body
            .code
            .iter()
            .any(|op| matches!(op, Op::DeclareStatic(_))));
        let program = parse_source("static x = 1;").expect("parse deve passar");
        let err = compile(&program).expect_err("static fora deve falhar");
        assert!(
            err.message.contains("fora de funcao"),
            "obteve: {}",
            err.message
        );
    }

    #[test]
    fn compiles_match_expression() {
        // `StrictEq` por braco; `Throw` no caminho sem-match; valido.
        let chunk = compile_src("echo match x { 1 => \"a\", default => \"b\" };");
        chunk.validate().expect("match deve validar");
        assert!(chunk.code.contains(&Op::StrictEq));
        assert!(chunk.code.contains(&Op::Throw));
        // Sem-match sem `default`: compila (erro e de runtime).
        let chunk = compile_src("echo match x { 1 => \"a\" };");
        chunk.validate().expect("match sem default deve validar");
    }

    #[test]
    fn compiles_for_c_and_break_levels() {
        let chunk = compile_src("for let i = 0; i < 3; i = i + 1 { echo i; }");
        chunk.validate().expect("for-C deve validar");
        let chunk = compile_src("for ;; { break; }");
        chunk.validate().expect("for infinito deve validar");
        // Niveis: `break 2` sai de dois; `continue 2` mira o laco externo.
        let chunk = compile_src("while true { switch 1 { case 1 { break 2; } } }");
        chunk.validate().expect("break 2 deve validar");
        let chunk = compile_src(
            "for let j = 0; j < 2; j = j + 1 { switch j { case 0 { continue 2; } default { echo j; } } }",
        );
        chunk.validate().expect("continue 2 deve validar");
        // Alem do aninhamento: erro.
        let program = parse_source("while true { break 2; }").expect("parse deve passar");
        let err = compile(&program).expect_err("deve falhar");
        assert!(err.message.contains("alem do aninhamento"));
        let program = parse_source("break;").expect("parse deve passar");
        let err = compile(&program).expect_err("deve falhar");
        assert!(err.message.contains("fora de laco"));
    }

    #[test]
    fn compiles_closure_and_general_calls() {
        // Closure vira `MakeFunc` (valor, nome unico `{closure:N}`); `f(1)`
        // direto preserva `Call`.
        let chunk = compile_src("let f = fn (x) { return x; }; echo f(1);");
        assert_eq!(chunk.funcs.len(), 1);
        assert_eq!(chunk.funcs[0].name, "{closure:0}");
        assert!(chunk.code.contains(&Op::MakeFunc(0)));
        assert!(chunk.code.contains(&Op::Call(1)));
        // Chamada de callee arbitrario compila callee + `Call`.
        let chunk = compile_src("echo (fn (x) { return x; })(41);");
        assert!(chunk.code.contains(&Op::MakeFunc(0)));
        assert!(chunk.code.contains(&Op::Call(1)));
    }

    #[test]
    fn rejects_builtin_arity_at_compile_time() {
        let program = parse_source("echo len(1, 2);").expect("parse deve passar");
        let err = compile(&program).expect_err("deve falhar");
        assert!(err.message.contains("espera 1"));
    }

    #[test]
    fn checks_generic_bounds_at_compile_time() {
        // Bound satisfeito (subclasse e classe exata passam).
        compile_src(
            "class E { fn novo() { this[\"i\"] = 0; } } class U extends E { fn novo() { this[\"i\"] = 1; } } class R<T: E> { fn novo() { this[\"x\"] = 0; } } let a = new R<U>(); let b = new R<E>();",
        );
        // Violacao: tipo fora da hierarquia.
        let program = parse_source(
            "class E { fn novo() { this[\"i\"] = 0; } } class O { fn novo() { this[\"x\"] = 0; } } class R<T: E> { fn novo() { this[\"x\"] = 0; } } let a = new R<O>();",
        )
        .expect("parse deve passar");
        let err = compile(&program).expect_err("deve falhar");
        assert!(
            err.message.contains("nao satisfaz"),
            "obteve: {}",
            err.message
        );
        // Bound de interface: implementa passa, nao-implementa falha.
        compile_src(
            "interface S { fn salvar(); } class M implements S { fn novo() { this[\"x\"] = 0; } fn salvar() { } } fn g<T: S>(v) { return v; } echo g<M>(1);",
        );
        let program = parse_source(
            "interface S { fn salvar(); } class N { fn novo() { this[\"x\"] = 0; } } fn g<T: S>(v) { return v; } echo g<N>(1);",
        )
        .expect("parse deve passar");
        let err = compile(&program).expect_err("deve falhar");
        assert!(
            err.message.contains("nao satisfaz"),
            "obteve: {}",
            err.message
        );
        // Bound desconhecido na definicao = erro claro.
        let program =
            parse_source("fn f<T: NaoExiste>(x) { return x; }").expect("parse deve passar");
        let err = compile(&program).expect_err("deve falhar");
        assert!(
            err.message.contains("desconhecido"),
            "obteve: {}",
            err.message
        );
        // Aridade: 2 declarados, 1 usado e vice-versa.
        let program =
            parse_source("class C<T, U> { fn novo() { this[\"x\"] = 0; } } let c = new C<int>();")
                .expect("parse deve passar");
        let err = compile(&program).expect_err("deve falhar");
        assert!(err.message.contains("espera 2"), "obteve: {}", err.message);
        // Nao-generica com argumentos de tipo = erro.
        let program =
            parse_source("class S { fn novo() { this[\"x\"] = 0; } } let c = new S<int>();")
                .expect("parse deve passar");
        let err = compile(&program).expect_err("deve falhar");
        assert!(
            err.message.contains("nao e classe generica"),
            "obteve: {}",
            err.message
        );
        // Builtin com argumentos de tipo = erro.
        let program = parse_source("echo len<int>(\"abc\");").expect("parse deve passar");
        let err = compile(&program).expect_err("deve falhar");
        assert!(
            err.message.contains("nao aceita argumentos de tipo"),
            "obteve: {}",
            err.message
        );
        // Sem argumentos de tipo = apagado (legado, sem checagem).
        compile_src("class C<T> { fn novo(v) { this[\"v\"] = v; } } let c = new C(1); echo 1;");
        // Parametro de tipo visivel adia a checagem para a instanciacao.
        compile_src(
            "class E { fn novo() { this[\"i\"] = 0; } } class C<T: E> { fn novo(v) { this[\"v\"] = v; } } fn f<T>(x) { let c = new C<T>(x); return c; }",
        );
    }

    #[test]
    fn compiles_if_with_jumps_in_range() {
        let chunk = compile_src("if true { echo 1; } else { echo 2; }");
        chunk.validate().expect("saltos devem ser validos");
        assert!(chunk.code.contains(&Op::EnterScope));
        assert!(chunk.code.contains(&Op::ExitScope));
    }

    #[test]
    fn compiles_while_loop() {
        let chunk = compile_src("while false { echo 1; }");
        chunk.validate().expect("laco deve ser valido");
    }

    #[test]
    fn compiles_switch_with_break_and_fallthrough() {
        // Discriminante + Dup/Eq/JumpIfFalse por case com teste; `break`
        // com `Pop` proprio; `Pop` final; chunk valido.
        let chunk = compile_src(
            "switch x { case 1 { echo \"a\"; break; } default { echo \"d\"; } case 2 { echo \"b\"; } }",
        );
        chunk.validate().expect("switch deve ser valido");
        assert!(chunk.code.contains(&Op::Dup));
        assert!(chunk.code.contains(&Op::Eq));
        assert!(chunk.code.contains(&Op::Pop));
        assert!(chunk.code.iter().any(|op| matches!(op, Op::JumpIfFalse(_))));
        // `break` fora de laco/`switch` continua erro.
        let program = parse_source("break;").expect("parse deve passar");
        let err = compile(&program).expect_err("break solto deve falhar");
        assert!(err.message.contains("fora de laco"));
        let program = parse_source("continue;").expect("parse deve passar");
        let err = compile(&program).expect_err("continue solto deve falhar");
        assert!(err.message.contains("fora de laco"));
        // `continue` em `switch` sem laco age como `break` (PHP): valido.
        let chunk = compile_src("switch 1 { case 1 { echo 1; continue; } case 2 { echo 2; } }");
        chunk.validate().expect("continue em switch deve validar");
        // `continue 2` com 1 so nivel: erro.
        let program =
            parse_source("switch 1 { case 1 { continue 2; } }").expect("parse deve passar");
        let err = compile(&program).expect_err("continue 2 sem niveis deve falhar");
        assert!(err.message.contains("alem do aninhamento"));
    }

    #[test]
    fn compiles_fn_def_and_call() {
        let chunk = compile_src("fn soma(a, b) { return a + b; } echo soma(1, 2);");
        assert_eq!(chunk.funcs.len(), 1);
        assert_eq!(chunk.funcs[0].name, "soma");
        assert_eq!(
            chunk.funcs[0].params,
            vec!["a".to_string(), "b".to_string()]
        );
        assert!(chunk.code.contains(&Op::DefineFunc(0)));
        assert!(chunk.code.contains(&Op::Call(2)));
    }

    #[test]
    fn compiles_ternary_and_null_coalescing() {
        let chunk = compile_src("echo true ? 1 : 2;");
        assert!(chunk.code.iter().any(|op| matches!(op, Op::JumpIfFalse(_))));
        let chunk = compile_src("echo a ?? b;");
        assert!(chunk.code.contains(&Op::Dup));
        assert!(chunk
            .code
            .iter()
            .any(|op| matches!(op, Op::JumpIfNotNull(_))));
    }

    #[test]
    fn compiles_const_and_rejects_const_assign() {
        let chunk = compile_src("const N = 10; echo N;");
        assert!(chunk.code.contains(&Op::DeclareName(0)));
        let program = parse_source("const N = 1; N = 2;").expect("parse deve passar");
        let err = compile(&program).expect_err("deve falhar");
        assert!(err.message.contains("constante"));
        let program = parse_source("const A = [1]; A[0] = 2;").expect("parse deve passar");
        let err = compile(&program).expect_err("deve falhar");
        assert!(err.message.contains("constante"));
    }

    #[test]
    fn compiles_break_continue_and_rejects_outside_loop() {
        let chunk = compile_src("while true { break; } while true { continue; }");
        chunk
            .validate()
            .expect("laco com break/continue deve validar");
        let program = parse_source("break;").expect("parse deve passar");
        let err = compile(&program).expect_err("deve falhar");
        assert!(err.message.contains("fora de laco"));
        let program = parse_source("continue;").expect("parse deve passar");
        let err = compile(&program).expect_err("deve falhar");
        assert!(err.message.contains("fora de laco"));
    }

    #[test]
    fn compiles_for_loop_with_break_continue() {
        let chunk = compile_src("for x in [1, 2] { if x == 1 { continue; } break; }");
        chunk.validate().expect("for deve validar");
        assert!(chunk.code.contains(&Op::IterList));
        // `for` aninhado usa nomes ocultos distintos (sem colisao).
        let nested = compile_src("for a in [1] { for b in [2] { echo b; } }");
        nested.validate().expect("for aninhado deve validar");
        // `for` dentro de funcao compila.
        let in_fn = compile_src("fn f() { for x in [1] { echo x; } }");
        in_fn.validate().expect("for em funcao deve validar");
        // Aridade dos novos builtins.
        let program = parse_source("echo sort([2, 1]); exit(0);").expect("parse deve passar");
        compile(&program).expect("sort/exit com aridade certa deve passar");
        let wrong = parse_source("echo sort([1], [2]);").expect("parse deve passar");
        assert!(compile(&wrong)
            .expect_err("deve falhar")
            .message
            .contains("espera 1"));
    }

    #[test]
    fn compiles_nested_for_linear_not_exponential() {
        // COMP-001: o corpo do `for x in` era compilado duas vezes (lazy +
        // eager), custo 2^N em l niveis (20 niveis travavam o CLI). O corpo
        // agora e compilado uma vez: crescimento linear com a profundidade.
        fn nested(depth: usize) -> String {
            let mut src = String::from("echo 1;");
            for _ in 0..depth {
                src = format!("for x in [1] {{ {src} }}");
            }
            src
        }
        let small = compile_src(&nested(4));
        small.validate().expect("for aninhado deve validar");
        let big = compile_src(&nested(8));
        big.validate().expect("for aninhado deve validar");
        assert!(
            big.code.len() < small.code.len() * 3,
            "crescimento super-linear: 4 niveis = {} ops, 8 niveis = {} ops",
            small.code.len(),
            big.code.len()
        );
    }

    #[test]
    fn compiles_try_catch_finally_variants() {
        for src in [
            "try { echo 1; } catch e { echo e; }",
            "try { echo 1; } catch { echo 2; }",
            "try { echo 1; } finally { echo 2; }",
            "try { echo 1; } catch e { echo e; } finally { echo 2; }",
            "throw \"boom\";",
            "fn f() { try { return 1; } finally { echo 2; } }",
            "while true { try { break; } finally { echo 1; } }",
            "for x in [1] { try { continue; } catch { } }",
            "try { try { throw 1; } catch e { throw e; } } catch e { echo e; }",
        ] {
            let chunk = compile_src(src);
            chunk.validate().expect("try deve validar");
        }
        // `throw` gera `Throw`; `try/finally` sem `catch` gera `Rethrow`;
        // `try/catch` nao gera `Rethrow` (erro consumido).
        let chunk = compile_src("throw 1;");
        assert!(chunk.code.contains(&Op::Throw));
        let chunk = compile_src("try { echo 1; } finally { echo 2; }");
        assert!(chunk.code.contains(&Op::Rethrow));
        let chunk = compile_src("try { echo 1; } catch e { echo e; }");
        assert!(!chunk.code.contains(&Op::Rethrow));
        assert!(chunk.code.iter().any(|op| matches!(op, Op::TryBegin(_))));
    }

    #[test]
    fn compiles_bitwise_ops() {
        let chunk = compile_src("echo 6 & 3 | 1 ^ 2; echo ~5; echo 1 << 4;");
        assert!(chunk.code.contains(&Op::BitAnd));
        assert!(chunk.code.contains(&Op::BitOr));
        assert!(chunk.code.contains(&Op::BitXor));
        assert!(chunk.code.contains(&Op::BitNot));
        assert!(chunk.code.contains(&Op::Shl));
    }

    #[test]
    fn rejects_duplicate_params() {
        let program = parse_source("fn f(a, a) { return a; }").expect("parse deve passar");
        let err = compile(&program).expect_err("deve falhar");
        assert!(err.message.contains("duplicado"));
    }

    #[test]
    fn rejects_return_outside_function() {
        let program = parse_source("return 1;").expect("parse deve passar");
        let err = compile(&program).expect_err("deve falhar");
        assert!(err.message.contains("fora de funcao"));
    }

    #[test]
    fn rejects_chunk_without_halt() {
        let chunk = Chunk {
            constants: vec![],
            names: vec![],
            funcs: vec![],
            code: vec![Op::Print],
        };
        let err = chunk.validate().expect_err("deve falhar");
        assert!(err.message.contains("Halt"));
    }

    #[test]
    fn rejects_constant_out_of_bounds() {
        let chunk = Chunk {
            constants: vec![],
            names: vec![],
            funcs: vec![],
            code: vec![Op::Constant(0), Op::Halt],
        };
        let err = chunk.validate().expect_err("deve falhar");
        assert!(err.message.contains("fora do pool"));
    }

    #[test]
    fn rejects_jump_out_of_range() {
        let chunk = Chunk {
            constants: vec![],
            names: vec![],
            funcs: vec![],
            code: vec![Op::Jump(99), Op::Halt],
        };
        let err = chunk.validate().expect_err("deve falhar");
        assert!(err.message.contains("salto"));
    }

    #[test]
    fn deduplicates_repeated_constants() {
        let program = parse_source(&"echo 1;\n".repeat(2000)).expect("parse deve passar");
        let chunk = compile(&program).expect("compile deve passar");
        assert_eq!(chunk.constants, vec![Value::Int(1)]);
        assert_eq!(chunk.names.len(), 0);
    }

    #[test]
    fn rejects_stack_underflow_statically() {
        // Print sem nada na pilha: antes so falhava em runtime.
        let chunk = Chunk {
            constants: vec![],
            names: vec![],
            funcs: vec![],
            code: vec![Op::Print, Op::Halt],
        };
        let err = chunk.validate().expect_err("deve falhar");
        assert!(err.message.contains("pilha"));
    }

    #[test]
    fn rejects_inconsistent_jump_heights() {
        // pos 2 alcancada com altura 0 (via 1) e altura 1 (via 3).
        let chunk = Chunk {
            constants: vec![Value::Int(0)],
            names: vec![],
            funcs: vec![],
            code: vec![
                Op::Constant(0),
                Op::JumpIfFalse(4),
                Op::Constant(0),
                Op::Jump(2),
                Op::Halt,
            ],
        };
        let err = chunk.validate().expect_err("deve falhar");
        assert!(err.message.contains("altura"));
    }

    #[test]
    fn builtin_ids_roundtrip() {
        let all = [
            Builtin::Len,
            Builtin::Push,
            Builtin::Pop,
            Builtin::Keys,
            Builtin::Str,
            Builtin::Int,
            Builtin::Float,
            Builtin::Type,
            Builtin::Assert,
            Builtin::HttpGet,
            Builtin::Contains,
            Builtin::StartsWith,
            Builtin::EndsWith,
            Builtin::Split,
            Builtin::Join,
            Builtin::Trim,
            Builtin::Upper,
            Builtin::Lower,
            Builtin::Replace,
            Builtin::Abs,
            Builtin::Min,
            Builtin::Max,
            Builtin::Sqrt,
            Builtin::Pow,
            Builtin::Floor,
            Builtin::Ceil,
            Builtin::Values,
            Builtin::Entries,
            Builtin::Merge,
            Builtin::Has,
            Builtin::FsRead,
            Builtin::FsWrite,
            Builtin::FsAppend,
            Builtin::FsExists,
            Builtin::Argv,
            Builtin::Env,
            Builtin::Sort,
            Builtin::Exit,
            Builtin::JsonParse,
            Builtin::JsonStringify,
            Builtin::Now,
            Builtin::Sleep,
            Builtin::DateStr,
            Builtin::HttpFetch,
            Builtin::DbOpen,
            Builtin::DbExec,
            Builtin::DbQuery,
            Builtin::DbClose,
            Builtin::HttpServe,
            Builtin::Template,
            Builtin::Map,
            Builtin::Filter,
            Builtin::Usort,
            Builtin::Rand,
            Builtin::RandRange,
            Builtin::FsList,
            Builtin::HashPassword,
            Builtin::VerifyPassword,
            Builtin::RegexMatch,
            Builtin::RegexReplace,
            Builtin::RegexSplit,
            Builtin::Sha256,
            Builtin::HmacSha256,
            Builtin::HmacVerify,
            Builtin::Next,
            Builtin::Exhausted,
            Builtin::AsyncSpawn,
            Builtin::AsyncJoin,
            Builtin::TcpConnect,
            Builtin::TcpSend,
            Builtin::TcpRecv,
            Builtin::TcpClose,
            Builtin::RedisOpen,
            Builtin::RedisExec,
            Builtin::RedisClose,
            Builtin::AwaitTimeout,
            Builtin::AsyncCancel,
            Builtin::AsyncCancelled,
            Builtin::AwaitAll,
            Builtin::AwaitRace,
        ];
        for (expected_id, builtin) in all.iter().enumerate() {
            let id = BuiltinId::from(*builtin);
            assert_eq!(id.0 as usize, expected_id);
            assert_eq!(id.resolve().expect("id valido"), *builtin);
        }
        assert_eq!(builtin_by_name("len"), Some(Builtin::Len));
        assert_eq!(builtin_by_name("nao_existe"), None);
        assert_eq!(builtin_arity(Builtin::Push), 2);
        assert_eq!(builtin_arity(Builtin::Replace), 3);
        assert_eq!(builtin_arity(Builtin::Len), 1);
    }

    #[test]
    fn rejects_unknown_builtin() {
        let chunk = Chunk {
            constants: vec![],
            names: vec![],
            funcs: vec![],
            code: vec![Op::CallBuiltin(BuiltinId(99)), Op::Halt],
        };
        let err = chunk.validate().expect_err("deve falhar");
        assert!(err.message.contains("builtin"));
    }

    #[test]
    fn rejects_direct_reexport_compile() {
        // Re-export so via loader (como `export`/`import`).
        let program = parse_source("export { a } from \"m.rhp\";").expect("parse deve passar");
        let err = compile(&program).expect_err("deve falhar");
        assert!(
            err.message.contains("fora de modulo"),
            "obteve: {}",
            err.message
        );
    }

    #[test]
    fn rejects_deeply_nested_constant() {
        let mut nested = Value::Int(0);
        for _ in 0..(MAX_VALUE_DEPTH + 10) {
            nested = Value::Array(vec![nested]);
        }
        let chunk = Chunk {
            constants: vec![nested],
            names: vec![],
            funcs: vec![],
            code: vec![Op::Constant(0), Op::Print, Op::Halt],
        };
        let err = chunk.validate().expect_err("deve falhar");
        assert!(err.message.contains("profundidade maxima"));
    }

    #[test]
    fn accepts_constant_within_depth_limit() {
        let mut nested = Value::Int(0);
        for _ in 0..MAX_VALUE_DEPTH {
            nested = Value::Array(vec![nested]);
        }
        let chunk = Chunk {
            constants: vec![nested],
            names: vec![],
            funcs: vec![],
            code: vec![Op::Constant(0), Op::Print, Op::Halt],
        };
        chunk.validate().expect("deve passar");
    }

    #[test]
    fn displays_values() {
        assert_eq!(Value::Null.to_string(), "null");
        assert_eq!(Value::Bool(true).to_string(), "true");
        assert_eq!(Value::Int(-7).to_string(), "-7");
        assert_eq!(
            Value::Str("Hello from RHP!".to_string()).to_string(),
            "Hello from RHP!"
        );
        assert_eq!(
            Value::Array(vec![Value::Int(1), Value::Bool(false)]).to_string(),
            "[1, false]"
        );
        let mut map = HashMap::new();
        map.insert("b".to_string(), Value::Int(2));
        map.insert("a".to_string(), Value::Int(1));
        assert_eq!(Value::Map(map).to_string(), "{a: 1, b: 2}");
        let func = Rc::new(FuncDef {
            name: "f".to_string(),
            params: vec![],
            defaults: vec![],
            variadic: None,
            param_types: vec![],
            return_type: None,
            is_generator: false,
            is_async: false,
            native_id: None,
            body: Rc::new(Chunk::new()),
        });
        assert_eq!(
            Value::Func(FuncValue {
                def: func,
                env: HashMap::new()
            })
            .to_string(),
            "<fn f>"
        );
    }
}
