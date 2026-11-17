//! Bytecode e geracao de codigo do RHP (Fase 3).
//!
//! Modelo: chunk com pool de constantes, pool de nomes, tabela de funcoes e
//! vetor de instrucoes, mais builtins da biblioteca-padrao nucleo. A VM e
//! baseada em pilha e valida o chunk (recursivamente) antes de executar.

use rhp_parser::{
    AssignTarget, BinOp, CallArg, DestructureTarget, Expr, MatchArm, Program, Stmt, StmtNode,
    SwitchCase, UnaryOp,
};
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
}

/// Valor funcional de primeira classe: definicao + ambiente capturado.
#[derive(Debug, Clone, PartialEq)]
pub struct FuncValue {
    pub def: Rc<FuncDef>,
    pub env: HashMap<String, Value>,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => write!(f, "null"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Int(i) => write!(f, "{i}"),
            Self::Float(x) => write!(f, "{x}"),
            Self::Str(s) => write!(f, "{s}"),
            Self::Array(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, "]")
            }
            Self::Map(entries) => {
                let mut keys: Vec<&String> = entries.keys().collect();
                keys.sort();
                write!(f, "{{")?;
                for (i, key) in keys.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    if let Some(value) = entries.get(*key) {
                        write!(f, "{key}: {value}")?;
                    }
                }
                write!(f, "}}")
            }
            Self::Func(func) => write!(f, "<fn {}>", func.def.name),
        }
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
        _ => None,
    }
}

#[must_use]
pub const fn builtin_arity(builtin: Builtin) -> usize {
    match builtin {
        Builtin::Argv | Builtin::Now | Builtin::Rand => 0,
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
        | Builtin::HmacSha256 => 2,
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
                Op::DefineFunc(idx) | Op::MakeFunc(idx) if (idx as usize) >= self.funcs.len() => {
                    return Err(CompileError::new(format!(
                        "chunk invalido em `{context}`: funcao {idx} inexistente"
                    )));
                }
                Op::JumpIfFalse(target)
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
                Op::CallMixed(desc) => {
                    // Descritor existe; o tipo e checado em `call_mixed_slots`.
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
            Op::JumpIfFalse(target) | Op::JumpIfNotNull(target) => {
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
            // Compostos e funcoes: sem dedup (comparacao profunda cara,
            // beneficio atipico).
            Value::Array(_) | Value::Map(_) | Value::Func(_) => None,
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
    fn compile_func_body(
        &mut self,
        name: &str,
        params: &[rhp_parser::Param],
        variadic: Option<&str>,
        body: &[StmtNode],
    ) -> Result<u32, CompileError> {
        let mut seen = std::collections::HashSet::new();
        let mut names = Vec::with_capacity(params.len());
        let mut defaults = Vec::with_capacity(params.len());
        for param in params {
            if !seen.insert(param.name.clone()) {
                return Err(CompileError::new(format!(
                    "parametro duplicado: {}",
                    param.name
                )));
            }
            names.push(param.name.clone());
            defaults.push(param.default.as_ref().map(eval_const_expr).transpose()?);
        }
        if let Some(rest) = variadic {
            if !seen.insert(rest.to_string()) {
                return Err(CompileError::new(format!("parametro duplicado: {rest}")));
            }
        }
        let mut fn_compiler = Compiler::new(true);
        fn_compiler.compile_stmts(body)?;
        let null_idx = fn_compiler.add_constant(Value::Null)?;
        fn_compiler.emit(Op::Constant(null_idx));
        fn_compiler.emit(Op::Return);
        fn_compiler.emit(Op::Halt);
        let body_chunk = fn_compiler.chunk;
        body_chunk.validate()?;
        if self.chunk.funcs.len() >= u32::MAX as usize {
            return Err(CompileError::new("limite de funcoes excedido"));
        }
        self.chunk.funcs.push(Rc::new(FuncDef {
            name: name.to_string(),
            params: names,
            defaults,
            variadic: variadic.map(str::to_string),
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
        let list_name = self.hidden_for_name("lista");
        let idx_name = self.hidden_for_name("indice");
        self.compile_expr(iter)?;
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
        self.emit_scope_open();
        let body_depth = self.scopes.len();
        self.loops.push(LoopCtx {
            is_loop: true,
            break_depth: for_depth,
            continue_depth: body_depth,
            start: None,
            breaks: Vec::new(),
            continues: Vec::new(),
        });
        // VAR = H_list[H_idx].
        let list_elem = self.intern_name(&list_name)?;
        self.emit(Op::LoadName(list_elem));
        let idx_elem = self.intern_name(&idx_name)?;
        self.emit(Op::LoadName(idx_elem));
        self.emit(Op::Index);
        let var_idx = self.intern_name(var)?;
        self.emit(Op::DeclareName(var_idx));
        // Como no `while`: desvios no corpo mexem no rastreamento estatico;
        // restaura-o para o caminho normal (incremento apos o corpo).
        let saved_scopes = self.scopes.clone();
        self.compile_stmts(body)?;
        self.scopes = saved_scopes;
        let continue_target = self.chunk.code.len() as u32;
        self.emit_scope_close()?;
        // H_idx = H_idx + 1.
        let idx_inc = self.intern_name(&idx_name)?;
        self.emit(Op::LoadName(idx_inc));
        let one = self.add_constant(Value::Int(1))?;
        self.emit(Op::Constant(one));
        self.emit(Op::Add);
        let idx_store = self.intern_name(&idx_name)?;
        self.emit(Op::StoreName(idx_store));
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
            self.patch_jump(*pos, continue_target);
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

    /// Compila `try B (catch V? C)? (finally F)?` (RFC 0001).
    ///
    /// Desenho: `TryBegin->alvo` + escopo + corpo; `TryEnd` + `finally`
    /// (caminho normal); `catch`: vinculo do erro + corpo + `finally`;
    /// sem `catch`: `finally` + `Rethrow`. `finally` e duplicado por
    /// caminho (codigo neutro na pilha; validador aprova cada copia).
    /// `break`/`continue`/`return` dentro do corpo fecham handler e rodam
    /// `finally` via `emit_abrupt_exits` (braco respectivo).
    fn compile_try(
        &mut self,
        body: &[StmtNode],
        catch_var: &Option<String>,
        catch_body: &Option<Vec<StmtNode>>,
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
        if let Some(cbody) = catch_body {
            self.emit_scope_open();
            match catch_var {
                Some(name) => {
                    let name_idx = self.intern_name(name)?;
                    self.emit(Op::DeclareName(name_idx));
                }
                None => {
                    self.emit(Op::Pop);
                }
            }
            let saved_catch = self.scopes.clone();
            self.compile_stmts(cbody)?;
            self.scopes = saved_catch;
            if let Some(fin) = finally_body {
                self.emit_finally_copy(fin)?;
            }
            self.emit_scope_close()?;
        } else {
            // Sem `catch`: `finally` roda e o erro segue propagando.
            // A pilha aqui tem exatamente o valor do erro (altura 1).
            if let Some(fin) = finally_body {
                self.emit_finally_copy(fin)?;
            }
            self.emit(Op::Rethrow);
        }
        let end_target = self.chunk.code.len() as u32;
        self.patch_jump(end_jump, end_target);
        self.pending_trys.pop();
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
                let name_idx = self.intern_name(name)?;
                self.emit(Op::LoadName(name_idx));
            }
            other => {
                self.compile_expr(other)?;
            }
        }
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
        let desc_idx = self.add_constant(Value::Array(desc))?;
        self.emit(Op::CallMixed(desc_idx));
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
            Stmt::Let { name, init } => {
                match init {
                    Some(expr) => self.compile_expr(expr)?,
                    None => {
                        let idx = self.add_constant(Value::Null)?;
                        self.emit(Op::Constant(idx));
                    }
                }
                let name_idx = self.intern_name(name)?;
                self.emit(Op::DeclareName(name_idx));
            }
            Stmt::Const { name, init } => {
                self.compile_expr(init)?;
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
                body,
            } => {
                let func_idx = self.compile_func_body(name, params, variadic.as_deref(), body)?;
                self.intern_name(name)?;
                self.emit(Op::DefineFunc(func_idx));
            }
            Stmt::Return { expr } => {
                if !self.in_function {
                    return Err(CompileError::new("`return` fora de funcao"));
                }
                // `return` cruza todos os `try`s do frame (handler + `finally`).
                self.emit_abrupt_exits(None)?;
                match expr {
                    Some(expr) => self.compile_expr(expr)?,
                    None => {
                        let idx = self.add_constant(Value::Null)?;
                        self.emit(Op::Constant(idx));
                    }
                }
                self.emit(Op::Return);
            }
            Stmt::Throw { expr } => {
                self.compile_expr(expr)?;
                self.emit(Op::Throw);
            }
            Stmt::Try {
                body,
                catch_var,
                catch_body,
                finally_body,
            } => self.compile_try(body, catch_var, catch_body, finally_body)?,
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
                let name_idx = self.intern_name(name)?;
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
                };
            }
            Expr::Call { name, args } => {
                if let Some(builtin) = builtin_by_name(name) {
                    if args.iter().any(|a| !matches!(a, CallArg::Pos(_))) {
                        return Err(CompileError::new(format!(
                            "builtin `{name}` nao aceita argumentos nomeados nem spread"
                        )));
                    }
                    if args.len() != builtin_arity(builtin) {
                        return Err(CompileError::new(format!(
                            "`{name}` espera {} argumentos, recebeu {}",
                            builtin_arity(builtin),
                            args.len()
                        )));
                    }
                    for arg in args {
                        self.compile_expr(arg.value())?;
                    }
                    self.emit(Op::CallBuiltin(BuiltinId::from(builtin)));
                } else if args.iter().all(|a| matches!(a, CallArg::Pos(_))) {
                    let name_idx = self.intern_name(name)?;
                    self.emit(Op::LoadName(name_idx));
                    for arg in args {
                        self.compile_expr(arg.value())?;
                    }
                    if args.len() > u32::MAX as usize {
                        return Err(CompileError::new("numero de argumentos excedido"));
                    }
                    self.emit(Op::Call(args.len() as u32));
                } else {
                    self.compile_call_mixed(&Expr::Var(name.clone()), args)?;
                }
            }
            Expr::CallValue { callee, args } => {
                // `f(x)` direto preserva o caminho classico (precedencia
                // de builtins e erro de aridade em compilacao).
                if let Expr::Var(name) = callee.as_ref() {
                    self.compile_expr(&Expr::Call {
                        name: name.clone(),
                        args: args.clone(),
                    })?;
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
                let func_idx = self.compile_func_body(
                    "{closure}",
                    &data.params,
                    data.variadic.as_deref(),
                    &data.body,
                )?;
                self.emit(Op::MakeFunc(func_idx));
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
        // Closure vira `MakeFunc` (valor); `f(1)` direto preserva `Call`.
        let chunk = compile_src("let f = fn (x) { return x; }; echo f(1);");
        assert_eq!(chunk.funcs.len(), 1);
        assert_eq!(chunk.funcs[0].name, "{closure}");
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
