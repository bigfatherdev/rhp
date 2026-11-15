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
        _ => None,
    }
}

#[must_use]
pub const fn builtin_arity(builtin: Builtin) -> usize {
    match builtin {
        Builtin::Len
        | Builtin::Pop
        | Builtin::Keys
        | Builtin::Str
        | Builtin::Int
        | Builtin::Float
        | Builtin::Type
        | Builtin::Assert
        | Builtin::HttpGet => 1,
        Builtin::Push => 2,
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
                Op::JumpIfFalse(target) | Op::Jump(target) if (target as usize) >= len => {
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
            Op::Neg | Op::Not => {
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
            | Op::Index => {
                check_depth(height, 2, op, context)?;
                (height - 1, Some(vec![ip + 1]))
            }
            Op::StoreIndex(_) => {
                check_depth(height, 2, op, context)?;
                (height - 2, Some(vec![ip + 1]))
            }
            Op::JumpIfFalse(target) => {
                check_depth(height, 1, op, context)?;
                (height - 1, Some(vec![target as usize, ip + 1]))
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
            Op::Jump(_) => self.chunk.code[pos] = Op::Jump(target),
            _ => debug_assert!(false, "patch_jump em posicao sem salto"),
        }
    }

    fn compile_stmts(&mut self, stmts: &[Stmt]) -> Result<(), CompileError> {
        for stmt in stmts {
            self.compile_stmt(stmt)?;
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
            Stmt::Assign { target, expr } => match target {
                AssignTarget::Var(name) => {
                    self.compile_expr(expr)?;
                    let name_idx = self.intern_name(name)?;
                    self.emit(Op::StoreName(name_idx));
                }
                AssignTarget::Index { name, index } => {
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
            } => {
                self.compile_expr(cond)?;
                let else_jump = self.emit(Op::JumpIfFalse(u32::MAX));
                self.emit(Op::EnterScope);
                self.compile_stmts(then_branch)?;
                self.emit(Op::ExitScope);
                match else_branch {
                    Some(else_stmts) => {
                        let end_jump = self.emit(Op::Jump(u32::MAX));
                        let else_target = self.chunk.code.len() as u32;
                        self.patch_jump(else_jump, else_target);
                        self.emit(Op::EnterScope);
                        self.compile_stmts(else_stmts)?;
                        self.emit(Op::ExitScope);
                        let end_target = self.chunk.code.len() as u32;
                        self.patch_jump(end_jump, end_target);
                    }
                    None => {
                        let end_target = self.chunk.code.len() as u32;
                        self.patch_jump(else_jump, end_target);
                    }
                }
            }
            Stmt::While { cond, body } => {
                let loop_start = self.chunk.code.len() as u32;
                self.compile_expr(cond)?;
                let exit_jump = self.emit(Op::JumpIfFalse(u32::MAX));
                self.emit(Op::EnterScope);
                self.compile_stmts(body)?;
                self.emit(Op::ExitScope);
                self.emit(Op::Jump(loop_start));
                let exit_target = self.chunk.code.len() as u32;
                self.patch_jump(exit_jump, exit_target);
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
                match expr {
                    Some(expr) => self.compile_expr(expr)?,
                    None => {
                        let idx = self.add_constant(Value::Null)?;
                        self.emit(Op::Constant(idx));
                    }
                }
                self.emit(Op::Return);
            }
            Stmt::Block(stmts) => {
                self.emit(Op::EnterScope);
                self.compile_stmts(stmts)?;
                self.emit(Op::ExitScope);
            }
            Stmt::Import(_) => {
                // Resolucao de modulos e papel do carregador (`rhp run`);
                // a compilacao direta ignora (programa ja deve vir resolvido).
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
                };
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
