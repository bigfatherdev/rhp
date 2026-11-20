//! Extensoes nativas do RHP (Fase Next-Gen 6).
//!
//! Estrategia estatica (sem `dlopen`/`unsafe`): as extensoes sao modulos Rust
//! compilados junto ao runtime e registrados aqui, com ABI versionada.
//! `import native "ext";` carrega as funcoes como globais RHP.
//!
//! # ABI
//! - `RHP_EXTENSION_ABI: u32 = 1`: versao da interface. Extensoes compiladas
//!   contra outra versao sao recusadas com erro claro.
//! - Funcoes: `fn(Vec<Value>) -> Result<Value, String>` (sincronas, sem I/O
//!   bloqueante longo; para I/O use `async fn` + tarefas).
//! - Tipos suportados nos args/retornos: `Null`, `Bool`, `Int`, `Float`,
//!   `Str`, `Array`, `Map`. Outros (`Func`, `Classe`, `Instancia`, `Gerador`)
//!   sao recusados na fronteira com erro claro (sem serializacao implicita).
//! - Erros `Err(String)` viram erro de runtime RHP (capturavel por `try`).
//!
//! # Seguranca
//! - Sem carregamento dinamico: so codigo compilado no binario (sem path
//!   traversal, sem `.so` nao confiavel).
//! - Sem `unsafe` neste modulo; funcoes de extensao sao Rust seguro.
//! - Recursos (arquivos, rede) seguem as mesmas jaulas do runtime.

use super::{type_name, Value};
use std::collections::HashMap;

/// Versao da ABI de extensoes. Incremente em mudanca incompatível.
pub const RHP_EXTENSION_ABI: u32 = 1;

/// Assinatura de funcao nativa.
pub type NativeFn = fn(Vec<Value>) -> Result<Value, String>;

/// Descritor de funcao exposta por uma extensao.
#[derive(Debug, Clone)]
pub struct NativeFunc {
    /// Nome visivel no RHP (global apos `import native`).
    pub name: String,
    /// Aridade fixa (checada antes da chamada).
    pub arity: usize,
    /// Implementacao.
    pub func: NativeFn,
}

/// Descritor de extensao.
#[derive(Debug, Clone)]
pub struct Extension {
    /// Nome do `import native "..."`.
    pub name: String,
    /// ABI com a qual foi compilada (deve ser `RHP_EXTENSION_ABI`).
    pub abi: u32,
    /// Funcoes expostas.
    pub funcs: Vec<NativeFunc>,
}

/// Registro estatico de extensoes (construido sob demanda; sem `unsafe`).
pub fn registry() -> HashMap<String, Extension> {
    let demo = demo_extension();
    let mut map = HashMap::new();
    map.insert(demo.name.clone(), demo);
    map
}

/// Existe a extensao `name`?
pub fn exists(name: &str) -> bool {
    registry().contains_key(name)
}

/// Despacha `ext::func(args)` (aridade ja checada pelo chamador).
pub fn call(ext: &str, func: &str, args: Vec<Value>) -> Result<Value, String> {
    let reg = registry();
    let extension = reg
        .get(ext)
        .ok_or_else(|| format!("extensao nativa desconhecida: `{ext}`"))?;
    if extension.abi != RHP_EXTENSION_ABI {
        return Err(format!(
            "extensao `{ext}` com ABI {} incompativel (esperava {})",
            extension.abi, RHP_EXTENSION_ABI
        ));
    }
    let native = extension
        .funcs
        .iter()
        .find(|f| f.name == func)
        .ok_or_else(|| format!("funcao `{func}` inexistente na extensao `{ext}`"))?;
    if args.len() != native.arity {
        return Err(format!(
            "`{func}` espera {} argumentos, recebeu {}",
            native.arity,
            args.len()
        ));
    }
    for arg in &args {
        match arg {
            Value::Null
            | Value::Bool(_)
            | Value::Int(_)
            | Value::Float(_)
            | Value::Str(_)
            | Value::Array(_)
            | Value::Map(_) => {}
            other => {
                return Err(format!(
                    "extensao `{ext}` nao suporta {} como argumento",
                    type_name(other)
                ));
            }
        }
    }
    let result = (native.func)(args)?;
    match &result {
        Value::Null
        | Value::Bool(_)
        | Value::Int(_)
        | Value::Float(_)
        | Value::Str(_)
        | Value::Array(_)
        | Value::Map(_) => Ok(result),
        other => Err(format!(
            "extensao `{ext}` devolveu {} invalido",
            type_name(other)
        )),
    }
}

// ---------------------------------------------------------------------------
// Extensao de demonstracao `demo`.
// ---------------------------------------------------------------------------

/// `demo`: exemplo minimo (eco, soma, comprimento).
fn demo_extension() -> Extension {
    Extension {
        name: "demo".to_string(),
        abi: RHP_EXTENSION_ABI,
        funcs: vec![
            NativeFunc {
                name: "demo_eco".to_string(),
                arity: 1,
                func: |args| match &args[0] {
                    Value::Str(s) => Ok(Value::Str(s.clone())),
                    other => Err(format!(
                        "`demo_eco` exige string, encontrou {}",
                        type_name(other)
                    )),
                },
            },
            NativeFunc {
                name: "demo_soma".to_string(),
                arity: 2,
                func: |args| match (&args[0], &args[1]) {
                    (Value::Int(a), Value::Int(b)) => a
                        .checked_add(*b)
                        .map(Value::Int)
                        .ok_or_else(|| "estouro de inteiro em `demo_soma`".to_string()),
                    (a, b) => Err(format!(
                        "`demo_soma` exige (int, int), encontrou ({}, {})",
                        type_name(a),
                        type_name(b)
                    )),
                },
            },
            NativeFunc {
                name: "demo_len".to_string(),
                arity: 1,
                func: |args| match &args[0] {
                    Value::Array(items) => Ok(Value::Int(items.len() as i64)),
                    Value::Str(s) => Ok(Value::Int(s.chars().count() as i64)),
                    other => Err(format!(
                        "`demo_len` exige array ou string, encontrou {}",
                        type_name(other)
                    )),
                },
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_functions_dispatch() {
        assert_eq!(
            call("demo", "demo_eco", vec![Value::Str("oi".to_string())]).unwrap(),
            Value::Str("oi".to_string())
        );
        assert_eq!(
            call("demo", "demo_soma", vec![Value::Int(2), Value::Int(3)]).unwrap(),
            Value::Int(5)
        );
        assert!(call("demo", "inexistente", vec![]).is_err());
        assert!(call("nope", "demo_eco", vec![]).is_err());
    }
}
