//! TLS cliente para `https://` (ADR 0012): `rustls` puro Rust.
//!
//! Raizes: `webpki-roots` (Mozilla embutidas, deterministicas) +
//! `RHP_CA_BUNDLE` (PEM extra do usuario, ex. proxies corporativos).
//! Sem `unsafe`, sem dependencias de SO alem de `cc` para o `ring`.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;

/// Transporte: TCP puro ou TLS sobre TCP (caixa no TLS: `ClientConnection`
/// e grande; sem ela o enum estouraria a pilha por valor).
pub enum Transport {
    Plain(TcpStream),
    Tls(Box<rustls::StreamOwned<rustls::ClientConnection, TcpStream>>),
}

impl Read for Transport {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Transport::Plain(stream) => stream.read(buf),
            Transport::Tls(stream) => stream.read(buf),
        }
    }
}

impl Write for Transport {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Transport::Plain(stream) => stream.write(buf),
            Transport::Tls(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Transport::Plain(stream) => stream.flush(),
            Transport::Tls(stream) => stream.flush(),
        }
    }
}

/// Configuracao TLS (raizes embutidas + `RHP_CA_BUNDLE` se definido).
fn client_config() -> Result<Arc<rustls::ClientConfig>, String> {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    if let Ok(path) = std::env::var("RHP_CA_BUNDLE") {
        let file =
            std::fs::File::open(&path).map_err(|e| format!("RHP_CA_BUNDLE `{path}`: {e}"))?;
        let mut reader = std::io::BufReader::new(file);
        let mut added = 0u32;
        for cert in rustls_pemfile::certs(&mut reader) {
            let cert = cert.map_err(|e| format!("RHP_CA_BUNDLE invalido: {e}"))?;
            roots
                .add(cert)
                .map_err(|e| format!("RHP_CA_BUNDLE rejeitado: {e:?}"))?;
            added += 1;
        }
        if added == 0 {
            return Err(format!("RHP_CA_BUNDLE `{path}` sem certificados"));
        }
    }
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(Arc::new(config))
}

/// Nome para SNI/verificacao: IP literal ou DNS (ASCII).
fn server_name(host: &str) -> Result<rustls::pki_types::ServerName<'static>, String> {
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return Ok(rustls::pki_types::ServerName::IpAddress(ip.into()));
    }
    rustls::pki_types::ServerName::try_from(host.to_string())
        .map(|name| name.to_owned())
        .map_err(|_| format!("`https`: hostname invalido `{host}`"))
}

/// Envolve o socket conectado em TLS com SNI = host.
pub fn connect_tls(stream: TcpStream, host: &str) -> Result<Transport, String> {
    let config = client_config()?;
    let name = server_name(host)?;
    let connection = rustls::ClientConnection::new(config, name)
        .map_err(|e| format!("`https`: iniciar TLS: {e}"))?;
    Ok(Transport::Tls(Box::new(rustls::StreamOwned::new(
        connection, stream,
    ))))
}
