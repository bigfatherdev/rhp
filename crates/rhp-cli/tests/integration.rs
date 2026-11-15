//! Testes de integracao do RHP: `rhp run` fim a fim.
//!
//! Fase 1: `hello.rhp` imprime exatamente `Hello from RHP!\n`.
//! Fase 2 (Marco 2): expressoes, variaveis/controle e funcoes.

use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(|p| p.parent())
        .map_or_else(|| PathBuf::from("."), std::path::Path::to_path_buf)
}

fn run_file(path: &std::path::Path) -> std::process::Output {
    let rhp_bin = env!("CARGO_BIN_EXE_rhp");
    Command::new(rhp_bin)
        .args(["run", &path.to_string_lossy()])
        .output()
        .expect("falha ao executar o binario rhp")
}

fn run_temp_source(name: &str, source: &str) -> std::process::Output {
    let dir = std::env::temp_dir().join(format!("rhp-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("nao foi possivel criar dir temporario");
    let file = dir.join(name);
    std::fs::write(&file, source).expect("nao foi possivel escrever programa temporario");
    run_file(&file)
}

#[test]
fn rhp_run_hello_rhp_prints_expected_output() {
    let hello = workspace_root().join("hello.rhp");
    assert!(
        hello.is_file(),
        "arquivo hello.rhp nao encontrado em {}",
        hello.display()
    );

    let output = run_file(&hello);
    assert!(
        output.status.success(),
        "rhp run falhou: status={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout, b"Hello from RHP!\n",
        "saida inesperada de `rhp run hello.rhp`"
    );
}

#[test]
fn rhp_run_rejects_missing_semicolon() {
    let output = run_temp_source("bad.rhp", "echo \"x\"");
    assert!(!output.status.success(), "programa sem `;` deveria falhar");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("esperado `;`"),
        "stderr deveria mencionar `;` ausente, obteve: {stderr}"
    );
}

#[test]
fn rhp_run_expressions_and_variables() {
    let output = run_temp_source(
        "expr.rhp",
        "echo 1 + 2 * 3;\nlet x = 10;\nx = x * 2;\necho x;\necho \"a\" + \"b\";\n",
    );
    assert!(
        output.status.success(),
        "rhp run falhou: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"7\n20\nab\n");
}

#[test]
fn rhp_run_new_syntax_batch() {
    let output = run_temp_source(
        "novo.rhp",
        "const LIMITE = 3;\nlet i = 0;\nwhile i < 10 {\n  i = i + 1;\n  if i % 2 == 0 {\n    continue;\n  }\n  if i > LIMITE {\n    break;\n  }\n  echo i;\n}\necho 6 & 3;\necho 1 << 4;\necho null ?? \"padrao\";\necho true ? \"s\" : \"n\";\n",
    );
    assert!(
        output.status.success(),
        "rhp run falhou: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"1\n3\n2\n16\npadrao\ns\n");
}

#[test]
fn rhp_run_control_flow() {
    let output = run_temp_source(
        "ctrl.rhp",
        "let i = 0;\nwhile i < 3 {\n  if i == 1 {\n    echo \"um\";\n  } else {\n    echo i;\n  }\n  i = i + 1;\n}\n",
    );
    assert!(
        output.status.success(),
        "rhp run falhou: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"0\num\n2\n");
}

#[test]
fn rhp_run_for_sort_and_scopes() {
    let output = run_temp_source(
        "laco.rhp",
        "for x in [3, 1, 2] {\n  echo x;\n}\nfor k in {b: 1, a: 2} {\n  echo k;\n}\nlet t = 0;\nfor i in [1, 2, 3, 4] {\n  if i % 2 == 0 {\n    continue;\n  }\n  if i > 2 {\n    break;\n  }\n  t = t + i;\n}\necho t;\necho sort([3, 1, 2]);\n",
    );
    assert!(
        output.status.success(),
        "rhp run falhou: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"3\n1\n2\na\nb\n1\n[1, 2, 3]\n");
}

#[test]
fn rhp_run_json_roundtrip() {
    let output = run_temp_source(
        "json.rhp",
        "let v = json_parse(\"{\\\"a\\\": [1, true, null]}\");\necho v[\"a\"][0];\necho json_stringify({b: 1, a: \"x\"});\n",
    );
    assert!(
        output.status.success(),
        "rhp run falhou: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"1\n{\"a\":\"x\",\"b\":1}\n");
    // Malformado: exit 1 com mensagem de `json_parse`.
    let bad = run_temp_source("json_ruim.rhp", "echo json_parse(\"{oops}\");\n");
    assert!(!bad.status.success());
    assert!(
        String::from_utf8_lossy(&bad.stderr).contains("json_parse"),
        "stderr={}",
        String::from_utf8_lossy(&bad.stderr)
    );
}

#[test]
fn rhp_run_try_catch_finally() {
    let output = run_temp_source(
        "try.rhp",
        "try {\n  throw \"boom\";\n} catch e {\n  echo e;\n} finally {\n  echo \"fim\";\n}\ntry {\n  echo json_parse(\"{oops}\");\n} catch e {\n  echo e[\"tipo\"];\n}\n",
    );
    assert!(
        output.status.success(),
        "rhp run falhou: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"boom\nfim\nruntime\n");
    // Sem handler: exit 1 com a mensagem original.
    let uncaught = run_temp_source("try_ruim.rhp", "throw \"x\";\n");
    assert!(!uncaught.status.success());
    assert!(
        String::from_utf8_lossy(&uncaught.stderr).contains("excecao nao capturada: x"),
        "stderr={}",
        String::from_utf8_lossy(&uncaught.stderr)
    );
    // `exit` dentro de `try` pula `finally` e sai com o codigo.
    let said = run_temp_source(
        "try_sai.rhp",
        "try {\n  echo \"antes\";\n  exit(3);\n} finally {\n  echo \"fim\";\n}\n",
    );
    assert_eq!(said.status.code(), Some(3));
    assert_eq!(said.stdout, b"antes\n");
}

#[test]
fn rhp_run_sqlite_end_to_end() {
    let dir = std::env::temp_dir().join(format!("rhp-cli-db-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir temporario");
    let path = dir.join("app.db");
    let output = run_temp_source(
        "banco.rhp",
        &format!(
            "let h = db_open(\"{}\");\necho db_exec(h, \"CREATE TABLE u (id, nome)\", []);\necho db_exec(h, \"INSERT INTO u VALUES (?, ?)\", [7, \"ana\"]);\necho db_query(h, \"SELECT id, nome FROM u\", []);\necho db_close(h);\n",
            path.display()
        ),
    );
    assert!(
        output.status.success(),
        "sqlite falhou: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"0\n1\n[{id: 7, nome: ana}]\nnull\n");
    assert!(path.is_file(), "arquivo do banco nao foi criado");
}

#[test]
fn rhp_run_time_builtins() {
    let output = run_temp_source(
        "hora.rhp",
        "echo date_str(0);\necho type(now());\necho sleep(0);\n",
    );
    assert!(
        output.status.success(),
        "rhp run falhou: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"1970-01-01T00:00:00Z\nint\nnull\n");
}

#[test]
fn rhp_run_http_fetch_follows_redirects() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local deve passar");
    let port = listener.local_addr().expect("addr").port();
    let server = std::thread::spawn(move || {
        // 3 requisicoes sequenciais: /inicio -> /meio -> URL absoluta /fim.
        for (path, target, status) in [
            ("/inicio", "/meio", "302 Found"),
            (
                "/meio",
                &format!("http://127.0.0.1:{port}/fim"),
                "302 Found",
            ),
            ("/fim", "", "200 OK"),
        ] {
            let (mut stream, _) = listener.accept().expect("accept deve passar");
            let mut request = vec![0u8; 4096];
            let _ = stream.read(&mut request);
            let request_text = String::from_utf8_lossy(&request);
            assert!(
                request_text.contains(&format!("GET {path} ")),
                "pedido inesperado: {request_text}"
            );
            let (extra, body): (String, &str) = if target.is_empty() {
                ("X-Test: ok\r\n".to_string(), "chegou")
            } else {
                (format!("Location: {target}\r\n"), "")
            };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).expect("write");
        }
    });
    let output = run_temp_source(
        "fetch.rhp",
        &format!(
            "let r = http_fetch(\"http://127.0.0.1:{port}/inicio\", {{}});\necho r[\"status\"];\necho r[\"corpo\"];\necho r[\"cabecalhos\"][\"x-test\"];\n"
        ),
    );
    server.join().expect("servidor deve terminar");
    assert!(
        output.status.success(),
        "fetch falhou: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"200\nchegou\nok\n");
}

#[test]
fn rhp_run_http_fetch_sends_headers_and_stops() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    // Ecoa o cabecalho X-Eco no corpo (prova que o custom header chegou).
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local deve passar");
    let port = listener.local_addr().expect("addr").port();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept deve passar");
        let mut request = vec![0u8; 4096];
        let n = stream.read(&mut request).expect("read deve passar");
        let request_text = String::from_utf8_lossy(&request[..n]).into_owned();
        let echo = request_text
            .lines()
            .find(|l| l.to_lowercase().starts_with("x-eco:"))
            .and_then(|l| l.split_once(':').map(|(_, v)| v.trim().to_string()))
            .unwrap_or_default();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{echo}",
            echo.len()
        );
        stream.write_all(response.as_bytes()).expect("write");
    });
    let output = run_temp_source(
        "fetch_h.rhp",
        &format!(
            "let r = http_fetch(\"http://127.0.0.1:{port}/x\", {{\"headers\": {{\"X-Eco\": \"ping\"}}}});\necho r[\"corpo\"];\necho r[\"status\"];\n"
        ),
    );
    server.join().expect("servidor deve terminar");
    assert!(
        output.status.success(),
        "fetch falhou: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"ping\n200\n");

    // `seguir: false` devolve o 302 sem seguir.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local deve passar");
    let port = listener.local_addr().expect("addr").port();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept deve passar");
        let mut request = vec![0u8; 1024];
        let _ = stream.read(&mut request);
        stream
            .write_all(b"HTTP/1.1 302 Found\r\nLocation: /alvo\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .expect("write");
    });
    let output = run_temp_source(
        "fetch_nf.rhp",
        &format!(
            "let r = http_fetch(\"http://127.0.0.1:{port}/a\", {{\"seguir\": false}});\necho r[\"status\"];\necho r[\"cabecalhos\"][\"location\"];\n"
        ),
    );
    server.join().expect("servidor deve terminar");
    assert!(
        output.status.success(),
        "fetch falhou: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"302\n/alvo\n");
}

#[test]
fn rhp_run_http_fetch_enforces_limits() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    // Loop de redirects: cliente desiste apos `max_redirecionamentos`.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local deve passar");
    let port = listener.local_addr().expect("addr").port();
    let server = std::thread::spawn(move || {
        for target in ["/b", "/a", "/b"] {
            let (mut stream, _) = listener.accept().expect("accept deve passar");
            let mut request = vec![0u8; 1024];
            let _ = stream.read(&mut request);
            let response = format!(
                "HTTP/1.1 302 Found\r\nLocation: {target}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            stream.write_all(response.as_bytes()).expect("write");
        }
    });
    let output = run_temp_source(
        "fetch_loop.rhp",
        &format!(
            "echo http_fetch(\"http://127.0.0.1:{port}/a\", {{\"max_redirecionamentos\": 2}});\n"
        ),
    );
    server.join().expect("servidor deve terminar");
    assert!(!output.status.success(), "loop deveria falhar");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("redirecionamentos excedidos"),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Timeout por requisicao: servidor lento, teto curto.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local deve passar");
    let port = listener.local_addr().expect("addr").port();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept deve passar");
        let mut request = vec![0u8; 1024];
        let _ = stream.read(&mut request);
        std::thread::sleep(std::time::Duration::from_secs(3));
        let _ = stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
    });
    let output = run_temp_source(
        "fetch_tempo.rhp",
        &format!("echo http_fetch(\"http://127.0.0.1:{port}/lento\", {{\"timeout\": 1}});\n"),
    );
    server.join().expect("servidor deve terminar");
    assert!(!output.status.success(), "timeout deveria falhar");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("falhou"),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn rhp_run_exit_codes() {
    // `exit(3)`: stdout anterior preservado + codigo 3, sem mensagem de erro.
    let output = run_temp_source("sai3.rhp", "echo \"antes\";\nexit(3);\necho \"depois\";\n");
    assert_eq!(
        output.status.code(),
        Some(3),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"antes\n");
    assert!(
        output.stderr.is_empty(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    // `exit(0)`: sucesso.
    let output = run_temp_source("sai0.rhp", "exit(0);\n");
    assert!(output.status.success());
    // Codigo fora de 0-255: erro comum (exit 1).
    let output = run_temp_source("sai_ruim.rhp", "exit(256);\n");
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn rhp_run_while_scope_does_not_leak() {
    // Regressao: `let` no corpo do `while` + `break` nao vaza para fora.
    let output = run_temp_source(
        "vazou.rhp",
        "while true {\n  let vazou = 1;\n  break;\n}\necho vazou;\n",
    );
    assert!(
        !output.status.success(),
        "vazamento de escopo voltou a passar"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("nao definida"),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn rhp_run_functions_and_fib_example() {
    // Recursao direta no programa temporario.
    let output = run_temp_source(
        "fact.rhp",
        "fn fat(n) {\n  if n <= 1 {\n    return 1;\n  }\n  return n * fat(n - 1);\n}\necho fat(5);\n",
    );
    assert!(
        output.status.success(),
        "rhp run falhou: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"120\n");

    // Exemplo versionado com laco: fib(10) = 55.
    let fib = workspace_root().join("examples/fib.rhp");
    assert!(
        fib.is_file(),
        "arquivo examples/fib.rhp nao encontrado em {}",
        fib.display()
    );
    let output = run_file(&fib);
    assert!(
        output.status.success(),
        "rhp run fib.rhp falhou: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"55\n");
}

#[test]
fn rhp_run_reports_runtime_type_error() {
    let output = run_temp_source("err.rhp", "echo 1 + true;\n");
    assert!(
        !output.status.success(),
        "erro de tipo deveria falhar com codigo != 0"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("incompativeis"),
        "stderr deveria mencionar tipos incompativeis, obteve: {stderr}"
    );
}

#[test]
fn rhp_run_resolves_imports() {
    let dir = std::env::temp_dir().join(format!("rhp-test-import-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("nao foi possivel criar dir temporario");
    std::fs::write(dir.join("mat.rhp"), "fn dobro(x) {\n  return x * 2;\n}\n")
        .expect("nao foi possivel escrever mat.rhp");
    std::fs::write(
        dir.join("app.rhp"),
        "import \"mat.rhp\";\necho dobro(21);\n",
    )
    .expect("nao foi possivel escrever app.rhp");
    let output = run_file(&dir.join("app.rhp"));
    assert!(
        output.status.success(),
        "rhp run com import falhou: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"42\n");
}

#[test]
fn rhp_run_passes_extra_args_to_argv() {
    // `run_temp_source` nao passa args: sem args, argv() e vazio.
    let empty = run_temp_source("args0.rhp", "echo len(argv());\n");
    assert!(empty.status.success());
    assert_eq!(empty.stdout, b"0\n");
    // Com args extras, o binario os repassa para `argv()`.
    let dir = std::env::temp_dir().join(format!("rhp-test-argv-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("nao foi possivel criar dir temporario");
    let file = dir.join("args2.rhp");
    std::fs::write(&file, "echo len(argv());\necho argv()[1];\n").expect("write");
    let output = Command::new(env!("CARGO_BIN_EXE_rhp"))
        .args(["run", &file.to_string_lossy(), "ola", "mundo"])
        .output()
        .expect("rhp run com args");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"2\nmundo\n");
}

#[test]
fn rhp_test_runs_test_files() {
    let dir = std::env::temp_dir().join(format!("rhp-test-runner-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("nao foi possivel criar dir temporario");
    std::fs::write(dir.join("a_test.rhp"), "assert(1 + 1 == 2);\n").expect("write");
    std::fs::write(dir.join("b_test.rhp"), "assert(false);\n").expect("write");
    std::fs::write(dir.join("nao_teste.rhp"), "assert(false);\n").expect("write");
    let rhp_bin = env!("CARGO_BIN_EXE_rhp");

    let mixed = Command::new(rhp_bin)
        .args(["test", &dir.to_string_lossy()])
        .output()
        .expect("rhp test");
    assert!(!mixed.status.success(), "b_test deveria falhar");
    let out = String::from_utf8_lossy(&mixed.stdout);
    let err = String::from_utf8_lossy(&mixed.stderr);
    assert!(out.contains("a_test.rhp"), "obteve: {out}");
    assert!(out.contains("FALHOU"), "obteve: {out}");
    assert!(err.contains("1 falha(s)"), "obteve: {err}");

    let single = Command::new(rhp_bin)
        .args(["test", &dir.join("a_test.rhp").to_string_lossy()])
        .output()
        .expect("rhp test arquivo");
    assert!(single.status.success(), "a_test deveria passar");
}

#[test]
fn rhp_doctor_reports_environment() {
    let out = Command::new(env!("CARGO_BIN_EXE_rhp"))
        .arg("doctor")
        .output()
        .expect("rhp doctor");
    assert!(out.status.success(), "doctor deveria passar");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("rhp "), "obteve: {text}");
    assert!(text.contains("autoteste: ok"), "obteve: {text}");
}

#[test]
fn rhp_run_max_steps_stops_infinite_loop() {
    let output = Command::new(env!("CARGO_BIN_EXE_rhp"))
        .args(["run", "--max-steps", "1000"])
        .arg({
            let dir = std::env::temp_dir().join(format!("rhp-test-loop-{}", std::process::id()));
            std::fs::create_dir_all(&dir).expect("nao foi possivel criar dir temporario");
            let file = dir.join("loop.rhp");
            std::fs::write(&file, "while true {\n}\n").expect("nao foi possivel escrever loop.rhp");
            file
        })
        .output()
        .expect("falha ao executar o binario rhp");
    assert!(!output.status.success(), "loop infinito deveria falhar");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("limite de instrucoes"),
        "stderr deveria mencionar o limite, obteve: {stderr}"
    );
}

#[test]
fn rhp_check_reports_ok_and_diagnostics() {
    let rhp_bin = env!("CARGO_BIN_EXE_rhp");
    let ok = run_temp_source("ok.rhp", "echo 1 + 2;\n");
    // `run_temp_source` usa `run`; check e testado abaixo via binario.
    assert!(ok.status.success());

    let dir = std::env::temp_dir().join(format!("rhp-test-check-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("nao foi possivel criar dir temporario");
    let good = dir.join("good.rhp");
    std::fs::write(&good, "echo 1 + 2;\n").expect("write");
    let out = Command::new(rhp_bin)
        .args(["check", &good.to_string_lossy()])
        .output()
        .expect("rhp check");
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("ok"));

    let bad = dir.join("bad.rhp");
    std::fs::write(&bad, "echo \"x\"\n").expect("write");
    let out = Command::new(rhp_bin)
        .args(["check", &bad.to_string_lossy()])
        .output()
        .expect("rhp check");
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("Error[E1004]"), "obteve: {err}");

    let out = Command::new(rhp_bin)
        .args(["check", "--json", &bad.to_string_lossy()])
        .output()
        .expect("rhp check --json");
    assert!(!out.status.success());
    let js = String::from_utf8_lossy(&out.stdout);
    assert!(js.contains("\"code\":\"E1004\""), "obteve: {js}");
    assert!(js.contains("\"ok\":false"), "obteve: {js}");

    let out = Command::new(rhp_bin)
        .args(["check", "--json", &good.to_string_lossy()])
        .output()
        .expect("rhp check --json ok");
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("\"ok\":true"));
}

#[test]
fn vscode_grammar_covers_language() {
    // Consistencia gramatica-vs-linguagem (sem engine TextMate no std):
    // a gramatica precisa citar todo keyword e builtin implementados.
    let grammar = workspace_root().join("editors/vscode/syntaxes/rhp.tmLanguage.json");
    assert!(
        grammar.is_file(),
        "gramatica nao encontrada: {}",
        grammar.display()
    );
    let text = std::fs::read_to_string(&grammar).expect("nao foi possivel ler gramatica");
    assert!(text.contains("\"scopeName\": \"source.rhp\""));
    for keyword in [
        "echo", "let", "const", "if", "else", "while", "for", "try", "catch", "finally", "throw",
        "export", "break", "continue", "fn", "return", "import", "true", "false", "null",
    ] {
        assert!(
            text.contains(keyword),
            "keyword fora da gramatica: {keyword}"
        );
    }
    for builtin in [
        "len",
        "push",
        "pop",
        "keys",
        "str",
        "int",
        "float",
        "type",
        "assert",
        "http_get",
        "http_fetch",
        "contains",
        "starts_with",
        "ends_with",
        "split",
        "join",
        "trim",
        "upper",
        "lower",
        "replace",
        "abs",
        "min",
        "max",
        "sqrt",
        "pow",
        "floor",
        "ceil",
        "values",
        "entries",
        "merge",
        "has",
        "fs_read",
        "fs_write",
        "fs_append",
        "fs_exists",
        "argv",
        "env",
        "sort",
        "exit",
        "json_parse",
        "json_stringify",
        "now",
        "sleep",
        "date_str",
        "db_open",
        "db_exec",
        "db_query",
        "db_close",
    ] {
        assert!(
            text.contains(builtin),
            "builtin fora da gramatica: {builtin}"
        );
    }
}

#[test]
fn rhp_run_reports_structured_diagnostics() {
    // Erro lexico: codigo E0001 + trecho com caret.
    let lex = run_temp_source("lexerr.rhp", "echo @;\n");
    assert!(!lex.status.success());
    let lex_err = String::from_utf8_lossy(&lex.stderr);
    assert!(lex_err.contains("Error[E0001]"), "obteve: {lex_err}");
    assert!(lex_err.contains('^'), "obteve: {lex_err}");

    // Erro sintatico: codigo E1004 + snippet da fonte.
    let syn = run_temp_source("synerr.rhp", "echo \"x\"\n");
    assert!(!syn.status.success());
    let syn_err = String::from_utf8_lossy(&syn.stderr);
    assert!(syn_err.contains("Error[E1004]"), "obteve: {syn_err}");
    assert!(syn_err.contains("echo \"x\""), "obteve: {syn_err}");
    assert!(syn_err.contains("help:"), "obteve: {syn_err}");

    // Erro de runtime: codigo E3001 + ajuda.
    let run = run_temp_source("runerr.rhp", "echo 1 + true;\n");
    assert!(!run.status.success());
    let run_err = String::from_utf8_lossy(&run.stderr);
    assert!(run_err.contains("Error[E3001]"), "obteve: {run_err}");
    assert!(run_err.contains("help:"), "obteve: {run_err}");
}

#[test]
fn rhp_run_http_get_against_local_server() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local deve passar");
    let port = listener
        .local_addr()
        .expect("addr local deve existir")
        .port();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept deve passar");
        let mut request = vec![0u8; 1024];
        let _ = stream.read(&mut request);
        let body = "ola http";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .expect("write deve passar");
    });
    let output = run_temp_source(
        "http.rhp",
        &format!("echo http_get(\"http://127.0.0.1:{port}/saudacao\");\n"),
    );
    server.join().expect("servidor deve terminar");
    assert!(
        output.status.success(),
        "http_get local falhou: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"ola http\n");
}

#[test]
fn rhp_run_http_get_rejects_huge_body() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local deve passar");
    let port = listener
        .local_addr()
        .expect("addr local deve existir")
        .port();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept deve passar");
        let mut request = vec![0u8; 1024];
        let _ = stream.read(&mut request);
        let big = "x".repeat(2 * 1024 * 1024);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{big}",
            big.len()
        );
        stream
            .write_all(response.as_bytes())
            .expect("write deve passar");
    });
    let output = run_temp_source(
        "http-big.rhp",
        &format!("echo http_get(\"http://127.0.0.1:{port}/grande\");\n"),
    );
    server.join().expect("servidor deve terminar");
    assert!(!output.status.success(), "corpo gigante deveria falhar");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("limite"),
        "stderr deveria mencionar o limite, obteve: {stderr}"
    );
}

#[test]
fn rhp_fmt_rewrites_and_checks() {
    let rhp_bin = env!("CARGO_BIN_EXE_rhp");
    let dir = std::env::temp_dir().join(format!("rhp-test-fmt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("nao foi possivel criar dir temporario");
    let file = dir.join("messy.rhp");
    std::fs::write(&file, "echo   1+2;\n").expect("nao foi possivel escrever messy.rhp");

    // --check falha antes de formatar.
    let check_before = Command::new(rhp_bin)
        .args(["fmt", "--check", &file.to_string_lossy()])
        .output()
        .expect("falha ao executar rhp fmt --check");
    assert!(
        !check_before.status.success(),
        "`--check` deveria falhar em arquivo fora do formato"
    );

    let fmt = Command::new(rhp_bin)
        .args(["fmt", &file.to_string_lossy()])
        .output()
        .expect("falha ao executar rhp fmt");
    assert!(
        fmt.status.success(),
        "rhp fmt falhou: stderr={}",
        String::from_utf8_lossy(&fmt.stderr)
    );
    let content = std::fs::read_to_string(&file).expect("nao foi possivel reler arquivo");
    assert_eq!(content, "echo 1 + 2;\n");

    let check_after = Command::new(rhp_bin)
        .args(["fmt", "--check", &file.to_string_lossy()])
        .output()
        .expect("falha ao executar rhp fmt --check");
    assert!(
        check_after.status.success(),
        "`--check` deveria passar apos formatar"
    );
}

#[test]
fn rhp_run_reports_circular_import() {
    let dir = std::env::temp_dir().join(format!("rhp-test-cycle-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("nao foi possivel criar dir temporario");
    std::fs::write(dir.join("a.rhp"), "import \"b.rhp\";\n")
        .expect("nao foi possivel escrever a.rhp");
    std::fs::write(dir.join("b.rhp"), "import \"a.rhp\";\n")
        .expect("nao foi possivel escrever b.rhp");
    let output = run_file(&dir.join("a.rhp"));
    assert!(!output.status.success(), "ciclo deveria falhar");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("importacao circular"),
        "stderr deveria mencionar ciclo, obteve: {stderr}"
    );
}

#[test]
fn rhp_run_all_examples_succeed() {
    // Auditoria AUD-212: todo exemplo versionado executa com exit 0.
    // Saidas conhecidas ficam pinadas; demais exemplos ao menos nao falham.
    let mut count = 0;
    let examples = workspace_root().join("examples");
    let mut entries: Vec<_> = std::fs::read_dir(&examples)
        .expect("nao foi possivel listar examples/")
        .filter_map(Result::ok)
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rhp") {
            continue;
        }
        count += 1;
        let output = run_file(&path);
        assert!(
            output.status.success(),
            "exemplo {} falhou: stderr={}",
            path.display(),
            String::from_utf8_lossy(&output.stderr)
        );
        if path.file_name().and_then(|n| n.to_str()) == Some("fib.rhp") {
            assert_eq!(output.stdout, b"55\n");
        }
        if path.file_name().and_then(|n| n.to_str()) == Some("fatorial.rhp") {
            assert_eq!(output.stdout, b"720\n");
        }
        if path.file_name().and_then(|n| n.to_str()) == Some("hello_world.rhp") {
            assert_eq!(output.stdout, b"Hello, world!\nHello from RHP!\n");
        }
    }
    assert!(count >= 1, "nenhum exemplo encontrado em examples/");
}
