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

mod builtins;
use builtins::call_builtin;
pub use builtins::MAX_HTTP_BODY;

mod json;
mod time;

/// Profundidade maxima da pilha de chamadas (protecao contra estouro).
pub const MAX_CALL_DEPTH: usize = 512;

/// Limites opcionais de execucao (opt-in; padrao = ilimitado).
///
/// Preserva compatibilidade: sem limites configurados o comportamento e o
/// mesmo de antes. `max_steps` aborta loops; `max_stack_depth` limita a
/// pilha de operandos por frame; `max_collection_len` limita tamanho em
/// bytes (strings) ou elementos (arrays/mapas) de cada valor empilhado.
#[derive(Debug, Clone, Default)]
pub struct VmLimits {
    pub max_steps: Option<u64>,
    pub max_stack_depth: Option<usize>,
    pub max_collection_len: Option<usize>,
}

/// Erro de runtime da VM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmError {
    pub message: String,
    /// Codigo de `exit(codigo)` quando o erro e um pedido de saida;
    /// `None` para erros comuns. Leitura publica; construcao interna.
    exit_code: Option<i32>,
}

impl VmError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            exit_code: None,
        }
    }

    /// Pedido de `exit(codigo)` (0–255): desenrola frames como erro, mas a
    /// CLI o traduz em codigo de saida em vez de mensagem de falha.
    pub(crate) fn exit(code: i32) -> Self {
        Self {
            message: format!("exit({code})"),
            exit_code: Some(code),
        }
    }

    /// Codigo de saida pedido por `exit`, se for o caso.
    #[must_use]
    pub fn exit_code(&self) -> Option<i32> {
        self.exit_code
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
    /// Pilha de handlers de `try` (RFC 0001); o mais interno por ultimo.
    handlers: Vec<Handler>,
}

/// Handler de `try`: para onde saltar e com que escopos/pilha.
#[derive(Debug, Clone, Copy)]
struct Handler {
    /// Instrucao do `catch` ou do bloco `finally`+`Rethrow`.
    target: usize,
    /// `scopes.len()` ao registrar (restaura no unwinding).
    scope_len: usize,
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
    argv: Vec<String>,
}

impl Vm {
    #[must_use]
    pub fn new() -> Self {
        Self {
            frames: Vec::new(),
            globals: HashMap::new(),
            limits: VmLimits::default(),
            steps: 0,
            argv: Vec::new(),
        }
    }

    /// Define os argumentos de linha de comando visiveis via `argv()`.
    pub fn set_argv(&mut self, args: Vec<String>) {
        self.argv = args;
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
            handlers: Vec::new(),
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
                self.push_checked(caller, value)?;
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
                            "`for` exige array ou mapa, encontrado {}",
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
                let result = call_builtin(builtin, &args, &self.argv)?;
                self.push_checked(idx, result)?;
                self.frames[idx].ip += 1;
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
        self.frames[idx].stack.push(value);
        Ok(())
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
            if self.frames[idx].stack.len() > max {
                return Err(VmError::new(format!(
                    "pilha de operandos excedeu o limite ({max})"
                )));
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
            handlers: Vec::new(),
        });
        Ok(())
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
            ..Default::default()
        });
        let err = vm.run_to_string(&chunk).expect_err("deve falhar");
        assert!(err.message.contains("limite de instrucoes"));
        // Sem limite, programa finito passa normalmente.
        let out = run_src("let i = 0; while i < 3 { i = i + 1; } echo i;").expect("ok");
        assert_eq!(out, "3\n");
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
        // `https://` rejeitado (sem TLS nesta fase).
        let err = run_err("echo http_fetch(\"https://x/\", {});");
        assert!(err.contains("http://"), "obteve: {err}");
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
        assert!(err.contains("array ou mapa"), "obteve: {err}");
        let err = run_err("for x in \"ab\" { echo x; }");
        assert!(err.contains("array ou mapa"), "obteve: {err}");
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
