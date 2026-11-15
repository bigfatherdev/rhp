//! Maquina virtual baseada em pilha para o RHP (Fase 2 / Marco 2).
//!
//! Executa [`Chunk`] validado com frames explicitos, escopos lexicos e
//! globais compartilhadas. O bytecode e tratado como entrada potencialmente
//! nao confiavel e sempre validado (recursivamente) antes da execucao.

use rhp_bytecode::{builtin_arity, Chunk, FuncDef, Op, Value};
use std::collections::HashMap;
use std::fmt;
use std::io::Write;
use std::rc::Rc;

/// Profundidade maxima da pilha de chamadas (protecao contra estouro).
pub const MAX_CALL_DEPTH: usize = 512;

/// Corpo maximo aceito por `http_get` (protecao contra exaustao de memoria).
pub const MAX_HTTP_BODY: usize = 1024 * 1024;

/// Limites opcionais de execucao (opt-in; padrao = ilimitado).
///
/// Preserva compatibilidade: sem limites configurados o comportamento e o
/// mesmo de antes. `max_steps` conta instrucoes executadas em todos os
/// frames e aborta com erro limpo ao exceder (protecao contra loops
/// infinitos e crescimento descontrolado).
#[derive(Debug, Clone, Default)]
pub struct VmLimits {
    pub max_steps: Option<u64>,
}

/// Erro de runtime da VM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmError {
    pub message: String,
}

impl VmError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for VmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for VmError {}

/// Frame de execucao: uma ativacao de chunk (principal ou funcao).
#[derive(Debug)]
struct Frame {
    code: Vec<Op>,
    constants: Vec<Value>,
    names: Vec<String>,
    funcs: Vec<Rc<FuncDef>>,
    ip: usize,
    stack: Vec<Value>,
    /// Pilha de escopos locais; `scopes[0]` e o escopo da ativacao
    /// (parametros no caso de funcao).
    scopes: Vec<HashMap<String, Value>>,
}

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
}

impl Vm {
    #[must_use]
    pub fn new() -> Self {
        Self {
            frames: Vec::new(),
            globals: HashMap::new(),
            limits: VmLimits::default(),
            steps: 0,
        }
    }

    /// Define limites opcionais de execucao (ver [`VmLimits`]).
    pub fn set_limits(&mut self, limits: VmLimits) {
        self.limits = limits;
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

        self.frames.clear();
        self.globals.clear();
        self.steps = 0;
        self.frames.push(Frame {
            code: chunk.code.clone(),
            constants: chunk.constants.clone(),
            names: chunk.names.clone(),
            funcs: chunk.funcs.iter().map(|f| Rc::new(f.clone())).collect(),
            ip: 0,
            stack: Vec::new(),
            scopes: vec![HashMap::new()],
        });

        while !self.frames.is_empty() {
            let idx = self.frames.len() - 1;
            match self.step(idx, output)? {
                Step::Continue => {}
                Step::Done => return Ok(()),
            }
        }
        Err(VmError::new("execucao terminou sem `Halt`"))
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

    fn step(&mut self, idx: usize, output: &mut dyn Write) -> Result<Step, VmError> {
        self.steps += 1;
        if let Some(max) = self.limits.max_steps {
            if self.steps > max {
                return Err(VmError::new(format!(
                    "limite de instrucoes excedido ({max} passos)"
                )));
            }
        }
        let op = {
            let frame = self
                .frames
                .get(idx)
                .ok_or_else(|| VmError::new("frame de execucao inexistente"))?;
            *frame
                .code
                .get(frame.ip)
                .ok_or_else(|| VmError::new("ponteiro de instrucao fora do codigo"))?
        };
        match op {
            Op::Constant(const_idx) => {
                let value = self.frames[idx]
                    .constants
                    .get(const_idx as usize)
                    .cloned()
                    .ok_or_else(|| VmError::new(format!("constante {const_idx} inexistente")))?;
                self.frames[idx].stack.push(value);
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
            Op::Neg => {
                let value = self.pop(idx, "operador `-`")?;
                let result = neg_value(&value)?;
                self.frames[idx].stack.push(result);
                self.frames[idx].ip += 1;
            }
            Op::Not => {
                let value = self.pop(idx, "operador `!`")?;
                let result = not_value(&value)?;
                self.frames[idx].stack.push(result);
                self.frames[idx].ip += 1;
            }
            Op::Eq => self.binary(idx, "comparacao", |a, b| {
                Ok(Value::Bool(values_equal(a, b)))
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
            Op::Jump(target) => {
                self.frames[idx].ip = target as usize;
            }
            Op::LoadName(name_idx) => {
                let name = self.name_of(idx, name_idx)?;
                let value = self
                    .lookup(idx, &name)
                    .ok_or_else(|| VmError::new(format!("variavel nao definida: {name}")))?;
                self.frames[idx].stack.push(value);
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
                    .funcs
                    .get(func_idx as usize)
                    .cloned()
                    .ok_or_else(|| VmError::new(format!("funcao {func_idx} inexistente")))?;
                let name = func.name.clone();
                self.declare(idx, &name, Value::Func(func))?;
                self.frames[idx].ip += 1;
            }
            Op::Call(arity) => {
                self.call(idx, arity)?;
            }
            Op::Return => {
                let value = self.pop(idx, "`return`")?;
                if self.frames.len() == 1 {
                    return Err(VmError::new("`return` fora de funcao"));
                }
                self.frames.pop();
                let caller = self.frames.len() - 1;
                self.frames[caller].stack.push(value);
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
                self.frames[idx].stack.push(Value::Array(items));
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
                self.frames[idx].stack.push(Value::Map(entries));
                self.frames[idx].ip += 1;
            }
            Op::Index => {
                let index = self.pop(idx, "indice")?;
                let base = self.pop(idx, "indexacao")?;
                let result = index_value(&base, &index)?;
                self.frames[idx].stack.push(result);
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
                if !self.assign(idx, &name, updated) {
                    return Err(VmError::new(format!("variavel nao definida: {name}")));
                }
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
                let result = call_builtin(builtin, &args)?;
                self.frames[idx].stack.push(result);
                self.frames[idx].ip += 1;
            }
        }
        Ok(Step::Continue)
    }

    fn pop(&mut self, idx: usize, context: &str) -> Result<Value, VmError> {
        self.frames[idx]
            .stack
            .pop()
            .ok_or_else(|| VmError::new(format!("pilha vazia em {context}")))
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
        self.frames[idx].stack.push(result);
        self.frames[idx].ip += 1;
        Ok(())
    }

    fn name_of(&self, idx: usize, name_idx: u32) -> Result<String, VmError> {
        self.frames[idx]
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
        self.globals.get(name).cloned()
    }

    /// Atribui a variavel existente; retorna `false` se nao encontrada.
    fn assign(&mut self, idx: usize, name: &str, value: Value) -> bool {
        for scope in self.frames[idx].scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                scope.insert(name.to_string(), value);
                return true;
            }
        }
        if self.globals.contains_key(name) {
            self.globals.insert(name.to_string(), value);
            return true;
        }
        false
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
        if func.params.len() != arity {
            return Err(VmError::new(format!(
                "funcao `{}` espera {} argumentos, recebeu {}",
                func.name,
                func.params.len(),
                arity
            )));
        }
        if self.frames.len() >= MAX_CALL_DEPTH {
            return Err(VmError::new(
                "limite de chamadas excedido (recursao muito profunda?)",
            ));
        }
        let mut scope = HashMap::with_capacity(func.params.len());
        for (param, arg) in func.params.iter().zip(args) {
            scope.insert(param.clone(), arg);
        }
        self.frames[idx].ip += 1;
        self.frames.push(Frame {
            code: func.body.code.clone(),
            constants: func.body.constants.clone(),
            names: func.body.names.clone(),
            funcs: func.body.funcs.iter().map(|f| Rc::new(f.clone())).collect(),
            ip: 0,
            stack: Vec::new(),
            scopes: vec![scope],
        });
        Ok(())
    }
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
        (Value::Func(a), Value::Func(b)) => Rc::ptr_eq(a, b),
        _ => false,
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
        _ => Err(VmError::new(format!(
            "atribuicao indexada nao suportada para {}[{}]",
            type_name(container),
            type_name(index)
        ))),
    }
}

fn call_builtin(builtin: rhp_bytecode::Builtin, args: &[Value]) -> Result<Value, VmError> {
    match builtin {
        rhp_bytecode::Builtin::Len => match &args[0] {
            Value::Array(items) => Ok(Value::Int(items.len() as i64)),
            Value::Map(entries) => Ok(Value::Int(entries.len() as i64)),
            Value::Str(s) => Ok(Value::Int(s.chars().count() as i64)),
            other => Err(VmError::new(format!(
                "`len` nao suporta {}",
                type_name(other)
            ))),
        },
        rhp_bytecode::Builtin::Push => match (&args[0], &args[1]) {
            (Value::Array(items), item) => {
                let mut updated = items.clone();
                updated.push(item.clone());
                Ok(Value::Array(updated))
            }
            (other, _) => Err(VmError::new(format!(
                "`push` exige array, encontrado {}",
                type_name(other)
            ))),
        },
        rhp_bytecode::Builtin::Pop => match &args[0] {
            Value::Array(items) => items
                .last()
                .cloned()
                .ok_or_else(|| VmError::new("`pop` de array vazio".to_string())),
            other => Err(VmError::new(format!(
                "`pop` exige array, encontrado {}",
                type_name(other)
            ))),
        },
        rhp_bytecode::Builtin::Keys => match &args[0] {
            Value::Map(entries) => {
                let mut keys: Vec<String> = entries.keys().cloned().collect();
                keys.sort();
                Ok(Value::Array(keys.into_iter().map(Value::Str).collect()))
            }
            other => Err(VmError::new(format!(
                "`keys` exige mapa, encontrado {}",
                type_name(other)
            ))),
        },
        rhp_bytecode::Builtin::Str => Ok(Value::Str(args[0].to_string())),
        rhp_bytecode::Builtin::Int => match &args[0] {
            Value::Int(v) => Ok(Value::Int(*v)),
            Value::Float(v) => {
                if v.fract() != 0.0 || *v >= 2f64.powi(63) || *v < i64::MIN as f64 {
                    return Err(VmError::new(format!("`int` nao converte {v}")));
                }
                Ok(Value::Int(*v as i64))
            }
            Value::Str(s) => s
                .trim()
                .parse::<i64>()
                .map(Value::Int)
                .map_err(|_| VmError::new(format!("`int` nao converte {s:?}"))),
            other => Err(VmError::new(format!(
                "`int` nao converte {}",
                type_name(other)
            ))),
        },
        rhp_bytecode::Builtin::Float => match &args[0] {
            Value::Float(v) => Ok(Value::Float(*v)),
            Value::Int(v) => Ok(Value::Float(*v as f64)),
            Value::Str(s) => s
                .trim()
                .parse::<f64>()
                .map(Value::Float)
                .map_err(|_| VmError::new(format!("`float` nao converte {s:?}"))),
            other => Err(VmError::new(format!(
                "`float` nao converte {}",
                type_name(other)
            ))),
        },
        rhp_bytecode::Builtin::Type => Ok(Value::Str(type_name(&args[0]).to_string())),
        rhp_bytecode::Builtin::Assert => match &args[0] {
            Value::Bool(true) => Ok(Value::Null),
            Value::Bool(false) => Err(VmError::new("falha de assercao".to_string())),
            other => Err(VmError::new(format!(
                "`assert` exige bool, encontrado {}",
                type_name(other)
            ))),
        },
        rhp_bytecode::Builtin::HttpGet => match &args[0] {
            Value::Str(url) => Ok(Value::Str(http_get_body(url)?)),
            other => Err(VmError::new(format!(
                "`http_get` exige string (url), encontrado {}",
                type_name(other)
            ))),
        },
    }
}

/// Cliente HTTP minimo (somente `http://`), sem dependencias externas.
///
/// Timeouts de 5s para conexao e leitura; aceita apenas status 2xx e
/// corpo UTF-8 de ate [`MAX_HTTP_BODY`] bytes. A resolucao DNS usa o
/// resolvedor do sistema e nao tem timeout proprio (limitacao documentada).
/// Erros sao `VmError` com mensagem (sem panics).
fn http_get_body(url: &str) -> Result<String, VmError> {
    use std::io::{Read, Write as _};
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::Duration;

    const TIMEOUT: Duration = Duration::from_secs(5);
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| VmError::new("`http_get` suporta apenas urls `http://`".to_string()))?;
    let (authority, path) = match rest.find('/') {
        Some(pos) => (&rest[..pos], &rest[pos..]),
        None => (rest, "/"),
    };
    if authority.is_empty() {
        return Err(VmError::new("`http_get` com host vazio".to_string()));
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port_text)) if !host.is_empty() => {
            let port = port_text
                .parse::<u16>()
                .map_err(|_| VmError::new(format!("`http_get` com porta invalida: {port_text}")))?;
            (host, port)
        }
        _ => (authority, 80),
    };
    let addr = (host, port)
        .to_socket_addrs()
        .map_err(|e| VmError::new(format!("`http_get` falhou ao resolver {host}: {e}")))?
        .next()
        .ok_or_else(|| VmError::new(format!("`http_get` sem endereco para {host}")))?;
    let mut stream = TcpStream::connect_timeout(&addr, TIMEOUT).map_err(|e| {
        VmError::new(format!(
            "`http_get` falhou ao conectar em {host}:{port}: {e}"
        ))
    })?;
    stream
        .set_read_timeout(Some(TIMEOUT))
        .map_err(|e| VmError::new(format!("`http_get` falhou ao configurar timeout: {e}")))?;
    let request = format!("GET {path} HTTP/1.0\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .map_err(|e| VmError::new(format!("`http_get` falhou ao enviar requisicao: {e}")))?;
    let mut raw: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                raw.extend_from_slice(&chunk[..n]);
                if raw.len() > MAX_HTTP_BODY {
                    return Err(VmError::new(format!(
                        "`http_get` excedeu o limite de corpo ({MAX_HTTP_BODY} bytes)"
                    )));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => {
                return Err(VmError::new(format!(
                    "`http_get` falhou ao ler resposta: {e}"
                )));
            }
        }
    }
    let header_end = raw
        .windows(b"\r\n\r\n".len())
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| VmError::new("`http_get` com resposta sem cabecalhos".to_string()))?;
    let header = String::from_utf8_lossy(&raw[..header_end]);
    let status_line = header.lines().next().unwrap_or("");
    let code = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| VmError::new(format!("`http_get` com status invalido: {status_line:?}")))?;
    if !(200..300).contains(&code) {
        return Err(VmError::new(format!("`http_get` retornou status {code}")));
    }
    String::from_utf8(raw[header_end + 4..].to_vec())
        .map_err(|e| VmError::new(format!("`http_get` com corpo nao UTF-8: {e}")))
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
    fn reports_call_errors() {
        assert!(run_err("echo f();").contains("nao definida"));
        assert!(run_err("let f = 1; echo f();").contains("como funcao"));
        assert!(run_err("fn f(a) { return a; } echo f(1, 2);").contains("espera 1"));
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
        let err = Vm::new().run_to_string(&chunk).expect_err("deve falhar");
        assert!(err.message.contains("pilha vazia"));
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

    /// Gera expressao de `+`/`*` sobre 0..=9 com valor de referencia.
    /// Faixa sem overflow possivel: so soma/multiplicacao de digitos.
    fn gen_arith(rng: &mut Lcg, depth: u32) -> (String, i64) {
        if depth == 0 || rng.next(3) == 0 {
            let v = rng.next(10) as i64;
            return (v.to_string(), v);
        }
        let (l_src, l_val) = gen_arith(rng, depth - 1);
        let (r_src, r_val) = gen_arith(rng, depth - 1);
        if rng.next(2) == 0 {
            (format!("({l_src} + {r_src})"), l_val + r_val)
        } else {
            (format!("({l_src} * {r_src})"), l_val * r_val)
        }
    }

    #[test]
    fn property_arithmetic_matches_reference() {
        let mut rng = Lcg(0x1234_5678_9ABC_DEF0);
        for _ in 0..200 {
            let (src, expected) = gen_arith(&mut rng, 4);
            let got = run_src(&format!("echo {src};")).expect("programa gerado deve rodar");
            assert_eq!(got, format!("{expected}\n"), "fonte: {src}");
        }
    }

    #[test]
    fn respects_step_limit() {
        let chunk = compile_src("let i = 0; while true { i = i + 1; }");
        let mut vm = Vm::new();
        vm.set_limits(VmLimits {
            max_steps: Some(1000),
        });
        let err = vm.run_to_string(&chunk).expect_err("deve falhar");
        assert!(err.message.contains("limite de instrucoes"));
        // Sem limite, programa finito passa normalmente.
        let out = run_src("let i = 0; while i < 3 { i = i + 1; } echo i;").expect("ok");
        assert_eq!(out, "3\n");
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
