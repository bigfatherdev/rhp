//! Bytecode e geracao de codigo do RHP (Fase 3).
//!
//! Modelo: chunk com pool de constantes, pool de nomes, tabela de funcoes e
//! vetor de instrucoes, mais builtins da biblioteca-padrao nucleo. A VM e
//! baseada em pilha e valida o chunk (recursivamente) antes de executar.

use rhp_parser::{AssignTarget, BinOp, Expr, Program, Stmt, UnaryOp};
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
    /// Funcao definida por `fn`: parametros + corpo compilado.
    Func(Rc<FuncDef>),
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
            Self::Func(func) => write!(f, "<fn {}>", func.name),
        }
    }
}

/// Definicao de funcao compilada.
#[derive(Debug, Clone, PartialEq)]
pub struct FuncDef {
    pub name: String,
    pub params: Vec<String>,
    pub body: Chunk,
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
        _ => None,
    }
}

#[must_use]
pub const fn builtin_arity(builtin: Builtin) -> usize {
    match builtin {
        Builtin::Argv => 0,
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
        | Builtin::JsonStringify => 1,
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
        | Builtin::FsAppend => 2,
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
    /// Abre um escopo de bloco.
    EnterScope,
    /// Fecha o escopo de bloco atual.
    ExitScope,
    /// Cria `Func` de `funcs[idx]` e declara no escopo atual.
    DefineFunc(u32),
    /// Chama a funcao no topo (apos `aridade` argumentos); empilha retorno.
    Call(u32),
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
        };
        Self(id)
    }
}

/// Chunk de bytecode: constantes, nomes, funcoes e codigo.
#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub constants: Vec<Value>,
    pub names: Vec<String>,
    pub funcs: Vec<FuncDef>,
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
                | Op::StoreIndex(idx)
                    if (idx as usize) >= self.names.len() =>
                {
                    return Err(CompileError::new(format!(
                        "chunk invalido em `{context}`: nome {idx} fora do pool"
                    )));
                }
                Op::DefineFunc(idx) if (idx as usize) >= self.funcs.len() => {
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
                _ => {}
            }
        }
        check_stack_flow(&self.code, context)?;
        Ok(())
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
fn check_stack_flow(code: &[Op], context: &str) -> Result<(), CompileError> {
    let mut entry: Vec<Option<i64>> = vec![None; code.len()];
    entry[0] = Some(0);
    let mut worklist = vec![0usize];
    while let Some(ip) = worklist.pop() {
        let height = entry[ip].expect("posicao alcancavel tem altura");
        let op = code[ip];
        // Retorna (altura apos a instrucao, alvos; `None` = terminal).
        let (after, targets): (i64, Option<Vec<usize>>) = match op {
            Op::Constant(_) | Op::LoadName(_) => (height + 1, Some(vec![ip + 1])),
            Op::Print | Op::Pop | Op::StoreName(_) | Op::DeclareName(_) => {
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
            Op::Halt | Op::Return => (height, None),
            Op::Call(arity) => {
                let arity = arity as usize;
                check_depth(height, arity as i64 + 1, op, context)?;
                (height - arity as i64, Some(vec![ip + 1]))
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
    finally_body: Option<Vec<Stmt>>,
    /// `true` enquanto o corpo protegido compila (handler ativo no
    /// runtime); `false` nas regioes de `catch`/`finally` (handler ja
    /// consumido pelo unwinding ou pelo `TryEnd` do caminho normal).
    handler_live: bool,
    /// `loops.len()` ao abrir o `try`: desvios miram o laco do topo, logo
    /// so cruzam `try`s abertos dentro dele.
    loop_depth: usize,
}

/// Contexto de compilacao de um laco (para `break`/`continue`).
struct LoopCtx {
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

    fn compile_stmts(&mut self, stmts: &[Stmt]) -> Result<(), CompileError> {
        for stmt in stmts {
            self.compile_stmt(stmt)?;
        }
        Ok(())
    }

    /// Nome oculto do laco `for` (lista ou indice): contem NUL, impossivel
    /// de escrever no fonte, logo nunca colide com variaveis do usuario.
    fn hidden_for_name(&mut self, what: &str) -> String {
        let id = self.for_counter;
        self.for_counter += 1;
        format!("\0for:{what}:{id}")
    }

    /// Compila `for VAR in ITER BLOCO`.
    ///
    /// Estrategia (sem runtime novo): avalia `ITER` uma vez, normaliza com
    /// `IterList` (array = copia; mapa = chaves ordenadas) e itera por
    /// indice, como um `while` com incremento. `VAR` e declarada a cada
    /// iteracao no escopo do corpo. `break` fecha corpo + laco; `continue`
    /// salta ao incremento.
    fn compile_for(&mut self, var: &str, iter: &Expr, body: &[Stmt]) -> Result<(), CompileError> {
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
    fn emit_finally_copy(&mut self, body: &[Stmt]) -> Result<(), CompileError> {
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
        let mut copies: Vec<Vec<Stmt>> = Vec::new();
        let mut i = self.pending_trys.len();
        while i > 0 {
            i -= 1;
            let apply = match crossing_loop {
                Some(depth) => self.pending_trys[i].loop_depth == depth,
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
        body: &[Stmt],
        catch_var: &Option<String>,
        catch_body: &Option<Vec<Stmt>>,
        finally_body: &Option<Vec<Stmt>>,
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
        then_branch: &[Stmt],
        else_branch: &Option<Vec<Stmt>>,
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
            Stmt::Break => {
                let Some(loop_ctx) = self.loops.last() else {
                    return Err(CompileError::new("`break` fora de laco".to_string()));
                };
                let break_depth = loop_ctx.break_depth;
                let loop_level = self.loops.len();
                // Fecha `try`s cruzados (handler + `finally`) antes de sair.
                self.emit_abrupt_exits(Some(loop_level))?;
                while self.scopes.len() > break_depth {
                    self.emit_scope_close()?;
                }
                let pos = self.emit(Op::Jump(u32::MAX));
                if let Some(top) = self.loops.last_mut() {
                    top.breaks.push(pos);
                }
            }
            Stmt::Continue => {
                let (continue_depth, start) = match self.loops.last() {
                    Some(loop_ctx) => (loop_ctx.continue_depth, loop_ctx.start),
                    None => return Err(CompileError::new("`continue` fora de laco".to_string())),
                };
                let loop_level = self.loops.len();
                self.emit_abrupt_exits(Some(loop_level))?;
                while self.scopes.len() > continue_depth {
                    self.emit_scope_close()?;
                }
                match start {
                    Some(target) => {
                        self.emit(Op::Jump(target));
                    }
                    None => {
                        let pos = self.emit(Op::Jump(u32::MAX));
                        if let Some(top) = self.loops.last_mut() {
                            top.continues.push(pos);
                        }
                    }
                }
            }
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
            Stmt::FnDef { name, params, body } => {
                let mut seen = std::collections::HashSet::new();
                for param in params {
                    if !seen.insert(param) {
                        return Err(CompileError::new(format!("parametro duplicado: {param}")));
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
                self.chunk.funcs.push(FuncDef {
                    name: name.clone(),
                    params: params.clone(),
                    body: body_chunk,
                });
                let func_idx = (self.chunk.funcs.len() - 1) as u32;
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
                    if args.len() != builtin_arity(builtin) {
                        return Err(CompileError::new(format!(
                            "`{name}` espera {} argumentos, recebeu {}",
                            builtin_arity(builtin),
                            args.len()
                        )));
                    }
                    for arg in args {
                        self.compile_expr(arg)?;
                    }
                    self.emit(Op::CallBuiltin(BuiltinId::from(builtin)));
                } else {
                    let name_idx = self.intern_name(name)?;
                    self.emit(Op::LoadName(name_idx));
                    for arg in args {
                        self.compile_expr(arg)?;
                    }
                    if args.len() > u32::MAX as usize {
                        return Err(CompileError::new("numero de argumentos excedido"));
                    }
                    self.emit(Op::Call(args.len() as u32));
                }
            }
        }
        Ok(())
    }
}

/// Indica se a execucao nunca passa do fim desta lista de instrucoes
/// (termina em `break`, `continue` ou `return`, considerando `if`/`{}`).
/// Usado para nao emitir fechamento de escopo em codigo inalancavel.
fn diverges(stmts: &[Stmt]) -> bool {
    match stmts.last() {
        Some(Stmt::Break) | Some(Stmt::Continue) | Some(Stmt::Return { .. }) => true,
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
            statements: vec![Stmt::Echo {
                expr: Expr::StringLiteral("Hello from RHP!".to_string()),
            }],
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
        let chunk = compile(&Program { statements: vec![] }).expect("deve passar");
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
            body: Chunk::new(),
        });
        assert_eq!(Value::Func(func).to_string(), "<fn f>");
    }
}
