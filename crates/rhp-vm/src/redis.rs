//! Cliente Redis (RESP2) sobre TCP (Etapa 5, RFC 0020).
//!
//! `redis_open(host, port)` (sem AUTH v1) + `redis_exec(h, [cmd, args])`
//! + `redis_close(h)`. Tipos RESP:
//!
//! - simples/bulk => `Str`; inteiro => `Int`; nulo => `null`;
//!   array => `Array` (recursivo, teto de 128 niveis).
//! - Erro RESP (`-...`) vira `VmError`. Sem dependencias.

use super::tcp::TcpConn;
use super::type_name;
use rhp_bytecode::Value;
use std::collections::HashMap;

/// Profundidade maxima de arrays RESP aninhados.
const MAX_DEPTH: usize = 128;

/// Conexao Redis (socket proprio; fecha no `drop` via `TcpConn`... nao:
// -- `close` explicito; veja `RedisHandles`).
#[derive(Debug)]
pub struct RedisConn {
    conn: TcpConn,
}

/// Handles de Redis (`redis_*`; mesmo padrao de `DbHandles`).
#[derive(Debug, Default)]
pub struct RedisHandles {
    pub conns: HashMap<i64, RedisConn>,
    pub next: i64,
}

fn resp_encode(parts: &[String], out: &mut Vec<u8>) {
    out.extend_from_slice(format!("*{}\r\n", parts.len()).as_bytes());
    for part in parts {
        out.extend_from_slice(format!("${}\r\n", part.len()).as_bytes());
        out.extend_from_slice(part.as_bytes());
        out.extend_from_slice(b"\r\n");
    }
}

fn resp_line(line: &[u8]) -> Result<&[u8], String> {
    line.strip_suffix(b"\r\n")
        .ok_or_else(|| "RESP sem CRLF".to_string())
}

fn resp_int(line: &[u8]) -> Result<i64, String> {
    std::str::from_utf8(resp_line(line)?)
        .map_err(|_| "RESP inteiro invalido".to_string())?
        .parse::<i64>()
        .map_err(|_| "RESP inteiro invalido".to_string())
}

/// Decodifica uma resposta RESP2 completa (chamadas recursivas limitadas).
fn resp_decode(conn: &mut TcpConn, depth: usize) -> Result<Option<Value>, String> {
    if depth > MAX_DEPTH {
        return Err("RESP aninhado alem de 128 niveis".to_string());
    }
    let mut line = Vec::new();
    conn.read_line(&mut line)?;
    let (kind, rest) = line
        .split_first()
        .ok_or_else(|| "resposta RESP vazia".to_string())?;
    match kind {
        b'+' => Ok(Some(Value::Str(
            String::from_utf8_lossy(resp_line(rest)?).into_owned(),
        ))),
        b'-' => Err(String::from_utf8_lossy(resp_line(rest)?).into_owned()),
        b':' => Ok(Some(Value::Int(resp_int(rest)?))),
        b'$' => {
            let len = resp_int(rest)?;
            if len < 0 {
                return Ok(None);
            }
            if len as usize > super::tcp::MAX_READ {
                return Err("bulk maior que 8 MiB".to_string());
            }
            let mut buf = Vec::new();
            conn.read_exact(len as usize, &mut buf)?;
            let mut crlf = Vec::new();
            conn.read_exact(2, &mut crlf)?;
            if crlf != *b"\r\n" {
                return Err("bulk sem CRLF final".to_string());
            }
            String::from_utf8(buf)
                .map(Value::Str)
                .map(Some)
                .map_err(|_| "bulk nao-UTF-8 (fora do escopo v1)".to_string())
        }
        b'*' => {
            let len = resp_int(rest)?;
            if len < 0 {
                return Ok(None);
            }
            let mut items = Vec::with_capacity(len as usize);
            for _ in 0..len {
                match resp_decode(conn, depth + 1)? {
                    Some(value) => items.push(value),
                    None => items.push(Value::Null),
                }
            }
            Ok(Some(Value::Array(items)))
        }
        _ => Err("tipo RESP desconhecido".to_string()),
    }
}

impl RedisConn {
    /// Conecta; se o servidor exigir AUTH (`-NOAUTH`), falha claro
    /// (sem AUTH na v1; RFC 0020).
    pub fn open(host: &str, port: u16) -> Result<Self, String> {
        let mut conn = TcpConn::connect(host, port)?;
        // Ping de saudacao: detecta AUTH obrigatorio e valida o protocolo.
        let mut hello = Vec::new();
        resp_encode(&["PING".to_string()], &mut hello);
        conn.write_all(&hello)?;
        match resp_decode(&mut conn, 0) {
            Ok(_) => Ok(Self { conn }),
            Err(message) => Err(format!("`redis_open`: {message}")),
        }
    }

    /// Executa `cmd` com `args` (todos `Str`; resto = erro claro).
    pub fn exec(&mut self, cmd: &str, args: &[Value]) -> Result<Value, String> {
        let mut parts = vec![cmd.to_string()];
        for arg in args {
            match arg {
                Value::Str(s) => parts.push(s.clone()),
                other => {
                    return Err(format!(
                        "`redis_exec`: argumento deve ser string, encontrado {}",
                        type_name(other)
                    ));
                }
            }
        }
        let mut out = Vec::new();
        resp_encode(&parts, &mut out);
        self.conn.write_all(&out)?;
        // Nulo de topo = `Null` (ex.: `GET` de chave ausente).
        Ok(resp_decode(&mut self.conn, 0)?.unwrap_or(Value::Null))
    }
}
