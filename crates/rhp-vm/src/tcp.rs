//! Conexoes TCP com timeout (Etapa 5, RFC 0020): base do `tcp_*`,
//! do cliente Redis (RESP2) e do cliente MySQL. Puro `std::net`
//! (bloqueante com timeout; paralelismo via `async_spawn`).

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// Timeout de conexao/leitura/escrita (igual ao HTTP da Fase A5).
const TIMEOUT: Duration = Duration::from_secs(5);
/// Teto de uma leitura avulsa (`tcp_recv` com `max` maior e cortado).
pub const MAX_READ: usize = 8 * 1024 * 1024;

/// Conexao TCP com timeouts configurados.
#[derive(Debug)]
pub struct TcpConn {
    stream: TcpStream,
}

impl TcpConn {
    /// Conecta em `host:porta` (DNS do sistema; primeira resolucao valida).
    pub fn connect(host: &str, port: u16) -> Result<Self, String> {
        let mut last_err = "sem endereco".to_string();
        for addr in (host, port)
            .to_socket_addrs()
            .map_err(|e| format!("`tcp`: DNS `{host}` falhou: {e}"))?
        {
            match TcpStream::connect_timeout(&addr, TIMEOUT) {
                Ok(stream) => {
                    stream
                        .set_read_timeout(Some(TIMEOUT))
                        .and(stream.set_write_timeout(Some(TIMEOUT)))
                        .map_err(|e| format!("`tcp`: timeout falhou: {e}"))?;
                    return Ok(Self { stream });
                }
                Err(e) => {
                    last_err = format!("`tcp`: conectar em {addr} falhou: {e}");
                }
            }
        }
        Err(last_err)
    }

    /// Escreve todos os bytes; devolve a quantidade.
    pub fn write_all(&mut self, data: &[u8]) -> Result<usize, String> {
        self.stream
            .write_all(data)
            .map_err(|e| format!("`tcp`: escrita falhou: {e}"))?;
        Ok(data.len())
    }

    /// Le ate `max.len()` bytes (bloqueia ate chegar ao menos 1 byte ou
    /// EOF). `Ok(None)` = EOF limpo.
    pub fn read_into(&mut self, max: usize, buf: &mut Vec<u8>) -> Result<Option<usize>, String> {
        let capped = max.min(MAX_READ);
        buf.resize(capped, 0);
        match self.stream.read(buf) {
            Ok(0) => {
                buf.clear();
                Ok(None)
            }
            Ok(n) => {
                buf.truncate(n);
                Ok(Some(n))
            }
            Err(e) => Err(format!("`tcp`: leitura falhou: {e}")),
        }
    }

    /// Le exatamente `n` bytes (ou erro/EOF prematuro).
    pub fn read_exact(&mut self, n: usize, buf: &mut Vec<u8>) -> Result<(), String> {
        buf.resize(n, 0);
        self.stream
            .read_exact(buf)
            .map_err(|e| format!("`tcp`: leitura exata falhou ({n} bytes): {e}"))
    }

    /// Le ate `\n` (inclusive); devolve a linha crua.
    pub fn read_line(&mut self, out: &mut Vec<u8>) -> Result<(), String> {
        let mut byte = [0u8; 1];
        loop {
            match self.stream.read(&mut byte) {
                Ok(0) => return Err("`tcp`: EOF no meio da linha".to_string()),
                Ok(_) => {
                    out.push(byte[0]);
                    if byte[0] == b'\n' {
                        return Ok(());
                    }
                }
                Err(e) => return Err(format!("`tcp`: leitura falhou: {e}")),
            }
        }
    }

    /// Desliga (shutdown nos dois sentidos; erro ignorado no drop).
    pub fn close(self) {
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }
}

/// Handles de TCP cru (`tcp_*`; mesmo padrao de `DbHandles`).
#[derive(Debug, Default)]
pub struct TcpHandles {
    pub conns: HashMap<i64, TcpConn>,
    pub next: i64,
}
