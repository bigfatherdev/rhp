//! Runtime builtin do RHP: biblioteca-padrao e I/O (`http_get`).
//!
//! Separado do nucleo de execucao (`Vm::step`) por responsabilidade:
//! aqui vivem efeitos (rede, conversoes); la, despacho e pilha.

use super::{type_name, VmError};
use rhp_bytecode::{Builtin, Value};

/// Corpo maximo aceito por `http_get` (protecao contra exaustao de memoria).
pub const MAX_HTTP_BODY: usize = 1024 * 1024;

pub(super) fn call_builtin(builtin: Builtin, args: &[Value]) -> Result<Value, VmError> {
    match builtin {
        Builtin::Len => match &args[0] {
            Value::Array(items) => Ok(Value::Int(items.len() as i64)),
            Value::Map(entries) => Ok(Value::Int(entries.len() as i64)),
            Value::Str(s) => Ok(Value::Int(s.chars().count() as i64)),
            other => Err(VmError::new(format!(
                "`len` nao suporta {}",
                type_name(other)
            ))),
        },
        Builtin::Push => match (&args[0], &args[1]) {
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
        Builtin::Pop => match &args[0] {
            Value::Array(items) => items
                .last()
                .cloned()
                .ok_or_else(|| VmError::new("`pop` de array vazio".to_string())),
            other => Err(VmError::new(format!(
                "`pop` exige array, encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::Keys => match &args[0] {
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
        Builtin::Str => Ok(Value::Str(args[0].to_string())),
        Builtin::Int => match &args[0] {
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
        Builtin::Float => match &args[0] {
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
        Builtin::Type => Ok(Value::Str(type_name(&args[0]).to_string())),
        Builtin::Assert => match &args[0] {
            Value::Bool(true) => Ok(Value::Null),
            Value::Bool(false) => Err(VmError::new("falha de assercao".to_string())),
            other => Err(VmError::new(format!(
                "`assert` exige bool, encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::HttpGet => match &args[0] {
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
pub(super) fn http_get_body(url: &str) -> Result<String, VmError> {
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
