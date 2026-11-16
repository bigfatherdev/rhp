//! Runtime builtin do RHP: biblioteca-padrao e I/O (`http_get`).
//!
//! Separado do nucleo de execucao (`Vm::step`) por responsabilidade:
//! aqui vivem efeitos (rede, conversoes); la, despacho e pilha.

use super::{type_name, DbConn, DbHandles, VmError};
use rhp_bytecode::{Builtin, Value};
use std::sync::Arc;

/// Corpo maximo aceito por `http_get` (protecao contra exaustao de memoria).
pub const MAX_HTTP_BODY: usize = 1024 * 1024;

/// Teto de cabecalhos por resposta (bytes; antes de achar `\r\n\r\n`).
const MAX_HEADERS: usize = 64 * 1024;

/// Teto de leitura de `fs_read` (protecao contra exaustao de memoria).
pub const MAX_FILE_BYTES: usize = 8 * 1024 * 1024;

pub(super) fn call_builtin(
    builtin: Builtin,
    args: &[Value],
    argv: &[String],
    db: &mut DbHandles,
    rng: &mut u64,
    program: Option<Arc<rhp_parser::Program>>,
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
        Builtin::JsonParse => match &args[0] {
            Value::Str(text) => super::json::parse_json(text),
            other => Err(VmError::new(format!(
                "`json_parse` exige string, encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::JsonStringify => match super::json::stringify_json(&args[0]) {
            Ok(text) => Ok(Value::Str(text)),
            Err(e) => Err(e),
        },
        Builtin::Now => super::time::now_unix(),
        Builtin::Sleep => super::time::sleep_secs(&args[0]),
        Builtin::DateStr => super::time::date_string(&args[0]),
        Builtin::DbOpen => match &args[0] {
            Value::Str(path) => {
                let conn = if let Some(conninfo) = path.strip_prefix("pg:") {
                    super::pg::Connection::connect(conninfo)
                        .map(DbConn::Postgres)
                        .map_err(VmError::new)?
                } else {
                    super::sqlite::Connection::open(path)
                        .map(DbConn::Sqlite)
                        .map_err(VmError::new)?
                };
                let handle = db.next;
                db.next += 1;
                db.conns.insert(handle, conn);
                Ok(Value::Int(handle))
            }
            other => Err(VmError::new(format!(
                "`db_open` exige caminho (string), encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::DbExec => {
            let handle = db_handle(&args[0])?;
            let sql = db_sql(&args[1])?;
            let params = db_params(&args[2])?;
            let conn = db_conn(db, handle)?;
            match conn {
                DbConn::Sqlite(conn) => conn
                    .exec(&sql, &params)
                    .map(Value::Int)
                    .map_err(VmError::new),
                DbConn::Postgres(conn) => {
                    let pg_params = pg_params(&params)?;
                    conn.exec(&sql, &pg_params)
                        .map(Value::Int)
                        .map_err(VmError::new)
                }
            }
        }
        Builtin::DbQuery => {
            let handle = db_handle(&args[0])?;
            let sql = db_sql(&args[1])?;
            let params = db_params(&args[2])?;
            let conn = db_conn(db, handle)?;
            match conn {
                DbConn::Sqlite(conn) => conn
                    .query(&sql, &params)
                    .map(|rows| {
                        Value::Array(
                            rows.into_iter()
                                .map(|row| {
                                    Value::Map(
                                        row.into_iter()
                                            .map(|(k, v)| {
                                                let value = match v {
                                                    super::sqlite::SqlColumn::Int(n) => {
                                                        Value::Int(n)
                                                    }
                                                    super::sqlite::SqlColumn::Float(n) => {
                                                        Value::Float(n)
                                                    }
                                                    super::sqlite::SqlColumn::Text(s) => {
                                                        Value::Str(s)
                                                    }
                                                    super::sqlite::SqlColumn::Null => Value::Null,
                                                };
                                                (k, value)
                                            })
                                            .collect(),
                                    )
                                })
                                .collect(),
                        )
                    })
                    .map_err(VmError::new),
                DbConn::Postgres(conn) => {
                    let pg_params = pg_params(&params)?;
                    conn.query(&sql, &pg_params)
                        .map(|rows| {
                            Value::Array(
                                rows.into_iter()
                                    .map(|row| {
                                        Value::Map(
                                            row.into_iter()
                                                .map(|(k, v)| {
                                                    let value = match v {
                                                        super::pg::PgValue::Int(n) => Value::Int(n),
                                                        super::pg::PgValue::Float(n) => {
                                                            Value::Float(n)
                                                        }
                                                        super::pg::PgValue::Bool(b) => {
                                                            Value::Bool(b)
                                                        }
                                                        super::pg::PgValue::Text(s) => {
                                                            Value::Str(s)
                                                        }
                                                        super::pg::PgValue::Null => Value::Null,
                                                    };
                                                    (k, value)
                                                })
                                                .collect(),
                                        )
                                    })
                                    .collect(),
                            )
                        })
                        .map_err(VmError::new)
                }
            }
        }
        Builtin::DbClose => {
            let handle = db_handle(&args[0])?;
            match db.conns.remove(&handle) {
                Some(DbConn::Sqlite(_)) => Ok(Value::Null),
                Some(DbConn::Postgres(conn)) => {
                    conn.close().map(|()| Value::Null).map_err(VmError::new)
                }
                None => Err(VmError::new(format!(
                    "`db_close`: handle {handle} invalido ou ja fechado"
                ))),
            }
        }
        Builtin::HttpServe => super::http_server::serve_builtin(args, program),
        Builtin::Template => match (&args[0], &args[1]) {
            (Value::Str(template), Value::Map(values)) => {
                Ok(Value::Str(render_template(template, values)?))
            }
            (other, _) => Err(VmError::new(format!(
                "`template` exige (texto, mapa), encontrado {}",
                type_name(other)
            ))),
        },
        // HOFs com callback (RFC 0005) nunca chegam aqui: o `step` intercepta
        // e executa via protocolo de chamada pendente. Defesa em profundidade.
        Builtin::Map | Builtin::Filter | Builtin::Usort => Err(VmError::new(
            "HOF com callback fora do protocolo de execucao (bug interno)".to_string(),
        )),
        // xorshift64* (RFC 0012): rapido, std-only, NAO criptografico.
        // `>> 1` limita a `[0, i64::MAX]` (u64 cru estouraria o `Int`).
        Builtin::Rand => Ok(Value::Int((next_rand(rng) >> 1) as i64)),
        Builtin::RandRange => match (&args[0], &args[1]) {
            (Value::Int(min), Value::Int(max)) if min <= max => {
                let span = (*max as i128 - *min as i128 + 1) as u128;
                let draw = (next_rand(rng) as u128) % span;
                Ok(Value::Int((*min as i128 + draw as i128) as i64))
            }
            (Value::Int(_), Value::Int(_)) => {
                Err(VmError::new("`rand_range` exige min <= max".to_string()))
            }
            (other, _) => Err(VmError::new(format!(
                "`rand_range` exige (min, max) inteiros, encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::FsList => match &args[0] {
            Value::Str(path) => {
                let entries = std::fs::read_dir(path)
                    .map_err(|e| VmError::new(format!("`fs_list` falhou em `{path}`: {e}")))?;
                let mut names: Vec<String> = Vec::new();
                for entry in entries {
                    let entry = entry
                        .map_err(|e| VmError::new(format!("`fs_list` falhou em `{path}`: {e}")))?;
                    names.push(entry.file_name().to_string_lossy().into_owned());
                }
                names.sort();
                Ok(Value::Array(names.into_iter().map(Value::Str).collect()))
            }
            other => Err(VmError::new(format!(
                "`fs_list` exige diretorio (string), encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::RegexMatch => match (&args[0], &args[1]) {
            (Value::Str(haystack), Value::Str(pattern)) => {
                let re = compile_regex(pattern)?;
                Ok(Value::Bool(re.is_match(haystack)))
            }
            (other, _) => Err(VmError::new(format!(
                "`regex_match` exige (texto, padrao) em strings, encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::RegexReplace => match (&args[0], &args[1], &args[2]) {
            (Value::Str(haystack), Value::Str(pattern), Value::Str(replacement)) => {
                let re = compile_regex(pattern)?;
                Ok(Value::Str(
                    re.replace_all(haystack, replacement.as_str()).into_owned(),
                ))
            }
            (other, _, _) => Err(VmError::new(format!(
                "`regex_replace` exige (texto, padrao, substituicao) em strings, encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::RegexSplit => match (&args[0], &args[1]) {
            (Value::Str(haystack), Value::Str(pattern)) => {
                let re = compile_regex(pattern)?;
                Ok(Value::Array(
                    re.split(haystack)
                        .map(|s| Value::Str(s.to_string()))
                        .collect(),
                ))
            }
            (other, _) => Err(VmError::new(format!(
                "`regex_split` exige (texto, padrao) em strings, encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::Sha256 => match &args[0] {
            Value::Str(text) => {
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(text.as_bytes());
                Ok(Value::Str(hex_encode(&hasher.finalize())))
            }
            other => Err(VmError::new(format!(
                "`sha256` exige string, encontrada {}",
                type_name(other)
            ))),
        },
        Builtin::HmacSha256 => match (&args[0], &args[1]) {
            (Value::Str(key), Value::Str(message)) => {
                use hmac::{Hmac, KeyInit, Mac};
                use sha2::Sha256;
                type HmacSha256 = Hmac<Sha256>;
                let mut mac = HmacSha256::new_from_slice(key.as_bytes())
                    .map_err(|_| VmError::new("`hmac_sha256`: chave invalida".to_string()))?;
                mac.update(message.as_bytes());
                Ok(Value::Str(hex_encode(&mac.finalize().into_bytes())))
            }
            (other, _) => Err(VmError::new(format!(
                "`hmac_sha256` exige (chave, mensagem) em strings, encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::HmacVerify => match (&args[0], &args[1], &args[2]) {
            (Value::Str(key), Value::Str(message), Value::Str(tag)) => {
                use hmac::{Hmac, KeyInit, Mac};
                use sha2::Sha256;
                type HmacSha256 = Hmac<Sha256>;
                let mut mac = HmacSha256::new_from_slice(key.as_bytes())
                    .map_err(|_| VmError::new("`hmac_verify`: chave invalida".to_string()))?;
                mac.update(message.as_bytes());
                let expected = hex_decode(tag).ok_or_else(|| {
                    VmError::new("`hmac_verify`: tag nao e hexadecimal valido".to_string())
                })?;
                Ok(Value::Bool(mac.verify_slice(&expected).is_ok()))
            }
            (other, _, _) => Err(VmError::new(format!(
                "`hmac_verify` exige (chave, mensagem, tag) em strings, encontrado {}",
                type_name(other)
            ))),
        },
        Builtin::HashPassword => match &args[0] {
            Value::Str(password) => {
                use argon2::{Argon2, PasswordHasher};
                Argon2::default()
                    .hash_password(password.as_bytes())
                    .map(|hash| Value::Str(hash.to_string()))
                    .map_err(|e| VmError::new(format!("`hash_password` falhou: {e}")))
            }
            other => Err(VmError::new(format!(
                "`hash_password` exige senha (string), encontrada {}",
                type_name(other)
            ))),
        },
        Builtin::VerifyPassword => match (&args[0], &args[1]) {
            (Value::Str(password), Value::Str(hash)) => {
                if hash.starts_with("$argon2id$") {
                    use argon2::{Argon2, PasswordVerifier};
                    use password_hash::phc::PasswordHash;
                    let parsed = PasswordHash::new(hash)
                        .map_err(|_| VmError::new("hash argon2 invalido"))?;
                    check_argon2_bounds(&parsed)?;
                    Ok(Value::Bool(
                        Argon2::default()
                            .verify_password(password.as_bytes(), &parsed)
                            .is_ok(),
                    ))
                } else if hash.starts_with("$2a$")
                    || hash.starts_with("$2b$")
                    || hash.starts_with("$2x$")
                    || hash.starts_with("$2y$")
                {
                    bcrypt::verify(password, hash)
                        .map(Value::Bool)
                        .map_err(|_| VmError::new("hash bcrypt invalido"))
                } else {
                    Err(VmError::new(
                        "algoritmo nao suportado (use Argon2id ou bcrypt)".to_string(),
                    ))
                }
            }
            (other, _) => Err(VmError::new(format!(
                "`verify_password` exige (senha, hash) em strings, encontrado {}",
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
        Builtin::HttpFetch => {
            let url = match &args[0] {
                Value::Str(url) => url.clone(),
                other => {
                    return Err(VmError::new(format!(
                        "`http_fetch` exige string (url), encontrado {}",
                        type_name(other)
                    )));
                }
            };
            let opts = match &args[1] {
                Value::Map(opts) => parse_fetch_opts(opts)?,
                other => {
                    return Err(VmError::new(format!(
                        "`http_fetch` exige mapa de opcoes, encontrado {}",
                        type_name(other)
                    )));
                }
            };
            http_fetch_body(&url, &opts)
        }
    }
}

/// Ordena copia do array (estavel). Aceita arrays homogeneos — numeros
/// (`Int`/`Float` misturados comparam exato quando possivel), `Str`
/// (ordem lexicografica de bytes UTF-8) ou `Bool` (`false` < `true`) —
/// e o array vazio. Qualquer mistura com outros tipos e erro.
/// Passo do xorshift64* (RFC 0012): avanca o estado e devolve `u64`.
/// Rapido e std-only; NAO criptografico (documentado em `rand`).
fn next_rand(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    *state = x;
    x.wrapping_mul(0x2545_F491_4F6C_DD1D)
}

/// Compila padrao regex com teto anti-DoS (RFC 0014): programa compilado
/// limitado a 1 MiB. Erro inclui o motivo (ex.: lookaround nao suportado).
fn compile_regex(pattern: &str) -> Result<regex::Regex, VmError> {
    const MAX_REGEX_BYTES: usize = 1 << 20;
    regex::RegexBuilder::new(pattern)
        .size_limit(MAX_REGEX_BYTES)
        .build()
        .map_err(|e| VmError::new(format!("regex invalida: {e}")))
}

/// Codifica bytes em hexadecimal minusculo (sem crate extra).
fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xF) as usize] as char);
    }
    out
}

/// Decodifica hexadecimal (`None` se impar ou com digito invalido).
fn hex_decode(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    let raw = text.as_bytes();
    let mut out = Vec::with_capacity(raw.len() / 2);
    let mut i = 0;
    while i < raw.len() {
        let hi = (raw[i] as char).to_digit(16)?;
        let lo = (raw[i + 1] as char).to_digit(16)?;
        out.push((hi * 16 + lo) as u8);
        i += 2;
    }
    Some(out)
}

/// Tetos anti-DoS para verificacao Argon2 (RFC 0013): os parametros vem do
/// proprio hash (atacante controla); acima disto = erro antes de alocar.
/// Padroes OWASP (m=19MiB, t=2, p=1) passam folgados.
fn check_argon2_bounds(parsed: &password_hash::phc::PasswordHash) -> Result<(), VmError> {
    let m = parsed.params.get_decimal("m").unwrap_or(19456);
    let t = parsed.params.get_decimal("t").unwrap_or(2);
    let p = parsed.params.get_decimal("p").unwrap_or(1);
    if m > 65536 || t > 16 || p > 4 {
        return Err(VmError::new(
            "parametros Argon2 acima do teto (m<=65536KiB, t<=16, p<=4)".to_string(),
        ));
    }
    Ok(())
}

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

/// Opcoes de `http_fetch` (tudo opcional; padroes = comportamento util).
struct FetchOpts {
    /// Seguir redirects 301/302/303/307/308 (padrao: sim).
    follow: bool,
    /// Teto de redirects seguidos (0–10; padrao: 5).
    max_redirects: u32,
    /// Timeout de conexao e leitura por requisicao (padrao: 5 s).
    timeout: std::time::Duration,
    /// Cabecalhos extras (`Nome: valor`).
    headers: Vec<(String, String)>,
}

/// Teto de redirects aceito em `max_redirecionamentos`.
const MAX_REDIRECT_OPT: u32 = 10;

/// Timeout maximo aceito em `timeout` (segundos).
const MAX_TIMEOUT_SECS: f64 = 60.0;

fn parse_fetch_opts(opts: &std::collections::HashMap<String, Value>) -> Result<FetchOpts, VmError> {
    let mut follow = true;
    let mut max_redirects = 5u32;
    let mut timeout = std::time::Duration::from_secs(5);
    let mut headers = Vec::new();
    for (key, value) in opts {
        match key.as_str() {
            "seguir" => match value {
                Value::Bool(b) => follow = *b,
                other => {
                    return Err(VmError::new(format!(
                        "`http_fetch`: `seguir` exige bool, encontrado {}",
                        type_name(other)
                    )));
                }
            },
            "max_redirecionamentos" => match value {
                Value::Int(n) if (0..=i64::from(MAX_REDIRECT_OPT)).contains(n) => {
                    max_redirects = *n as u32;
                }
                other => {
                    return Err(VmError::new(format!(
                        "`http_fetch`: `max_redirecionamentos` exige 0–{MAX_REDIRECT_OPT}, encontrado {}",
                        value_desc(other)
                    )));
                }
            },
            "timeout" => {
                let secs = match value {
                    Value::Int(n) => *n as f64,
                    Value::Float(n) => *n,
                    other => {
                        return Err(VmError::new(format!(
                            "`http_fetch`: `timeout` exige segundos (numero), encontrado {}",
                            type_name(other)
                        )));
                    }
                };
                if !secs.is_finite() || secs <= 0.0 || secs > MAX_TIMEOUT_SECS {
                    return Err(VmError::new(format!(
                        "`http_fetch`: `timeout` exige 0–{MAX_TIMEOUT_SECS}s, encontrado {secs}"
                    )));
                }
                timeout = std::time::Duration::from_secs_f64(secs);
            }
            "headers" => match value {
                Value::Map(entries) => {
                    for (name, item) in entries {
                        let item = match item {
                            Value::Str(item) => item.clone(),
                            other => {
                                return Err(VmError::new(format!(
                                    "`http_fetch`: cabecalho `{name}` exige string, encontrado {}",
                                    type_name(other)
                                )));
                            }
                        };
                        check_header_field(name, &item)?;
                        headers.push((name.clone(), item));
                    }
                }
                other => {
                    return Err(VmError::new(format!(
                        "`http_fetch`: `headers` exige mapa, encontrado {}",
                        type_name(other)
                    )));
                }
            },
            "metodo" => match value {
                Value::Str(method) if method == "GET" => {}
                other => {
                    return Err(VmError::new(format!(
                        "`http_fetch`: `metodo` nao suportado (so GET), encontrado {}",
                        value_desc(other)
                    )));
                }
            },
            unknown => {
                return Err(VmError::new(format!(
                    "`http_fetch`: opcao desconhecida `{unknown}` (use `seguir`, `max_redirecionamentos`, `timeout`, `headers`)"
                )));
            }
        }
    }
    Ok(FetchOpts {
        follow,
        max_redirects,
        timeout,
        headers,
    })
}

/// Descricao curta de valor para erros de opcao (sem vazar conteudo longo).
fn value_desc(value: &Value) -> String {
    match value {
        Value::Str(s) if s.len() > 40 => format!("string ({} bytes)", s.len()),
        other => type_name(other).to_string(),
    }
}

/// Rejeita quebra de linha em nome/valor de cabecalho (injection).
fn check_header_field(name: &str, value: &str) -> Result<(), VmError> {
    if name.is_empty()
        || name.bytes().any(|b| b <= b' ' || b == b':' || b == 0x7f)
        || value.bytes().any(|b| b == b'\r' || b == b'\n')
    {
        return Err(VmError::new(format!(
            "`http_fetch`: cabecalho invalido `{name}`"
        )));
    }
    Ok(())
}

/// Resposta HTTP crua (status + cabecalhos em minusculas + corpo).
struct HttpResponse {
    status: u16,
    headers: std::collections::HashMap<String, String>,
    body: Vec<u8>,
}

/// Faz UMA requisicao GET (sem seguir redirects).
fn do_request(url: &str, opts: &FetchOpts) -> Result<HttpResponse, VmError> {
    use std::io::{Read, Write as _};
    use std::net::{TcpStream, ToSocketAddrs};

    let (tls, rest, default_port) = if let Some(rest) = url.strip_prefix("http://") {
        (false, rest, 80)
    } else if let Some(rest) = url.strip_prefix("https://") {
        (true, rest, 443)
    } else {
        return Err(VmError::new(
            "`http_fetch` suporta apenas urls `http://` ou `https://`".to_string(),
        ));
    };
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
        _ => (authority, default_port),
    };
    let addr = (host, port)
        .to_socket_addrs()
        .map_err(|e| VmError::new(format!("`http_get` falhou ao resolver {host}: {e}")))?
        .next()
        .ok_or_else(|| VmError::new(format!("`http_get` sem endereco para {host}")))?;
    let socket = TcpStream::connect_timeout(&addr, opts.timeout).map_err(|e| {
        VmError::new(format!(
            "`http_get` falhou ao conectar em {host}:{port}: {e}"
        ))
    })?;
    socket
        .set_read_timeout(Some(opts.timeout))
        .and_then(|()| socket.set_write_timeout(Some(opts.timeout)))
        .map_err(|e| VmError::new(format!("`http_get` falhou ao configurar timeout: {e}")))?;
    let mut stream = if tls {
        super::tls::connect_tls(socket, host).map_err(VmError::new)?
    } else {
        super::tls::Transport::Plain(socket)
    };
    let mut request = format!("GET {path} HTTP/1.0\r\nHost: {host}\r\n");
    for (name, item) in &opts.headers {
        request.push_str(&format!("{name}: {item}\r\n"));
    }
    request.push_str("Connection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .map_err(|e| VmError::new(format!("`http_get` falhou ao enviar requisicao: {e}")))?;
    // Le ate o fim dos cabecalhos; depois, corpo pelo Content-Length
    // quando presente (nao espera o close: necessario no TLS, onde o par
    // pode fechar sem `close_notify`). Sem Content-Length, le ate EOF
    // limpo (TLS sem `close_notify` = erro, documentado).
    let mut raw: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 8192];
    let header_end = loop {
        match stream.read(&mut chunk) {
            Ok(0) => break None,
            Ok(n) => {
                raw.extend_from_slice(&chunk[..n]);
                if raw.len() > MAX_HTTP_BODY + MAX_HEADERS {
                    return Err(VmError::new(format!(
                        "`http_get` excedeu o limite de corpo ({MAX_HTTP_BODY} bytes)"
                    )));
                }
                if let Some(pos) = raw
                    .windows(b"\r\n\r\n".len())
                    .position(|window| window == b"\r\n\r\n")
                {
                    break Some(pos);
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
    .ok_or_else(|| VmError::new("`http_get` com resposta sem cabecalhos".to_string()))?;
    let header = String::from_utf8_lossy(&raw[..header_end]);
    let mut lines = header.lines();
    let status_line = lines.next().unwrap_or("");
    let code = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| VmError::new(format!("`http_get` com status invalido: {status_line:?}")))?;
    let mut headers = std::collections::HashMap::new();
    for line in lines {
        if let Some((name, item)) = line.split_once(':') {
            let entry = headers
                .entry(name.trim().to_lowercase())
                .or_insert_with(String::new);
            if !entry.is_empty() {
                entry.push_str(", ");
            }
            entry.push_str(item.trim());
        }
    }
    // Corpo: Content-Length manda (para ao completar, sem esperar o
    // close); sem ele, le ate EOF limpo.
    let mut body = raw[header_end + 4..].to_vec();
    if let Some(expected) = headers
        .get("content-length")
        .and_then(|v| v.trim().parse::<usize>().ok())
    {
        if expected > MAX_HTTP_BODY {
            return Err(VmError::new(format!(
                "`http_get` excedeu o limite de corpo ({MAX_HTTP_BODY} bytes)"
            )));
        }
        while body.len() < expected {
            match stream.read(&mut chunk) {
                Ok(0) => {
                    return Err(VmError::new("`http_get` com resposta truncada".to_string()));
                }
                Ok(n) => {
                    body.extend_from_slice(&chunk[..n]);
                    if body.len() > MAX_HTTP_BODY {
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
        body.truncate(expected);
    } else {
        loop {
            match stream.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    body.extend_from_slice(&chunk[..n]);
                    if body.len() > MAX_HTTP_BODY {
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
    }
    Ok(HttpResponse {
        status: code,
        headers,
        body,
    })
}

/// Resolve `Location` contra a URL base (absoluta, raiz ou relativa).
/// Upgrades `http://` -> `https://` sao seguidos; downgrades `https://` ->
/// `http://` sao recusados; esquema preservado em relativos.
fn resolve_location(base: &str, location: &str) -> Result<String, VmError> {
    let location = location.trim();
    let base_https = base.starts_with("https://");
    let loc_https = location.starts_with("https://");
    let loc_http = location.starts_with("http://");
    if loc_http && base_https {
        return Err(VmError::new(
            "`http_fetch`: redirect de `https://` para `http://` recusado (downgrade)".to_string(),
        ));
    }
    if loc_http || loc_https {
        return Ok(location.to_string());
    }
    let scheme = if base_https { "https" } else { "http" };
    let after_scheme = base
        .strip_prefix("http://")
        .or_else(|| base.strip_prefix("https://"))
        .unwrap_or(base);
    let authority_end = after_scheme.find('/').unwrap_or(after_scheme.len());
    let authority = &after_scheme[..authority_end];
    if location.starts_with('/') {
        return Ok(format!("{scheme}://{authority}{location}"));
    }
    if location.is_empty() {
        return Err(VmError::new(
            "`http_fetch`: redirect sem `Location` utilizavel".to_string(),
        ));
    }
    let base_path = &after_scheme[authority_end..];
    let base_dir = match base_path.rfind('/') {
        Some(pos) => &base_path[..=pos],
        None => "/",
    };
    Ok(format!("{scheme}://{authority}{base_dir}{location}"))
}

/// `http_fetch(url, opts)`: GET com redirects/headers/timeout; devolve
/// mapa `{status, cabecalhos, corpo}` (status final, mesmo fora de 2xx).
fn http_fetch_body(url: &str, opts: &FetchOpts) -> Result<Value, VmError> {
    let mut current = url.to_string();
    let mut hops = 0u32;
    loop {
        let response = do_request(&current, opts)?;
        let redirect = matches!(response.status, 301 | 302 | 303 | 307 | 308)
            && response.headers.contains_key("location");
        if opts.follow && redirect {
            if hops >= opts.max_redirects {
                return Err(VmError::new(format!(
                    "`http_fetch`: redirecionamentos excedidos (limite {})",
                    opts.max_redirects
                )));
            }
            let location = response
                .headers
                .get("location")
                .expect("location existe")
                .clone();
            current = resolve_location(&current, &location)?;
            hops += 1;
            continue;
        }
        let body = String::from_utf8(response.body)
            .map_err(|e| VmError::new(format!("`http_fetch` com corpo nao UTF-8: {e}")))?;
        let mut entries = std::collections::HashMap::with_capacity(3);
        entries.insert("status".to_string(), Value::Int(i64::from(response.status)));
        entries.insert(
            "cabecalhos".to_string(),
            Value::Map(
                response
                    .headers
                    .into_iter()
                    .map(|(k, v)| (k, Value::Str(v)))
                    .collect(),
            ),
        );
        entries.insert("corpo".to_string(), Value::Str(body));
        return Ok(Value::Map(entries));
    }
}
/// Handle `db_*` (inteiro nao-negativo).
fn db_handle(value: &Value) -> Result<i64, VmError> {
    match value {
        Value::Int(handle) if *handle >= 0 => Ok(*handle),
        other => Err(VmError::new(format!(
            "`db`: handle invalido, encontrado {}",
            type_name(other)
        ))),
    }
}

/// SQL como string.
fn db_sql(value: &Value) -> Result<String, VmError> {
    match value {
        Value::Str(sql) => Ok(sql.clone()),
        other => Err(VmError::new(format!(
            "`db`: SQL exige string, encontrado {}",
            type_name(other)
        ))),
    }
}

/// Array de parametros RHP => valores SQL.
fn db_params(value: &Value) -> Result<Vec<super::sqlite::SqlValue>, VmError> {
    match value {
        Value::Array(items) => items
            .iter()
            .map(|item| match item {
                Value::Int(n) => Ok(super::sqlite::SqlValue::Int(*n)),
                Value::Float(n) => Ok(super::sqlite::SqlValue::Float(*n)),
                Value::Str(s) => Ok(super::sqlite::SqlValue::Text(s.clone())),
                Value::Bool(true) => Ok(super::sqlite::SqlValue::Int(1)),
                Value::Bool(false) => Ok(super::sqlite::SqlValue::Int(0)),
                Value::Null => Ok(super::sqlite::SqlValue::Null),
                other => Err(VmError::new(format!(
                    "`db`: parametro exige numero/string/bool/null, encontrado {}",
                    type_name(other)
                ))),
            })
            .collect(),
        other => Err(VmError::new(format!(
            "`db`: parametros exigem array, encontrado {}",
            type_name(other)
        ))),
    }
}

/// Conexao aberta pelo handle (fechado/inexistente = erro).
fn db_conn(db: &mut DbHandles, handle: i64) -> Result<&mut DbConn, VmError> {
    db.conns
        .get_mut(&handle)
        .ok_or_else(|| VmError::new(format!("`db`: handle {handle} invalido ou ja fechado")))
}

/// Interpola `{{chave}}` com escape HTML (Fase C3). Chave ausente,
/// `}}` sem par ou valor nao escalonavel = erro. Sem lacos/condicionais
/// por desenho (logica fica no RHP, apresentacao aqui).
fn render_template(
    template: &str,
    values: &std::collections::HashMap<String, Value>,
) -> Result<String, VmError> {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    loop {
        let Some(open) = rest.find("{{") else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..open]);
        let after = &rest[open + 2..];
        let Some(close) = after.find("}}") else {
            return Err(VmError::new(
                "`template`: `{{` sem `}}` de fechamento".to_string(),
            ));
        };
        let key = after[..close].trim();
        if key.is_empty() {
            return Err(VmError::new("`template`: chave vazia".to_string()));
        }
        let Some(value) = values.get(key) else {
            return Err(VmError::new(format!("`template`: chave ausente `{key}`")));
        };
        out.push_str(&escape_html(&value.to_string()));
        rest = &after[close + 2..];
    }
    Ok(out)
}

/// Escapa texto para HTML (`&` primeiro; aspas incluidas).
fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}
/// Converte params RHP para parametros PG (texto; `?` nao existe no PG,
/// use `$1`, `$2`... no SQL).
fn pg_params(params: &[super::sqlite::SqlValue]) -> Result<Vec<super::pg::PgParam>, VmError> {
    params
        .iter()
        .map(|p| match p {
            super::sqlite::SqlValue::Int(n) => Ok(super::pg::PgParam::Text(n.to_string())),
            super::sqlite::SqlValue::Float(n) => Ok(super::pg::PgParam::Text(n.to_string())),
            super::sqlite::SqlValue::Text(s) => Ok(super::pg::PgParam::Text(s.clone())),
            super::sqlite::SqlValue::Null => Ok(super::pg::PgParam::Null),
        })
        .collect()
}

/// Cliente HTTP minimo (somente `http://`), sem dependencias externas.
///
/// Timeouts de 5s para conexao e leitura; aceita apenas status 2xx e
/// corpo UTF-8 de ate [`MAX_HTTP_BODY`] bytes. A resolucao DNS usa o
/// resolvedor do sistema e nao tem timeout proprio (limitacao documentada).
/// Erros sao `VmError` com mensagem (sem panics). Comportamento identico
/// ao da Fase 6 (congelado; evolucao vai em `http_fetch`).
pub(super) fn http_get_body(url: &str) -> Result<String, VmError> {
    // Mensagem historica preservada byte a byte (Fase 6 congelada).
    if !url.starts_with("http://") {
        return Err(VmError::new(
            "`http_get` suporta apenas urls `http://`".to_string(),
        ));
    }
    let opts = FetchOpts {
        follow: false,
        max_redirects: 0,
        timeout: std::time::Duration::from_secs(5),
        headers: Vec::new(),
    };
    let response = do_request(url, &opts)?;
    if !(200..300).contains(&response.status) {
        return Err(VmError::new(format!(
            "`http_get` retornou status {}",
            response.status
        )));
    }
    String::from_utf8(response.body)
        .map_err(|e| VmError::new(format!("`http_get` com corpo nao UTF-8: {e}")))
}
