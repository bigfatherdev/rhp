//! PostgreSQL via protocolo proprio (mensagens v3) sobre `TcpStream` std
//! (ADR 0011, Fase B3). Testado contra servidor real (Docker).
//!
//! Escopo deliberado: startup + auth `trust`, extended query
//! (Parse/Bind/Describe/Execute/Sync, parametros em texto), parsing de
//! `RowDescription`/`DataRow`/`ErrorResponse`, `Terminate` ao fechar.
//! Fora do escopo (erro claro): outros metodos de auth (MD5/SASL =>
//! `trust` apenas), COPY, LISTEN, prepared statements cacheados,
//! transacoes implicitas (use SQL), tipos binarios.
//!
//! Sem `unsafe`, sem dependencias: bytes montados/lidos manualmente
//! (big-endian), UTF-8 validado.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// Timeout de conexao (s).
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// Timeout de leitura/escrita por operacao (s).
const IO_TIMEOUT: Duration = Duration::from_secs(30);

/// Conexao PostgreSQL (socket + parametros de sessao).
#[derive(Debug)]
pub struct Connection {
    stream: TcpStream,
    #[allow(dead_code)]
    params: HashMap<String, String>,
}

/// Resposta de consulta: linhas como mapas coluna => valor RHP-like.
#[derive(Debug, Clone)]
pub enum PgValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    Text(String),
    Null,
}

impl Connection {
    /// Conecta com `conninfo` (`host=.. port=.. user=.. dbname=..`;
    /// padroes: 127.0.0.1/5432/`postgres`/=user). Auth `trust` apenas.
    pub fn connect(conninfo: &str) -> Result<Self, String> {
        let mut params: HashMap<String, String> = HashMap::new();
        for part in conninfo.split_whitespace() {
            match part.split_once('=') {
                Some((k, v)) if !k.is_empty() => {
                    params.insert(k.to_string(), v.to_string());
                }
                _ => {
                    return Err(format!(
                        "`db`: conninfo invalida `{part}` (use `host=.. port=.. user=.. dbname=..`)"
                    ));
                }
            }
        }
        let host = params
            .get("host")
            .cloned()
            .unwrap_or("127.0.0.1".to_string());
        let port: u16 = params
            .get("port")
            .map(|p| {
                p.parse::<u16>()
                    .map_err(|_| format!("`db`: porta invalida `{p}`"))
            })
            .transpose()?
            .unwrap_or(5432);
        let user = params
            .get("user")
            .cloned()
            .unwrap_or_else(|| "postgres".to_string());
        let dbname = params
            .get("dbname")
            .cloned()
            .unwrap_or_else(|| user.clone());
        let addr = (host.as_str(), port)
            .to_socket_addrs()
            .map_err(|e| format!("`db`: resolver {host}:{port}: {e}"))?
            .next()
            .ok_or_else(|| format!("`db`: sem endereco para {host}:{port}"))?;
        let stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT)
            .map_err(|e| format!("`db`: conectar em {host}:{port}: {e}"))?;
        stream
            .set_read_timeout(Some(IO_TIMEOUT))
            .and_then(|()| stream.set_write_timeout(Some(IO_TIMEOUT)))
            .map_err(|e| format!("`db`: configurar timeout: {e}"))?;
        let mut conn = Self { stream, params };
        conn.startup(&user, &dbname)?;
        Ok(conn)
    }

    fn send(&mut self, kind: u8, payload: &[u8]) -> Result<(), String> {
        let len = (payload.len() + 4) as u32;
        let mut message = Vec::with_capacity(1 + payload.len() + 4);
        message.push(kind);
        message.extend_from_slice(&len.to_be_bytes());
        message.extend_from_slice(payload);
        self.stream
            .write_all(&message)
            .map_err(|e| format!("`db`: enviar mensagem: {e}"))?;
        Ok(())
    }

    fn recv(&mut self) -> Result<(u8, Vec<u8>), String> {
        let mut header = [0u8; 5];
        self.stream
            .read_exact(&mut header)
            .map_err(|e| format!("`db`: ler resposta: {e}"))?;
        let len = u32::from_be_bytes([header[1], header[2], header[3], header[4]]) as usize;
        if !(4..=64 * 1024 * 1024).contains(&len) {
            return Err("`db`: resposta com tamanho invalido".to_string());
        }
        let mut payload = vec![0u8; len - 4];
        self.stream
            .read_exact(&mut payload)
            .map_err(|e| format!("`db`: ler resposta: {e}"))?;
        Ok((header[0], payload))
    }

    fn startup(&mut self, user: &str, dbname: &str) -> Result<(), String> {
        let mut payload = 196_608i32.to_be_bytes().to_vec();
        for (k, v) in [("user", user), ("database", dbname)] {
            payload.extend_from_slice(k.as_bytes());
            payload.push(0);
            payload.extend_from_slice(v.as_bytes());
            payload.push(0);
        }
        payload.push(0);
        // Startup nao tem byte de tipo: Int32(tamanho) + payload.
        let len = (payload.len() + 4) as u32;
        let mut message = len.to_be_bytes().to_vec();
        message.extend_from_slice(&payload);
        self.stream
            .write_all(&message)
            .map_err(|e| format!("`db`: enviar startup: {e}"))?;
        loop {
            let (kind, payload) = self.recv()?;
            match kind {
                b'R' => {
                    let method = read_i32(&payload, 0)?;
                    match method {
                        0 => {}
                        2 | 5 => {
                            return Err(
                                "`db`: auth Kerberos/MD5 nao suportado (use `trust`)".to_string()
                            );
                        }
                        10 => {
                            return Err(
                                "`db`: auth SASL/SCRAM nao suportado (use `trust`)".to_string()
                            );
                        }
                        _ => {
                            return Err(format!(
                                "`db`: metodo de auth {method} nao suportado (use `trust`)"
                            ));
                        }
                    }
                }
                b'E' => return Err(pg_error(&payload)),
                b'N' | b'S' | b'K' => {}
                b'Z' => return Ok(()),
                other => {
                    return Err(format!("`db`: mensagem inesperada `{other}` no startup"));
                }
            }
        }
    }

    /// Executa (uma instrucao); devolve linhas afetadas.
    pub fn exec(&mut self, sql: &str, params: &[PgParam]) -> Result<i64, String> {
        let (rows, _) = self.extended(sql, params)?;
        Ok(rows)
    }

    /// Consulta; devolve (linhas afetadas, linhas).
    pub fn query(
        &mut self,
        sql: &str,
        params: &[PgParam],
    ) -> Result<Vec<HashMap<String, PgValue>>, String> {
        let (_, rows) = self.extended(sql, params)?;
        Ok(rows)
    }

    fn extended(
        &mut self,
        sql: &str,
        params: &[PgParam],
    ) -> Result<(i64, Vec<HashMap<String, PgValue>>), String> {
        if sql.as_bytes().contains(&0) {
            return Err("`db`: SQL com NUL embutido".to_string());
        }
        // Parse: stmt "" + query + 0 tipos (inferencia do servidor).
        let mut parse = Vec::new();
        parse.push(0u8);
        parse.extend_from_slice(sql.as_bytes());
        parse.push(0);
        parse.extend_from_slice(&0u16.to_be_bytes());
        self.send(b'P', &parse)?;
        // Bind: portal "" + stmt "" + 0 formatos + n params + 0 formatos out.
        let mut bind = Vec::new();
        bind.push(0u8);
        bind.push(0u8);
        bind.extend_from_slice(&0u16.to_be_bytes());
        bind.extend_from_slice(&(params.len() as u16).to_be_bytes());
        for param in params {
            match param {
                PgParam::Null => bind.extend_from_slice(&(-1i32).to_be_bytes()),
                PgParam::Text(text) => {
                    bind.extend_from_slice(&(text.len() as u32).to_be_bytes());
                    bind.extend_from_slice(text.as_bytes());
                }
            }
        }
        bind.extend_from_slice(&0u16.to_be_bytes());
        self.send(b'B', &bind)?;
        // Describe (statement) + Execute (tudo) + Sync.
        self.send(b'D', &[b'S', 0u8])?;
        let mut execute = Vec::new();
        execute.push(0u8);
        execute.extend_from_slice(&0u32.to_be_bytes());
        self.send(b'E', &execute)?;
        self.send(b'S', &[])?;
        // Leitura: colunas, linhas, tag, ate ReadyForQuery.
        // Erros NAO retornam no meio: registram em `failure` e drenam ate
        // o `Z` (RHP-AUD-IND-002). Retornar aqui deixaria o `Z` no socket:
        // o proximo comando leria o `Z` alheio como conclusao propria
        // (sucesso falso) e o erro apareceria um comando atrasado (ou nunca).
        let mut columns: Vec<(String, u32)> = Vec::new();
        let mut rows: Vec<HashMap<String, PgValue>> = Vec::new();
        let mut affected: i64 = 0;
        let mut failure: Option<String> = None;
        loop {
            // Falha de I/O: conexao perdida; sem dreno possivel.
            let (kind, payload) = self.recv()?;
            match kind {
                // ParseComplete/BindComplete/NoData: reconhecidos, sem efeito.
                // (`C` e CommandComplete aqui; CloseComplete nao e enviado.)
                b'1' | b'2' | b'n' => {}
                b'T' => {
                    if let Err(e) = parse_row_description(&payload).map(|cols| columns = cols) {
                        failure.get_or_insert(e);
                    }
                }
                b'D' => match parse_data_row(&payload, &columns) {
                    Ok(row) => rows.push(row),
                    Err(e) => {
                        failure.get_or_insert(e);
                    }
                },
                b'C' => {
                    affected = parse_command_tag(&payload);
                }
                b'E' => {
                    failure.get_or_insert_with(|| pg_error(&payload));
                }
                b'N' => {}
                b'Z' => break,
                b'I' => {}
                _ => {}
            }
            if failure.is_some() {
                // Drena o resto ate ReadyForQuery (teto contra servidor
                // patologico; I/O continua falhando alto).
                for _ in 0..1_000_000 {
                    match self.recv() {
                        Ok((b'Z', _)) => break,
                        Ok(_) => continue,
                        Err(e) => return Err(e),
                    }
                }
                break;
            }
        }
        match failure {
            Some(e) => Err(e),
            None => Ok((affected, rows)),
        }
    }

    /// Encerra com `Terminate` (o `Drop` fecha o socket de qualquer forma).
    pub fn close(mut self) -> Result<(), String> {
        let _ = self.send(b'X', &[]);
        Ok(())
    }
}

/// Parametro de consulta (texto ou nulo).
#[derive(Debug, Clone)]
pub enum PgParam {
    Null,
    Text(String),
}

fn read_i32(payload: &[u8], at: usize) -> Result<i32, String> {
    payload
        .get(at..at + 4)
        .and_then(|w| w.try_into().ok())
        .map(i32::from_be_bytes)
        .ok_or_else(|| "`db`: resposta truncada".to_string())
}

/// Erro do servidor (`ErrorResponse`: campos codigo+`M` mensagem).
fn pg_error(payload: &[u8]) -> String {
    let mut code = String::new();
    let mut message = String::from("erro do servidor");
    let mut rest = payload;
    while rest.len() >= 2 && rest[0] != 0 {
        let field = rest[0];
        let end = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
        let text = String::from_utf8_lossy(&rest[1..end]).into_owned();
        if field == b'C' {
            code = text;
        } else if field == b'M' {
            message = text;
        }
        rest = if end < rest.len() {
            &rest[end + 1..]
        } else {
            &[]
        };
    }
    if code.is_empty() {
        format!("`db`: {message}")
    } else {
        format!("`db` [{code}]: {message}")
    }
}

/// `RowDescription` => [(nome, oid do tipo)].
fn parse_row_description(payload: &[u8]) -> Result<Vec<(String, u32)>, String> {
    if payload.len() < 2 {
        return Err("`db`: RowDescription truncada".to_string());
    }
    let count = u16::from_be_bytes([payload[0], payload[1]]) as usize;
    let mut rest = &payload[2..];
    let mut columns = Vec::with_capacity(count);
    for _ in 0..count {
        let end = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| "`db`: RowDescription truncada".to_string())?;
        let name = std::str::from_utf8(&rest[..end])
            .map_err(|_| "`db`: nome de coluna nao UTF-8".to_string())?
            .to_string();
        rest = &rest[end + 1..];
        if rest.len() < 18 {
            return Err("`db`: RowDescription truncada".to_string());
        }
        let oid = u32::from_be_bytes([rest[6], rest[7], rest[8], rest[9]]);
        rest = &rest[18..];
        columns.push((name, oid));
    }
    Ok(columns)
}

/// `DataRow` => mapa coluna => valor (texto; nulo = -1).
fn parse_data_row(
    payload: &[u8],
    columns: &[(String, u32)],
) -> Result<HashMap<String, PgValue>, String> {
    if payload.len() < 2 {
        return Err("`db`: DataRow truncada".to_string());
    }
    let count = u16::from_be_bytes([payload[0], payload[1]]) as usize;
    if count != columns.len() {
        return Err("`db`: DataRow com colunas divergentes".to_string());
    }
    let mut rest = &payload[2..];
    let mut row = HashMap::with_capacity(count);
    for (name, oid) in columns {
        if rest.len() < 4 {
            return Err("`db`: DataRow truncada".to_string());
        }
        let len = i32::from_be_bytes([rest[0], rest[1], rest[2], rest[3]]);
        rest = &rest[4..];
        if len == -1 {
            row.insert(name.clone(), PgValue::Null);
            continue;
        }
        let len = len as usize;
        if rest.len() < len {
            return Err("`db`: DataRow truncada".to_string());
        }
        let text =
            std::str::from_utf8(&rest[..len]).map_err(|_| "`db`: valor nao UTF-8".to_string())?;
        row.insert(name.clone(), convert_text(*oid, text)?);
        rest = &rest[len..];
    }
    Ok(row)
}

/// Converte texto pelo OID do tipo (texto: resto vira `Str`).
fn convert_text(oid: u32, text: &str) -> Result<PgValue, String> {
    match oid {
        // int2/int4/int8
        21 | 23 | 20 => text
            .parse::<i64>()
            .map(PgValue::Int)
            .map_err(|_| format!("`db`: inteiro invalido `{text}`")),
        // float4/float8
        700 | 701 => text
            .parse::<f64>()
            .map(|v| {
                if v.is_finite() {
                    PgValue::Float(v)
                } else {
                    PgValue::Text(text.to_string())
                }
            })
            .map_err(|_| format!("`db`: numero invalido `{text}`")),
        // bool
        16 => match text {
            "t" => Ok(PgValue::Bool(true)),
            "f" => Ok(PgValue::Bool(false)),
            _ => Err(format!("`db`: booleano invalido `{text}`")),
        },
        // numeric: inteiro => Int, resto => Float, resto => Str
        1700 => {
            if let Ok(n) = text.parse::<i64>() {
                Ok(PgValue::Int(n))
            } else if let Ok(n) = text.parse::<f64>() {
                Ok(PgValue::Float(n))
            } else {
                Ok(PgValue::Text(text.to_string()))
            }
        }
        _ => Ok(PgValue::Text(text.to_string())),
    }
}

/// Tag de `CommandComplete` => linhas afetadas.
fn parse_command_tag(payload: &[u8]) -> i64 {
    let tag = payload
        .split(|&b| b == 0)
        .next()
        .map(String::from_utf8_lossy)
        .unwrap_or_default()
        .into_owned();
    tag.split_whitespace()
        .last()
        .and_then(|n| n.parse::<i64>().ok())
        .unwrap_or(0)
}
