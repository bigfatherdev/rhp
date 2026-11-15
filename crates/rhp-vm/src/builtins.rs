//! Runtime builtin do RHP: biblioteca-padrao e I/O (`http_get`).
//!
//! Separado do nucleo de execucao (`Vm::step`) por responsabilidade:
//! aqui vivem efeitos (rede, conversoes); la, despacho e pilha.

use super::{type_name, VmError};
use rhp_bytecode::{Builtin, Value};

/// Corpo maximo aceito por `http_get` (protecao contra exaustao de memoria).
pub const MAX_HTTP_BODY: usize = 1024 * 1024;

/// Teto de leitura de `fs_read` (protecao contra exaustao de memoria).
pub const MAX_FILE_BYTES: usize = 8 * 1024 * 1024;

pub(super) fn call_builtin(
    builtin: Builtin,
    args: &[Value],
    argv: &[String],
) -> Result<Value, VmError> {
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
        Builtin::Contains => match (&args[0], &args[1]) {
            (Value::Str(hay), Value::Str(needle)) => Ok(Value::Bool(hay.contains(needle as &str))),
            (other, _) => Err(VmError::new(format!(
                "`contains` exige strings, encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::StartsWith => match (&args[0], &args[1]) {
            (Value::Str(hay), Value::Str(prefix)) => {
                Ok(Value::Bool(hay.starts_with(prefix as &str)))
            }
            (other, _) => Err(VmError::new(format!(
                "`starts_with` exige strings, encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::EndsWith => match (&args[0], &args[1]) {
            (Value::Str(hay), Value::Str(suffix)) => Ok(Value::Bool(hay.ends_with(suffix as &str))),
            (other, _) => Err(VmError::new(format!(
                "`ends_with` exige strings, encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::Split => match (&args[0], &args[1]) {
            (Value::Str(text), Value::Str(sep)) => {
                let parts: Vec<Value> = if sep.is_empty() {
                    text.chars().map(|c| Value::Str(c.to_string())).collect()
                } else {
                    text.split(sep as &str)
                        .map(|s| Value::Str(s.to_string()))
                        .collect()
                };
                Ok(Value::Array(parts))
            }
            (other, _) => Err(VmError::new(format!(
                "`split` exige strings, encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::Join => match (&args[0], &args[1]) {
            (Value::Array(items), Value::Str(sep)) => {
                let mut parts: Vec<&str> = Vec::with_capacity(items.len());
                for item in items {
                    match item {
                        Value::Str(s) => parts.push(s),
                        other => {
                            return Err(VmError::new(format!(
                                "`join` exige array de strings, encontrado {}",
                                type_name(other)
                            )));
                        }
                    }
                }
                Ok(Value::Str(parts.join(sep as &str)))
            }
            (other, _) => Err(VmError::new(format!(
                "`join` exige (array, string), encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::Trim => match &args[0] {
            Value::Str(s) => Ok(Value::Str(s.trim().to_string())),
            other => Err(VmError::new(format!(
                "`trim` exige string, encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::Upper => match &args[0] {
            Value::Str(s) => Ok(Value::Str(s.to_uppercase())),
            other => Err(VmError::new(format!(
                "`upper` exige string, encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::Lower => match &args[0] {
            Value::Str(s) => Ok(Value::Str(s.to_lowercase())),
            other => Err(VmError::new(format!(
                "`lower` exige string, encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::Replace => match (&args[0], &args[1], &args[2]) {
            (Value::Str(text), Value::Str(from), Value::Str(to)) => {
                Ok(Value::Str(text.replace(from as &str, to as &str)))
            }
            (other, _, _) => Err(VmError::new(format!(
                "`replace` exige (string, string, string), encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::Abs => match &args[0] {
            Value::Int(v) => v
                .checked_abs()
                .map(Value::Int)
                .ok_or_else(|| VmError::new("estouro de inteiro em `abs`".to_string())),
            Value::Float(v) => Ok(Value::Float(v.abs())),
            other => Err(VmError::new(format!(
                "`abs` exige numero, encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::Min => match (&args[0], &args[1]) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(*a.min(b))),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.min(*b))),
            (Value::Int(a), Value::Float(b)) => Ok(Value::Float((*a as f64).min(*b))),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a.min(*b as f64))),
            (other, _) => Err(VmError::new(format!(
                "`min` exige numeros, encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::Max => match (&args[0], &args[1]) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(*a.max(b))),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.max(*b))),
            (Value::Int(a), Value::Float(b)) => Ok(Value::Float((*a as f64).max(*b))),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a.max(*b as f64))),
            (other, _) => Err(VmError::new(format!(
                "`max` exige numeros, encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::Sqrt => match &args[0] {
            Value::Int(v) if *v >= 0 => Ok(Value::Float((*v as f64).sqrt())),
            Value::Float(v) if *v >= 0.0 => Ok(Value::Float(v.sqrt())),
            Value::Int(_) | Value::Float(_) => {
                Err(VmError::new("`sqrt` de numero negativo".to_string()))
            }
            other => Err(VmError::new(format!(
                "`sqrt` exige numero, encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::Pow => match (&args[0], &args[1]) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Float((*a as f64).powf(*b as f64))),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.powf(*b))),
            (Value::Int(a), Value::Float(b)) => Ok(Value::Float((*a as f64).powf(*b))),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a.powf(*b as f64))),
            (other, _) => Err(VmError::new(format!(
                "`pow` exige numeros, encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::Floor => match &args[0] {
            Value::Int(v) => Ok(Value::Int(*v)),
            Value::Float(v) => {
                if !v.is_finite() || *v >= 2f64.powi(63) || *v < -(2f64.powi(63)) {
                    return Err(VmError::new(format!("`floor` fora do intervalo: {v}")));
                }
                Ok(Value::Int(v.floor() as i64))
            }
            other => Err(VmError::new(format!(
                "`floor` exige numero, encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::Ceil => match &args[0] {
            Value::Int(v) => Ok(Value::Int(*v)),
            Value::Float(v) => {
                if !v.is_finite() || *v >= 2f64.powi(63) || *v < -(2f64.powi(63)) {
                    return Err(VmError::new(format!("`ceil` fora do intervalo: {v}")));
                }
                Ok(Value::Int(v.ceil() as i64))
            }
            other => Err(VmError::new(format!(
                "`ceil` exige numero, encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::Values => match &args[0] {
            Value::Map(entries) => {
                let mut keys: Vec<&String> = entries.keys().collect();
                keys.sort();
                Ok(Value::Array(
                    keys.into_iter()
                        .map(|k| entries.get(k).expect("chave existe").clone())
                        .collect(),
                ))
            }
            other => Err(VmError::new(format!(
                "`values` exige mapa, encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::Entries => match &args[0] {
            Value::Map(entries) => {
                let mut keys: Vec<&String> = entries.keys().collect();
                keys.sort();
                Ok(Value::Array(
                    keys.into_iter()
                        .map(|k| {
                            Value::Array(vec![
                                Value::Str(k.clone()),
                                entries.get(k).expect("chave existe").clone(),
                            ])
                        })
                        .collect(),
                ))
            }
            other => Err(VmError::new(format!(
                "`entries` exige mapa, encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::Merge => match (&args[0], &args[1]) {
            (Value::Map(a), Value::Map(b)) => {
                let mut merged = a.clone();
                for (k, v) in b {
                    merged.insert(k.clone(), v.clone());
                }
                Ok(Value::Map(merged))
            }
            (other, _) => Err(VmError::new(format!(
                "`merge` exige dois mapas, encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::Has => match (&args[0], &args[1]) {
            (Value::Map(entries), Value::Str(key)) => Ok(Value::Bool(entries.contains_key(key))),
            (other, _) => Err(VmError::new(format!(
                "`has` exige (mapa, string), encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::Assert => match &args[0] {
            Value::Bool(true) => Ok(Value::Null),
            Value::Bool(false) => Err(VmError::new("falha de assercao".to_string())),
            other => Err(VmError::new(format!(
                "`assert` exige bool, encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::Sort => match &args[0] {
            Value::Array(items) => Ok(Value::Array(sort_array(items)?)),
            other => Err(VmError::new(format!(
                "`sort` exige array, encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::Exit => match &args[0] {
            Value::Int(code) if (0..=255).contains(code) => Err(VmError::exit(*code as i32)),
            Value::Int(code) => Err(VmError::new(format!(
                "`exit` exige codigo entre 0 e 255, encontrado {code}"
            ))),
            other => Err(VmError::new(format!(
                "`exit` exige codigo inteiro, encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::FsRead => match &args[0] {
            Value::Str(path) => {
                let bytes = std::fs::read(path)
                    .map_err(|e| VmError::new(format!("`fs_read` falhou em `{path}`: {e}")))?;
                if bytes.len() > MAX_FILE_BYTES {
                    return Err(VmError::new(format!(
                        "`fs_read` excedeu o limite ({MAX_FILE_BYTES} bytes): {path}"
                    )));
                }
                String::from_utf8(bytes)
                    .map(Value::Str)
                    .map_err(|e| VmError::new(format!("`fs_read` com conteudo nao UTF-8: {e}")))
            }
            other => Err(VmError::new(format!(
                "`fs_read` exige caminho (string), encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::FsWrite | Builtin::FsAppend => {
            let append = matches!(builtin, Builtin::FsAppend);
            let name = if append { "fs_append" } else { "fs_write" };
            match (&args[0], &args[1]) {
                (Value::Str(path), Value::Str(content)) => {
                    use std::io::Write as _;
                    let mut opts = std::fs::OpenOptions::new();
                    opts.write(true)
                        .create(true)
                        .truncate(!append)
                        .append(append);
                    let mut file = opts
                        .open(path)
                        .map_err(|e| VmError::new(format!("`{name}` falhou em `{path}`: {e}")))?;
                    file.write_all(content.as_bytes())
                        .map_err(|e| VmError::new(format!("`{name}` falhou em `{path}`: {e}")))?;
                    Ok(Value::Int(content.len() as i64))
                }
                (other, _) => Err(VmError::new(format!(
                    "`{name}` exige (caminho, texto), encontrado {}",
                    type_name(other)
                ))),
            }
        }
        Builtin::FsExists => match &args[0] {
            Value::Str(path) => Ok(Value::Bool(std::fs::metadata(path).is_ok())),
            other => Err(VmError::new(format!(
                "`fs_exists` exige caminho (string), encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::Argv => Ok(Value::Array(
            argv.iter().map(|a| Value::Str(a.clone())).collect(),
        )),
        Builtin::Env => match &args[0] {
            Value::Str(name) => Ok(std::env::var(name).map(Value::Str).unwrap_or(Value::Null)),
            other => Err(VmError::new(format!(
                "`env` exige nome (string), encontrado {}",
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

/// Ordena copia do array (estavel). Aceita arrays homogeneos — numeros
/// (`Int`/`Float` misturados comparam exato quando possivel), `Str`
/// (ordem lexicografica de bytes UTF-8) ou `Bool` (`false` < `true`) —
/// e o array vazio. Qualquer mistura com outros tipos e erro.
fn sort_array(items: &[Value]) -> Result<Vec<Value>, VmError> {
    let numeric = items
        .iter()
        .all(|v| matches!(v, Value::Int(_) | Value::Float(_)));
    let strings = items.iter().all(|v| matches!(v, Value::Str(_)));
    let bools = items.iter().all(|v| matches!(v, Value::Bool(_)));
    if !numeric && !strings && !bools {
        return Err(VmError::new(
            "`sort` exige array de numeros, strings ou bools (homogeneo)".to_string(),
        ));
    }
    let mut sorted = items.to_vec();
    sorted.sort_by(compare_sort_values);
    Ok(sorted)
}

fn compare_sort_values(a: &Value, b: &Value) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x.cmp(y),
        (Value::Float(x), Value::Float(y)) => x.total_cmp(y),
        (Value::Int(x), Value::Float(y)) => compare_int_float(*x, *y),
        (Value::Float(x), Value::Int(y)) => compare_int_float(*y, *x).reverse(),
        (Value::Str(x), Value::Str(y)) => x.cmp(y),
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
        // Inalcancavel: misturas sao rejeitadas em `sort_array`.
        _ => Ordering::Equal,
    }
}

/// Compara `Int` com `Float`: exato quando o float e inteiro representavel
/// em `i64`; senao, via `f64` (deterministico; `NaN` vai para o fim,
/// como em `total_cmp`).
fn compare_int_float(i: i64, f: f64) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    if f.is_nan() {
        return Ordering::Less;
    }
    if f.fract() == 0.0 && f >= i64::MIN as f64 && f < 2f64.powi(63) {
        i.cmp(&(f as i64))
    } else {
        (i as f64).total_cmp(&f)
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
