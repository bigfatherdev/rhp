//! E2E dos conectores TCP/Redis (Etapa 5, RFC 0020): servidores
//! in-process (deterministicos, sem rede externa) + programa RHP real.
//!
//! O MySQL tem suite propria em `mysql.rs` (`#[cfg(test)]`, precisa de
//! `native_password` interno para validar a autenticacao de verdade).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

fn run_src(source: &str) -> Result<String, rhp_vm::VmError> {
    let program = rhp_parser::parse_source(source).expect("parse deve passar");
    let chunk = rhp_bytecode::compile(&program).expect("compile deve passar");
    rhp_vm::Vm::new().run_to_string(&chunk)
}

fn run_err(source: &str) -> String {
    run_src(source).expect_err("deve falhar").message
}

/// Ecoa tudo que chegar ate EOF (uma conexao; depois encerra).
fn echo_server_once(listener: TcpListener) {
    let (mut stream, _) = listener.accept().expect("accept");
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .expect("timeout");
    let mut buf = vec![0u8; 4096];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => stream.write_all(&buf[..n]).expect("eco"),
            Err(e) => panic!("leitura do echo: {e}"),
        }
    }
}

#[test]
fn tcp_echo_roundtrip() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("porta").port();
    std::thread::spawn(move || echo_server_once(listener));
    let src = format!(
        "let h = tcp_connect(\"127.0.0.1\", {port});\necho tcp_send(h, \"ola\");\necho tcp_recv(h, 100);\necho tcp_close(h);\n"
    );
    assert_eq!(run_src(&src).expect("eco tcp"), "3\nola\nnull\n");
    // Handle invalido e erro claro; string vazia tambem transmite.
    assert!(run_err("echo tcp_recv(999, 10);").contains("invalido"));
    assert!(run_err("echo tcp_close(999);").contains("invalido"));
}

/// Mini-Redis scriptado: PING/SET/GET/INCR/DEL/QUIT + erros.
fn redis_mock(listener: TcpListener) {
    let (mut stream, _) = listener.accept().expect("accept");
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .expect("timeout");
    let mut store: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut tmp = [0u8; 1];
    let mut read_line = |stream: &mut TcpStream, buf: &mut Vec<u8>| -> Option<Vec<u8>> {
        buf.clear();
        loop {
            match stream.read(&mut tmp) {
                Ok(0) => return None,
                Ok(_) => {
                    buf.push(tmp[0]);
                    if tmp[0] == b'\n' {
                        return Some(std::mem::take(buf));
                    }
                }
                Err(e) => panic!("leitura redis mock: {e}"),
            }
        }
    };
    let mut line = Vec::new();
    // Saudacao: consome o PING inicial do `redis_open`.
    loop {
        let raw = read_line(&mut stream, &mut line).expect("PING inicial");
        let text = String::from_utf8_lossy(&raw);
        if text.starts_with('*') {
            break;
        }
    }
    // Le o resto do array do PING (1 bulk).
    let _ = read_line(&mut stream, &mut line).expect("bulk PING");
    let _ = read_line(&mut stream, &mut line).expect("texto PING");
    stream.write_all(b"+PONG\r\n").expect("PONG");
    while let Some(header) = read_line(&mut stream, &mut line) {
        // Cabecalho do array.
        let header = String::from_utf8_lossy(&header).into_owned();
        if !header.starts_with('*') {
            break;
        }
        let argc: usize = header[1..].trim().parse().expect("argc");
        let mut args = Vec::new();
        for _ in 0..argc {
            let h = read_line(&mut stream, &mut line).expect("bulk header");
            let h = String::from_utf8_lossy(&h).into_owned();
            assert!(h.starts_with('$'), "esperado bulk, obteve: {h}");
            let len: usize = h[1..].trim().parse().expect("bulk len");
            let mut data = vec![0u8; len + 2];
            stream.read_exact(&mut data).expect("bulk dados");
            args.push(String::from_utf8_lossy(&data[..len]).into_owned());
        }
        if args.is_empty() {
            break;
        }
        let cmd = args[0].to_uppercase();
        let reply: Vec<u8> = match cmd.as_str() {
            "PING" => b"+PONG\r\n".to_vec(),
            "QUIT" => {
                stream.write_all(b"+OK\r\n").expect("QUIT");
                break;
            }
            "SET" if args.len() == 3 => {
                store.insert(args[1].clone(), args[2].clone());
                b"+OK\r\n".to_vec()
            }
            "GET" if args.len() == 2 => match store.get(&args[1]) {
                Some(v) => format!("${}\r\n{v}\r\n", v.len()).into_bytes(),
                None => b"$-1\r\n".to_vec(),
            },
            "INCR" if args.len() == 2 => {
                let next: i64 = store
                    .get(&args[1])
                    .map(|v| v.parse().unwrap_or(0))
                    .unwrap_or(0)
                    + 1;
                store.insert(args[1].clone(), next.to_string());
                format!(":{next}\r\n").into_bytes()
            }
            "DEL" if args.len() == 2 => {
                let removed = store.remove(&args[1]).is_some() as i64;
                format!(":{removed}\r\n").into_bytes()
            }
            _ => format!("-ERR comando desconhecido '{}'\r\n", args[0]).into_bytes(),
        };
        stream.write_all(&reply).expect("resposta");
    }
}

fn redis_port() -> (u16, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("porta").port();
    let handle = std::thread::spawn(move || redis_mock(listener));
    (port, handle)
}

#[test]
fn redis_exec_roundtrip() {
    let (port, server) = redis_port();
    let src = format!(
        concat!(
            "let r = redis_open(\"127.0.0.1\", {port});\n",
            "echo redis_exec(r, [\"SET\", \"nome\", \"rhp\"]);\n",
            "echo redis_exec(r, [\"GET\", \"nome\"]);\n",
            "echo redis_exec(r, [\"GET\", \"ausente\"]);\n",
            "echo redis_exec(r, [\"INCR\", \"n\"]);\n",
            "echo redis_exec(r, [\"INCR\", \"n\"]);\n",
            "echo redis_exec(r, [\"DEL\", \"nome\"]);\n",
            "echo redis_exec(r, [\"GET\", \"nome\"]);\n",
            "echo redis_close(r);\n",
        ),
        port = port
    );
    assert_eq!(
        run_src(&src).expect("redis mock"),
        "OK\nrhp\nnull\n1\n2\n1\nnull\nnull\n"
    );
    server.join().expect("mock encerrou");
    // Erro do servidor vira erro RHP; handle invalido tambem.
    let (port2, server2) = redis_port();
    let bad =
        format!("let r = redis_open(\"127.0.0.1\", {port2});\necho redis_exec(r, [\"FOO\"]);\n");
    assert!(run_err(&bad).contains("desconhecido"));
    drop(server2);
}
