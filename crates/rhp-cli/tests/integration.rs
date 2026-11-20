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

fn run_temp_source_trace(name: &str, source: &str) -> std::process::Output {
    let dir = std::env::temp_dir().join(format!("rhp-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("nao foi possivel criar dir temporario");
    let file = dir.join(name);
    std::fs::write(&file, source).expect("nao foi possivel escrever programa temporario");
    let rhp_bin = env!("CARGO_BIN_EXE_rhp");
    Command::new(rhp_bin)
        .args(["run", "--trace", &file.to_string_lossy()])
        .output()
        .expect("falha ao executar o binario rhp")
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
fn rhp_run_postgres_end_to_end() {
    // Exige o servidor de teste: `docker run -d --name rhp-pg-test
    // -e POSTGRES_USER=rhp -e POSTGRES_PASSWORD=rhp -e POSTGRES_DB=rhp
    // -e POSTGRES_HOST_AUTH_METHOD=trust -p 127.0.0.1:55432:5432
    // postgres:16-alpine` (ver ADR 0011; sem mock, so servidor real).
    let pg = "pg:host=127.0.0.1 port=55432 user=rhp dbname=rhp";
    let output = run_temp_source(
        "pg.rhp",
        &format!(
            "let h = db_open(\"{pg}\");\necho db_exec(h, \"DROP TABLE IF EXISTS t\", []);\necho db_exec(h, \"CREATE TABLE t (id integer, nome text, ativo boolean, preco double precision)\", []);\necho db_exec(h, \"INSERT INTO t VALUES ($1, $2, $3, $4)\", [1, \"ana\", true, 2.5]);\necho db_query(h, \"SELECT id, nome, ativo, preco FROM t WHERE id = $1\", [1]);\necho db_close(h);\n"
        ),
    );
    assert!(
        output.status.success(),
        "postgres falhou: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"0\n0\n1\n[{ativo: true, id: 1, nome: ana, preco: 2.5}]\nnull\n"
    );
    // SQL invalido: erro do servidor com codigo.
    let output = run_temp_source(
        "pg_ruim.rhp",
        &format!("let h = db_open(\"{pg}\");\necho db_exec(h, \"SELEC 1\", []);\n"),
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("[42601]"),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    // RHP-AUD-IND-002: erro drena ate ReadyForQuery — o proximo comando na
    // mesma conexao nao le resposta alheia (antes: sucesso falso + erro
    // um comando atrasado).
    let output = run_temp_source(
        "pg_sync.rhp",
        &format!(
            "let h = db_open(\"{pg}\");\ntry {{ echo db_exec(h, \"INSERT INTO t VALUES (1, 'x', true, 1.0, 2.0)\", []); }} catch e {{ echo \"erro-ok\"; }}\necho db_query(h, \"SELECT 1 AS um\", []);\necho db_close(h);\n"
        ),
    );
    assert!(
        output.status.success(),
        "postgres dessincronizou: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"erro-ok\n[{um: 1}]\nnull\n");
}

#[test]
fn rhp_pkg_init_add_install_e_check() {
    use std::process::Command;
    let rhp = env!("CARGO_BIN_EXE_rhp");
    let dir = std::env::temp_dir().join(format!("rhp-pkgcli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let lib = dir.join("libtempo");
    std::fs::create_dir(&lib).expect("libdir");
    std::fs::write(
        lib.join("mat.rhp"),
        "export fn dobro(x) {\n  return x + x;\n}\n",
    )
    .expect("lib");
    let app = dir.join("app");
    std::fs::create_dir_all(&app).expect("appdir");
    let run = |args: &[&str]| -> std::process::Output {
        Command::new(rhp)
            .args(args)
            .current_dir(&app)
            .output()
            .expect("rodar rhp")
    };
    let out = run(&["init"]);
    assert!(
        out.status.success(),
        "init: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(app.join("rhp.toml").is_file());
    let out = run(&["pkg", "add", "tempo", "../libtempo"]);
    assert!(
        out.status.success(),
        "add: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = run(&["pkg", "install"]);
    assert!(
        out.status.success(),
        "install: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(app.join("pacotes/tempo/mat.rhp").is_file());
    assert!(app.join("rhp.lock").is_file());
    // Programa usa o pacote instalado.
    std::fs::write(
        app.join("main.rhp"),
        "import \"pacotes/tempo/mat.rhp\";\necho dobro(21);\n",
    )
    .expect("main");
    let out = run(&["run", "main.rhp"]);
    assert!(
        out.status.success(),
        "run: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, b"42\n");
    // Check passa; apos adulterar, falha.
    let out = run(&["pkg", "install", "--check"]);
    assert!(
        out.status.success(),
        "check: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    std::fs::write(app.join("pacotes/tempo/mat.rhp"), "adulterado\n").expect("adulterar");
    let out = run(&["pkg", "install", "--check"]);
    assert!(!out.status.success(), "adulteracao deveria falhar");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("alterado"),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
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

/// Servidor `http_serve` como processo filho; porta lida da primeira
/// linha do stdout (`rhp: ouvindo em 127.0.0.1:PORTA`). Mata ao sair.
struct ServerChild {
    child: Option<std::process::Child>,
    port: u16,
    announce: String,
}

impl ServerChild {
    fn spawn(name: &str, source: &str) -> Self {
        use std::io::BufRead;
        let dir = std::env::temp_dir().join(format!("rhp-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("nao foi possivel criar dir temporario");
        let file = dir.join(name);
        std::fs::write(&file, source).expect("nao foi possivel escrever servidor temporario");
        let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_rhp"))
            .args(["run", &file.to_string_lossy()])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("falha ao iniciar o servidor");
        let stdout = child.stdout.take().expect("stdout do servidor");
        let mut lines = std::io::BufReader::new(stdout).lines();
        let first = lines
            .next()
            .expect("servidor fechou stdout sem anunciar")
            .expect("leitura do anuncio falhou");
        let port: u16 = first
            .rsplit(':')
            .next()
            .expect("anuncio sem porta")
            .trim()
            .parse()
            .expect("porta do anuncio invalida");
        // Devolve o stdout restante? Nao preciso: descarta o leitor.
        Self {
            child: Some(child),
            port,
            announce: first,
        }
    }

    /// GET cru e resposta crua.
    fn get(&self, path: &str) -> String {
        Self::roundtrip(
            self.port,
            &format!("GET {path} HTTP/1.0\r\nHost: x\r\nConnection: close\r\n\r\n"),
        )
    }

    /// POST cru com corpo.
    fn post(&self, path: &str, body: &str) -> String {
        Self::roundtrip(
            self.port,
            &format!(
                "POST {path} HTTP/1.0\r\nHost: x\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            ),
        )
    }

    /// GET cru com cabecalhos extras (ex.: `Cookie`).
    fn get_with(&self, path: &str, extra: &str) -> String {
        Self::roundtrip(
            self.port,
            &format!("GET {path} HTTP/1.0\r\nHost: x\r\n{extra}\r\nConnection: close\r\n\r\n"),
        )
    }

    /// POST cru com corpo e cabecalhos extras.
    fn post_raw(&self, path: &str, body: &str, headers: &[(&str, &str)]) -> String {
        let mut request = format!("POST {path} HTTP/1.0\r\nHost: x\r\n");
        for (name, value) in headers {
            request.push_str(&format!("{name}: {value}\r\n"));
        }
        request.push_str(&format!(
            "Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ));
        Self::roundtrip(self.port, &request)
    }

    fn roundtrip(port: u16, request: &str) -> String {
        use std::io::{Read, Write};
        let mut stream =
            std::net::TcpStream::connect(("127.0.0.1", port)).expect("conectar no servidor");
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .expect("timeout");
        stream
            .write_all(request.as_bytes())
            .expect("enviar request");
        let mut out = Vec::new();
        stream.read_to_end(&mut out).expect("ler resposta");
        String::from_utf8_lossy(&out).into_owned()
    }

    /// Roundtrip de bytes crus (para requests que nao sao UTF-8 válido
    /// como texto ou que fecham o envio antes do corpo).
    fn roundtrip_bytes(port: u16, request: &[u8], shutdown_write: bool) -> String {
        use std::io::{Read, Write};
        let mut stream =
            std::net::TcpStream::connect(("127.0.0.1", port)).expect("conectar no servidor");
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .expect("timeout");
        stream.write_all(request).expect("enviar request");
        if shutdown_write {
            let _ = stream.shutdown(std::net::Shutdown::Write);
        }
        let mut out = Vec::new();
        stream.read_to_end(&mut out).expect("ler resposta");
        String::from_utf8_lossy(&out).into_owned()
    }

    fn wait(mut self) -> std::process::Output {
        self.child
            .take()
            .expect("servidor ja consumido")
            .wait_with_output()
            .expect("esperar servidor")
    }
}

impl Drop for ServerChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[test]
fn rhp_run_http_serve_basic() {
    let server = ServerChild::spawn(
        "srv.rhp",
        "fn app(req) {\n  if req[\"caminho\"] == \"/criar\" {\n    return {status: 201, cabecalhos: {\"x_feita\": \"sim\"}, corpo: \"criado \" + req[\"corpo\"]};\n  }\n  return \"ola \" + req[\"query\"][\"nome\"];\n}\nhttp_serve(0, \"app\", {\"max_requisicoes\": 3});\n",
    );
    let r1 = server.get("/oi?nome=mundo&x=1");
    assert!(r1.contains("200 OK"), "obteve: {r1}");
    assert!(r1.ends_with("ola mundo"), "obteve: {r1}");
    let r2 = server.post("/criar", "abc");
    assert!(r2.contains("201 Created"), "obteve: {r2}");
    assert!(r2.contains("x_feita: sim"), "obteve: {r2}");
    assert!(r2.ends_with("criado abc"), "obteve: {r2}");
    // Query decodificada (%20) e header de request visivel ao handler.
    let r3 = server.get("/q?nome=ola%20mundo");
    assert!(r3.ends_with("ola ola mundo"), "obteve: {r3}");
    let out = server.wait();
    assert!(
        out.status.success(),
        "servidor falhou: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn rhp_run_http_serve_com_async_no_handler() {
    // Regressao: `handle_one` define o programa (senao `async_spawn`
    // no handler falha com "sem programa-fonte").
    let server = ServerChild::spawn(
        "srv_async.rhp",
        "fn trabalho() {\n  return \"feito-ok\";\n}\nfn app(req) {\n  let h = async_spawn(\"trabalho\");\n  return async_join(h);\n}\nhttp_serve(0, \"app\", {\"max_requisicoes\": 1});\n",
    );
    let r1 = server.get("/u");
    assert!(r1.contains("200 OK"), "obteve: {r1}");
    assert!(r1.ends_with("feito-ok"), "obteve: {r1}");
    let out = server.wait();
    assert!(
        out.status.success(),
        "servidor falhou: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn rhp_run_http_serve_500_sem_vazar() {
    let server = ServerChild::spawn(
        "srv500.rhp",
        "fn app(req) {\n  if req[\"caminho\"] == \"/ruim\" {\n    throw \"segredo interno\";\n  }\n  return \"ok\";\n}\nhttp_serve(0, \"app\", {\"max_requisicoes\": 2});\n",
    );
    let bad = server.get("/ruim");
    assert!(bad.contains("500"), "obteve: {bad}");
    assert!(bad.ends_with("erro interno"), "obteve: {bad}");
    assert!(!bad.contains("segredo"), "vazou detalhe: {bad}");
    // Servidor segue vivo apos o erro.
    let ok = server.get("/ok");
    assert!(ok.ends_with("ok"), "obteve: {ok}");
    let out = server.wait();
    assert!(
        out.status.success(),
        "servidor falhou: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn rhp_run_http_serve_exit_encerra() {
    // `exit` no handler termina o servidor com o codigo (sem resposta).
    let server = ServerChild::spawn(
        "srvsai.rhp",
        "fn app(req) {\n  exit(3);\n}\nhttp_serve(0, \"app\", {\"max_requisicoes\": 5});\n",
    );
    let _ = server.get("/sair");
    let out = server.wait();
    assert_eq!(
        out.status.code(),
        Some(3),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn rhp_run_http_sessao_com_banco() {
    // Sessao = cookie + linha no SQLite (stateless por design; handler
    // abre o banco por request). XSS sai escapado pelo `template`.
    let dir = std::env::temp_dir().join(format!("rhp-sessao-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir temporario");
    let db = dir.join("sess.rhp.db");
    let db_path = db.to_string_lossy().into_owned();
    let source = "fn app(req) {\n  let h = db_open(\"@DB@\");\n  echo db_exec(h, \"CREATE TABLE IF NOT EXISTS sessoes (id text, usuario text)\", []);\n  if req[\"caminho\"] == \"/login\" {\n    let sid = \"s-\" + req[\"query\"][\"u\"];\n    echo db_exec(h, \"DELETE FROM sessoes WHERE id = ?\", [sid]);\n    echo db_exec(h, \"INSERT INTO sessoes VALUES (?, ?)\", [sid, req[\"query\"][\"u\"]]);\n    let saida = {status: 200, cabecalhos: {\"set-cookie\": \"sid=\" + sid}, corpo: template(\"logado como {{u}}!\", {\"u\": req[\"query\"][\"u\"]})};\n    echo db_close(h);\n    return saida;\n  }\n  if req[\"caminho\"] == \"/eu\" {\n    let quem = \"anonimo\";\n    let c = req[\"cookies\"];\n    if has(c, \"sid\") {\n      let r = db_query(h, \"SELECT usuario FROM sessoes WHERE id = ?\", [c[\"sid\"]]);\n      if len(r) == 1 { quem = r[0][\"usuario\"]; }\n    }\n    let pagina = template(\"<h1>ola, {{quem}}!</h1>\", {\"quem\": quem});\n    echo db_close(h);\n    return pagina;\n  }\n  echo db_close(h);\n  return \"nada\";\n}\nhttp_serve(0, \"app\", {\"max_requisicoes\": 4});\n"
        .replace("@DB@", &db_path);
    let server = ServerChild::spawn("sessao.rhp", &source);
    // Login com nome malicioso (XSS tenta passar pelo template).
    let r1 = server.post_raw("/login?u=<b>ana</b>", "", &[("Cookie", "x=1")]);
    assert!(r1.contains("200 OK"), "obteve: {r1}");
    assert!(
        r1.contains("logado como &lt;b&gt;ana&lt;/b&gt;!"),
        "obteve: {r1}"
    );
    assert!(!r1.contains("<b>ana</b>!"), "vazou sem escape: {r1}");
    assert!(r1.contains("set-cookie: sid="), "obteve: {r1}");
    // Perfil com o cookie: nome escapado.
    let r2 = server.get_with("/eu", "Cookie: sid=s-<b>ana</b>");
    assert!(
        r2.ends_with("<h1>ola, &lt;b&gt;ana&lt;/b&gt;!</h1>"),
        "obteve: {r2}"
    );
    // Sem cookie: anonimo. Cookie desconhecido: anonimo.
    let r3 = server.get("/eu");
    assert!(r3.ends_with("<h1>ola, anonimo!</h1>"), "obteve: {r3}");
    let r4 = server.get_with("/eu", "Cookie: sid=s-ninguem");
    assert!(r4.ends_with("<h1>ola, anonimo!</h1>"), "obteve: {r4}");
    let out = server.wait();
    assert!(
        out.status.success(),
        "servidor falhou: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn rhp_run_http_serve_respeita_limite_taxa() {
    // Limite generoso: 3 rapidas passam (fiação nao quebra o fluxo).
    let server = ServerChild::spawn(
        "srvlim.rhp",
        "fn app(req) {\n  return \"ok\";\n}\nhttp_serve(0, \"app\", {\"max_requisicoes\": 3, \"limite_taxa\": 1000});\n",
    );
    for _ in 0..3 {
        let r = server.get("/x");
        assert!(r.contains("200 OK"), "obteve: {r}");
    }
    let out = server.wait();
    assert!(
        out.status.success(),
        "servidor falhou: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn rhp_build_gera_bundle_equivalente() {
    let dir = std::env::temp_dir().join(format!("rhp-build-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    std::fs::write(
        dir.join("lib.rhp"),
        "export fn dobro(x) {\n  return x + x;\n}\n",
    )
    .expect("lib");
    let main = dir.join("main.rhp");
    std::fs::write(
        &main,
        "import { dobro } from \"lib.rhp\";\necho dobro(21);\n",
    )
    .expect("main");
    let rhp = env!("CARGO_BIN_EXE_rhp");
    let out = std::process::Command::new(rhp)
        .args(["build", &main.to_string_lossy()])
        .output()
        .expect("rhp build");
    assert!(
        out.status.success(),
        "build falhou: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let bundle = dir.join("main.bundle.rhp");
    assert!(bundle.is_file(), "bundle nao criado");
    let src = std::fs::read_to_string(&bundle).expect("ler bundle");
    assert!(!src.contains("import "), "bundle nao deve ter import");
    // Bundle executa igual ao original.
    let orig = std::process::Command::new(rhp)
        .args(["run", &main.to_string_lossy()])
        .output()
        .expect("run orig");
    let bund = std::process::Command::new(rhp)
        .args(["run", &bundle.to_string_lossy()])
        .output()
        .expect("run bundle");
    assert_eq!(orig.stdout, bund.stdout);
    assert_eq!(orig.stdout, b"42\n");
    // Destino explicito + idempotencia do formato.
    let alt = dir.join("out.rhp");
    let out = std::process::Command::new(rhp)
        .args(["build", &main.to_string_lossy(), &alt.to_string_lossy()])
        .output()
        .expect("rhp build -o");
    assert!(
        out.status.success(),
        "build -o falhou: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(alt.is_file());
}

#[test]
fn rhp_run_http_serve_rejeita_handler_desconhecido() {
    let output = run_temp_source("srvbad.rhp", "http_serve(0, \"inexistente\", {});\n");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("nao definido"),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn rhp_run_http_serve_host_bind() {
    // Bind em todas as interfaces: anuncio mostra 0.0.0.0 e 127.0.0.1 alcanca.
    let server = ServerChild::spawn(
        "srvhost.rhp",
        "fn app(req) {\n  return \"ok\";\n}\nhttp_serve(0, \"app\", {\"host\": \"0.0.0.0\", \"max_requisicoes\": 1});\n",
    );
    assert!(
        server.announce.contains("0.0.0.0"),
        "anuncio={}",
        server.announce
    );
    let r = server.get("/");
    assert!(r.contains("200 OK"), "obteve: {r}");
    assert!(r.ends_with("ok"), "obteve: {r}");
    let out = server.wait();
    assert!(out.status.success());
    // Bind default continua local.
    let server = ServerChild::spawn(
        "srvhost2.rhp",
        "fn app(req) {\n  return \"ok\";\n}\nhttp_serve(0, \"app\", {\"max_requisicoes\": 1});\n",
    );
    assert!(
        server.announce.contains("127.0.0.1"),
        "anuncio={}",
        server.announce
    );
    let r = server.get("/");
    assert!(r.contains("200 OK"), "obteve: {r}");
    let out = server.wait();
    assert!(out.status.success());
}

#[test]
fn rhp_run_http_serve_rejeita_host_invalido() {
    for host in ["999.1.1.1", "", "example.com", "0.0.0.0/0"] {
        let output = run_temp_source(
            "srvhostbad.rhp",
            &format!("fn app(req) {{\n  return \"x\";\n}}\nhttp_serve(0, \"app\", {{\"host\": \"{host}\"}});\n"),
        );
        assert!(!output.status.success(), "host aceito: {host}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("`host` invalido"),
            "stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    // `localhost` e apelido de 127.0.0.1 (anuncio prova).
    let server = ServerChild::spawn(
        "srvlocal.rhp",
        "fn app(req) {\n  return \"ok\";\n}\nhttp_serve(0, \"app\", {\"host\": \"localhost\", \"max_requisicoes\": 1});\n",
    );
    assert!(
        server.announce.contains("127.0.0.1"),
        "anuncio={}",
        server.announce
    );
    let r = server.get("/");
    assert!(r.contains("200 OK"), "obteve: {r}");
    let out = server.wait();
    assert!(out.status.success());
}

#[test]
fn rhp_run_http_serve_porta_ocupada() {
    let ocupada = std::net::TcpListener::bind("127.0.0.1:0").expect("bind local deve passar");
    let porta = ocupada.local_addr().expect("addr").port();
    let output = run_temp_source(
        "srvbusy.rhp",
        &format!("fn app(req) {{\n  return \"x\";\n}}\nhttp_serve({porta}, \"app\", {{}});\n"),
    );
    assert!(!output.status.success());
    let err = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(err.contains("bind"), "stderr={err}");
    assert!(err.contains(&porta.to_string()), "stderr={err}");
    drop(ocupada);
}

#[test]
fn rhp_run_http_serve_handler_como_valor() {
    // Nome de funcao como valor (sem string) resolve no topo.
    let server = ServerChild::spawn(
        "srvval.rhp",
        "fn app(req) {\n  return \"via-valor\";\n}\nhttp_serve(0, app, {\"max_requisicoes\": 1});\n",
    );
    let r = server.get("/");
    assert!(r.contains("200 OK"), "obteve: {r}");
    assert!(r.ends_with("via-valor"), "obteve: {r}");
    let out = server.wait();
    assert!(out.status.success());
    // Closure nao tem entrada no topo: erro claro, sem hang.
    let output = run_temp_source(
        "srvclo.rhp",
        "let h = fn (req) => \"x\";\nhttp_serve(0, h, {});\n",
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("nivel superior"),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn rhp_run_http_serve_400s_e_sobrevive() {
    let server = ServerChild::spawn(
        "srv400.rhp",
        "fn app(req) {\n  if req[\"caminho\"] == \"/eco\" {\n    return req[\"corpo\"];\n  }\n  return \"ok\";\n}\nhttp_serve(0, \"app\", {\"max_requisicoes\": 7});\n",
    );
    // Sem status-line valida.
    let r = ServerChild::roundtrip_bytes(server.port, b"GARBAGE\r\n\r\n", false);
    assert!(r.contains("400 Bad Request"), "obteve: {r}");
    // `Transfer-Encoding: chunked` funciona (corpo remontado e ecoado).
    let r = ServerChild::roundtrip_bytes(
        server.port,
        b"POST /eco HTTP/1.1\r\nHost: x\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n5\r\nola, \r\n6\r\nmundo!\r\n0\r\n\r\n",
        false,
    );
    assert!(r.contains("200 OK"), "obteve: {r}");
    assert!(r.ends_with("ola, mundo!"), "obteve: {r}");
    // Chunk com tamanho invalido = 400.
    let r = ServerChild::roundtrip_bytes(
        server.port,
        b"POST /eco HTTP/1.1\r\nHost: x\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\nZZ\r\nxx\r\n0\r\n\r\n",
        false,
    );
    assert!(r.contains("400 Bad Request"), "obteve: {r}");
    // Chunk truncado (declara 5, envia 2 e fecha envio) = 400.
    let r = ServerChild::roundtrip_bytes(
        server.port,
        b"POST /eco HTTP/1.1\r\nHost: x\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n5\r\nab",
        true,
    );
    assert!(r.contains("400 Bad Request"), "obteve: {r}");
    // Corpo incompleto (declara 100, envia 5 e fecha envio).
    let r = ServerChild::roundtrip_bytes(
        server.port,
        b"POST /x HTTP/1.0\r\nHost: x\r\nContent-Length: 100\r\nConnection: close\r\n\r\ncurto",
        true,
    );
    assert!(r.contains("400 Bad Request"), "obteve: {r}");
    // Content-Length invalido.
    let r = ServerChild::roundtrip_bytes(
        server.port,
        b"POST /x HTTP/1.0\r\nHost: x\r\nContent-Length: muitos\r\nConnection: close\r\n\r\n",
        false,
    );
    assert!(r.contains("400 Bad Request"), "obteve: {r}");
    // Servidor segue atendendo apos os 400s.
    let r = server.get("/");
    assert!(r.contains("200 OK"), "obteve: {r}");
    assert!(r.ends_with("ok"), "obteve: {r}");
    let out = server.wait();
    assert!(out.status.success());
}

#[test]
fn rhp_run_http_serve_cabecalhos_grandes_e_utf8() {
    let server = ServerChild::spawn(
        "srvbig.rhp",
        "fn app(req) {\n  return req[\"query\"][\"nome\"];\n}\nhttp_serve(0, \"app\", {\"max_requisicoes\": 2});\n",
    );
    // Cabecalhos acima do teto (64 KiB) viram 400.
    let grande = format!(
        "GET / HTTP/1.0\r\nHost: x\r\nX-Lixo: {}\r\nConnection: close\r\n\r\n",
        "a".repeat(70 * 1024)
    );
    let r = ServerChild::roundtrip_bytes(server.port, grande.as_bytes(), false);
    assert!(
        r.contains("400 Bad Request"),
        "obteve: {}",
        &r[..r.len().min(60)]
    );
    // Query UTF-8 percent-encoded decodifica em bytes (nao latin1).
    let r = server.get("/q?nome=caf%C3%A9");
    assert!(r.contains("200 OK"), "obteve: {r}");
    assert!(r.ends_with("café"), "obteve: {r}");
    let out = server.wait();
    assert!(out.status.success());
}

#[test]
fn rhp_run_http_serve_handler_loop_limitado() {
    // Handler em loop eterno: orcamento de passos vira 500; servidor segue.
    let server = ServerChild::spawn(
        "srvloop.rhp",
        "fn app(req) {\n  while true {\n  }\n  return \"nunca\";\n}\nhttp_serve(0, \"app\", {\"max_requisicoes\": 2, \"max_passos\": 200000});\n",
    );
    let r = server.get("/loop");
    assert!(r.contains("500 Internal Server Error"), "obteve: {r}");
    assert!(r.ends_with("erro interno"), "obteve: {r}");
    // Servidor segue vivo (nao atingiu `max_requisicoes`): o Drop mata o filho.
}

#[test]
fn rhp_run_http_serve_aninhado_rejeitado() {
    // `http_serve` dentro do handler: erro claro (500), sem hang, segue.
    let server = ServerChild::spawn(
        "srvnest.rhp",
        "fn app(req) {\n  http_serve(9999, \"app\", {});\n  return \"x\";\n}\nhttp_serve(0, \"app\", {\"max_requisicoes\": 2});\n",
    );
    let r = server.get("/a");
    assert!(r.contains("500 Internal Server Error"), "obteve: {r}");
    let r = server.get("/b");
    assert!(r.contains("500 Internal Server Error"), "obteve: {r}");
    let out = server.wait();
    assert!(out.status.success());
}

#[test]
fn rhp_run_http_serve_conexoes_concorrentes() {
    // 8 clientes simultaneos: todos 200 (thread por conexao).
    let server = ServerChild::spawn(
        "srvconc.rhp",
        "fn app(req) {\n  return \"ok\" + req[\"query\"][\"n\"];\n}\nhttp_serve(0, \"app\", {\"max_requisicoes\": 8});\n",
    );
    let port = server.port;
    let mut alvos = Vec::new();
    for n in 0..8 {
        alvos.push(std::thread::spawn(move || {
            ServerChild::roundtrip(
                port,
                &format!("GET /?n={n} HTTP/1.0\r\nHost: x\r\nConnection: close\r\n\r\n"),
            )
        }));
    }
    for (n, alvo) in alvos.into_iter().enumerate() {
        let r = alvo.join().expect("thread do cliente");
        assert!(r.contains("200 OK"), "cliente {n}: {r}");
        assert!(r.ends_with(&format!("ok{n}")), "cliente {n}: {r}");
    }
    let out = server.wait();
    assert!(out.status.success());
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

/// Servidor HTTPS local (Python stdlib + `openssl`): PKI de teste (CA
/// propria + cert de servidor assinado), validada via `RHP_CA_BUNDLE`.
/// Pre-requisitos (como o Docker do PG): `python3` com `ssl` + `openssl`.
struct TlsServer {
    child: Option<std::process::Child>,
    port: u16,
    bundle: std::path::PathBuf,
}

impl TlsServer {
    fn start(name: &str) -> Self {
        use std::io::BufRead;
        let dir = std::env::temp_dir().join(format!("rhp-tls-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir temporario");
        // PKI de teste: CA propria + certificado de servidor assinado
        // (SAN IP:127.0.0.1, sem CA:TRUE — verificacao real de cadeia).
        let ca_key = dir.join("ca.key");
        let ca_crt = dir.join("ca.crt");
        let srv_key = dir.join("srv.key");
        let srv_csr = dir.join("srv.csr");
        let srv_crt = dir.join("srv.crt");
        let cfg = dir.join("openssl.cnf");
        std::fs::write(
            &cfg,
            "[req]\ndistinguished_name = dn\n[dn]\n[v3]\nsubjectAltName = IP:127.0.0.1\nbasicConstraints = CA:FALSE\n",
        )
        .expect("openssl.cnf");
        let openssl = |args: &[&str]| {
            let out = std::process::Command::new("openssl")
                .args(args)
                .output()
                .expect("openssl ausente? instale openssl + python3 com ssl");
            assert!(
                out.status.success(),
                "openssl falhou ({}): {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr)
            );
        };
        openssl(&[
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-keyout",
            &ca_key.to_string_lossy(),
            "-out",
            &ca_crt.to_string_lossy(),
            "-days",
            "2",
            "-nodes",
            "-subj",
            "/CN=teste-rhp-CA",
        ]);
        openssl(&[
            "req",
            "-newkey",
            "rsa:2048",
            "-keyout",
            &srv_key.to_string_lossy(),
            "-out",
            &srv_csr.to_string_lossy(),
            "-nodes",
            "-subj",
            "/CN=127.0.0.1",
            "-config",
            &cfg.to_string_lossy(),
        ]);
        openssl(&[
            "x509",
            "-req",
            "-in",
            &srv_csr.to_string_lossy(),
            "-CA",
            &ca_crt.to_string_lossy(),
            "-CAkey",
            &ca_key.to_string_lossy(),
            "-CAcreateserial",
            "-out",
            &srv_crt.to_string_lossy(),
            "-days",
            "2",
            "-extensions",
            "v3",
            "-extfile",
            &cfg.to_string_lossy(),
        ]);
        let script = dir.join("srv.py");
        std::fs::write(
            &script,
            "import http.server, socketserver, ssl, sys\ncert, key, port = sys.argv[1], sys.argv[2], int(sys.argv[3])\nclass H(http.server.BaseHTTPRequestHandler):\n    def log_message(self, *a): pass\n    def end_ok(self, body):\n        self.send_response(200)\n        self.send_header(\"Content-Length\", str(len(body)))\n        self.end_headers()\n        self.wfile.write(body)\n    def do_GET(self):\n        if self.path == \"/seguro\":\n            self.end_ok(b\"via tls\")\n        elif self.path == \"/redireciona\":\n            p = self.server.server_address[1]\n            self.send_response(302)\n            self.send_header(\"Location\", f\"https://127.0.0.1:{p}/alvo\")\n            self.send_header(\"Content-Length\", \"0\")\n            self.end_headers()\n        elif self.path == \"/alvo\":\n            self.end_ok(b\"alvo tls\")\n        elif self.path == \"/desce\":\n            self.send_response(302)\n            self.send_header(\"Location\", \"http://127.0.0.1:9/plano\")\n            self.send_header(\"Content-Length\", \"0\")\n            self.end_headers()\n        else:\n            self.send_response(404)\n            self.send_header(\"Content-Length\", \"0\")\n            self.end_headers()\n    do_POST = do_GET\nclass S(socketserver.ThreadingMixIn, http.server.HTTPServer):\n    daemon_threads = True\nsrv = S((\"127.0.0.1\", port), H)\nctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)\nctx.load_cert_chain(cert, key)\nsrv.socket = ctx.wrap_socket(srv.socket, server_side=True)\nprint(srv.server_address[1], flush=True)\nsrv.serve_forever()\n",
        )
        .expect("escrever servidor tls");
        let mut child = std::process::Command::new("python3")
            .args([
                script.to_string_lossy().as_ref(),
                &srv_crt.to_string_lossy(),
                &srv_key.to_string_lossy(),
                "0",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("falha ao iniciar servidor tls (python3 + ssl?)");
        let stdout = child.stdout.take().expect("stdout do servidor tls");
        let mut lines = std::io::BufReader::new(stdout).lines();
        let first = lines
            .next()
            .expect("servidor tls fechou stdout")
            .expect("leitura da porta tls");
        let port: u16 = first.trim().parse().expect("porta tls invalida");
        Self {
            child: Some(child),
            port,
            bundle: ca_crt,
        }
    }

    /// Roda `fonte` com `RHP_CA_BUNDLE` apontando para o cert do servidor.
    fn run_rhp(&self, name: &str, source: &str) -> std::process::Output {
        let dir = self
            .bundle
            .parent()
            .expect("bundle tem diretorio pai")
            .to_path_buf();
        let file = dir.join(name);
        std::fs::write(&file, source).expect("escrever programa de teste");
        std::process::Command::new(env!("CARGO_BIN_EXE_rhp"))
            .args(["run", &file.to_string_lossy()])
            .env("RHP_CA_BUNDLE", &self.bundle)
            .output()
            .expect("executar rhp")
    }
}

impl Drop for TlsServer {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[test]
fn rhp_run_http_fetch_https_ok() {
    let server = TlsServer::start("ok");
    let output = server.run_rhp(
        "t.rhp",
        &format!(
            "let r = http_fetch(\"https://127.0.0.1:{}/seguro\", {{}});\necho r[\"status\"];\necho r[\"corpo\"];\n",
            server.port
        ),
    );
    assert!(
        output.status.success(),
        "fetch https falhou: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"200\nvia tls\n");
}

#[test]
fn rhp_run_http_fetch_upgrade_e_recusa_downgrade() {
    // http -> https: redirecionador simples em thread + TLS real.
    let tls = TlsServer::start("up");
    let tls_port = tls.port;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let plain_port = listener.local_addr().expect("addr").port();
    let redirector = std::thread::spawn(move || {
        use std::io::{Read, Write};
        let (mut stream, _) = listener.accept().expect("accept");
        let mut request = vec![0u8; 1024];
        let _ = stream.read(&mut request);
        let response = format!(
            "HTTP/1.1 302 Found\r\nLocation: https://127.0.0.1:{tls_port}/seguro\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        stream.write_all(response.as_bytes()).expect("write");
    });
    let output = tls.run_rhp(
        "t2.rhp",
        &format!("let r = http_fetch(\"http://127.0.0.1:{plain_port}/a\", {{}});\necho r[\"status\"];\necho r[\"corpo\"];\n"),
    );
    redirector.join().expect("redirecionador deve terminar");
    assert!(
        output.status.success(),
        "upgrade falhou: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"200\nvia tls\n");
    // Downgrade https -> http: recusado sem conectar.
    let output = tls.run_rhp(
        "t3.rhp",
        &format!(
            "echo http_fetch(\"https://127.0.0.1:{}/desce\", {{}});\n",
            tls.port
        ),
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("downgrade"),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    // CA errada: verificacao falha.
    let dir = tls
        .bundle
        .parent()
        .expect("bundle tem diretorio pai")
        .to_path_buf();
    let outra = dir.join("outra.crt");
    let out = std::process::Command::new("openssl")
        .args([
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-keyout",
            "/dev/null",
            "-out",
            &outra.to_string_lossy(),
            "-days",
            "1",
            "-nodes",
            "-subj",
            "/CN=outra",
        ])
        .output()
        .expect("openssl");
    assert!(out.status.success());
    let file = dir.join("t4.rhp");
    std::fs::write(
        &file,
        format!(
            "echo http_fetch(\"https://127.0.0.1:{}/seguro\", {{}});\n",
            tls.port
        ),
    )
    .expect("escrever");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_rhp"))
        .args(["run", &file.to_string_lossy()])
        .env("RHP_CA_BUNDLE", &outra)
        .output()
        .expect("executar rhp");
    assert!(!output.status.success(), "CA errada deveria falhar");
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
fn rhp_run_rejects_deep_chains_cleanly() {
    // RHP-AUD-001: cadeias planas profundas falham com E1000 (erro limpo),
    // nunca com abort da pilha nativa.
    for (name, src) in [
        ("soma", format!("echo {};\n", vec!["1"; 2000].join("+"))),
        ("bangs", format!("echo {}true;\n", "!".repeat(2000))),
        ("membros", format!("echo m{};\n", ".x".repeat(2000))),
    ] {
        let output = run_temp_source("deep.rhp", &src);
        assert!(!output.status.success(), "cadeia {name} deveria falhar");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("profundidade maxima"),
            "cadeia {name}: obteve: {stderr}"
        );
    }
}

#[test]
fn rhp_run_reports_both_arg_types() {
    // RHP-AUD: mensagens de aridade de tipo citam os DOIS argumentos.
    let output = run_temp_source("tipos.rhp", "echo split(\"a,b\", 1);\n");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("(string, int)"), "obteve: {stderr}");
    let output = run_temp_source("tipos2.rhp", "echo has([1], \"a\");\n");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("(array, string)"), "obteve: {stderr}");
}

#[test]
fn rhp_run_logical_operators_end_to_end() {
    // RFC 0031: curto-circuito observavel + precedencia + erro em nao-Bool.
    let output = run_temp_source(
        "logica.rhp",
        "fn lado(n) { echo n; return true; }\necho false && lado(\"x\");\necho true || lado(\"y\");\necho true || true && false;\n",
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"false\ntrue\ntrue\n");
    let output = run_temp_source("logica2.rhp", "echo 1 && true;\n");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("condicao deve ser bool"),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn rhp_run_error_shows_call_stack() {
    // Etapa 4: stack trace da mais interna para a mais externa; erro no
    // nivel superior nao imprime pilha.
    let output = run_temp_source(
        "trace.rhp",
        "fn f() {\n  g();\n}\nfn g() {\n  x_indefinida;\n}\nf();\n",
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let pos_g = stderr.find("em g").expect("trace deveria ter `em g`");
    let pos_f = stderr.find("em f").expect("trace deveria ter `em f`");
    assert!(pos_g < pos_f, "ordem interna->externa, obteve: {stderr}");
    let flat = run_temp_source("flat.rhp", "x_indefinida;\n");
    assert!(!flat.status.success());
    let flat_err = String::from_utf8_lossy(&flat.stderr);
    assert!(
        !flat_err.contains("  em "),
        "nivel superior nao tem pilha, obteve: {flat_err}"
    );
}

#[test]
fn rhp_run_trace_prints_ops_to_stderr() {
    // Etapa 4: `rhp run --trace` despeja `ip` + op em stderr; a saida do
    // programa continua intacta em stdout.
    let output = run_temp_source_trace("trace.rhp", "echo 1 + 2;\n");
    assert!(output.status.success());
    assert_eq!(output.stdout, b"3\n");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("[trace]"), "obteve: {stderr}");
    assert!(stderr.contains("Constant"), "obteve: {stderr}");
    assert!(stderr.contains("Halt"), "obteve: {stderr}");
}

#[test]
fn rhp_repl_keeps_state_across_lines() {
    // Etapa 4: `rhp repl` com stdin redirecionado; estado sobrevive,
    // erro de uma linha nao mata a sessao.
    use std::io::Write as _;
    use std::process::Stdio;
    let rhp_bin = env!("CARGO_BIN_EXE_rhp");
    let mut child = Command::new(rhp_bin)
        .arg("repl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("falha ao iniciar rhp repl");
    child
        .stdin
        .as_mut()
        .expect("stdin do repl")
        .write_all(
            b"let x = 40;\nfn soma(a) { return a + 2; }\necho soma(x);\necho x_ruim;\necho x;\n",
        )
        .expect("escrever no stdin do repl");
    let output = child.wait_with_output().expect("aguardar repl");
    assert!(output.status.success(), "repl nao deveria falhar");
    assert_eq!(output.stdout, b"42\n40\n", "stdout={:?}", output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("x_ruim"),
        "erro da linha ruim, obteve: {stderr}"
    );
}

#[test]
fn rhp_repl_async_sees_previous_lines() {
    // Regressao: `async_spawn` no REPL precisa do programa acumulado
    // (senao: "sem programa-fonte").
    use std::io::Write as _;
    use std::process::Stdio;
    let rhp_bin = env!("CARGO_BIN_EXE_rhp");
    let mut child = Command::new(rhp_bin)
        .arg("repl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("falha ao iniciar rhp repl");
    child
        .stdin
        .as_mut()
        .expect("stdin do repl")
        .write_all(b"fn f() { return 7; }\nlet h = async_spawn(\"f\");\necho async_join(h);\n")
        .expect("escrever no stdin do repl");
    let output = child.wait_with_output().expect("aguardar repl");
    assert!(output.status.success(), "repl nao deveria falhar");
    assert_eq!(output.stdout, b"7\n", "stdout={:?}", output.stdout);
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
fn rhp_test_supports_async() {
    // Regressao: `rhp test` define o programa (senao `async_spawn`
    // falha com "sem programa-fonte").
    let dir = std::env::temp_dir().join(format!("rhp-test-async-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("nao foi possivel criar dir temporario");
    std::fs::write(
        dir.join("async_test.rhp"),
        "fn f() { return 7; }\nlet h = async_spawn(\"f\");\nassert(async_join(h) == 7);\n",
    )
    .expect("write");
    let out = Command::new(env!("CARGO_BIN_EXE_rhp"))
        .args(["test", &dir.join("async_test.rhp").to_string_lossy()])
        .output()
        .expect("rhp test async");
    assert!(
        out.status.success(),
        "async em teste deveria passar: {}",
        String::from_utf8_lossy(&out.stderr)
    );
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
        "http_serve",
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
        // Cliente certo para de ler ao ver o teto; EPIPE aqui e esperado.
        let _ = stream.write_all(response.as_bytes());
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

// --- Next Generation (Fases 1-6) -------------------------------------------

#[test]
fn nextgen_heranca_basica_e_override() {
    let output = run_temp_source(
        "ng_heranca.rhp",
        "class Animal {\n  fn novo(n) { this[\"nome\"] = n; }\n  fn falar() { return \"Som\"; }\n}\nclass Cachorro extends Animal {\n  fn falar() { return \"Au au\"; }\n}\nlet c = new Cachorro(\"Rex\");\necho c[\"nome\"];\necho c.falar();\n",
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"Rex\nAu au\n");
}

#[test]
fn nextgen_heranca_rejeita_pai_desconhecido_e_ciclo() {
    let output = run_temp_source(
        "ng_pai.rhp",
        "class B extends A { fn novo() { this[\"x\"] = 1; } }\n",
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("desconhecida"), "obteve: {stderr}");
    let output = run_temp_source(
        "ng_ciclo.rhp",
        "class A extends A { fn novo() { this[\"x\"] = 1; } }\n",
    );
    assert!(!output.status.success());
}

#[test]
fn nextgen_super_chama_pai_com_writeback() {
    let output = run_temp_source(
        "ng_super.rhp",
        "class Base {\n  fn novo() { this[\"n\"] = 1; }\n  fn inc() { this[\"n\"] = this[\"n\"] + 1; return this[\"n\"]; }\n}\nclass Filha extends Base {\n  fn novo() { this[\"n\"] = 10; }\n  fn inc2() { return super.inc() + 100; }\n}\nlet f = new Filha();\necho f.inc2();\necho f[\"n\"];\n",
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"111\n11\n");
}

#[test]
fn nextgen_interface_verificada_e_trait_composta() {
    let output = run_temp_source(
        "ng_iface.rhp",
        "interface Logger { fn log(mensagem); }\nclass ConsoleLogger implements Logger {\n  fn novo() { this[\"x\"] = 0; }\n  fn log(m) { echo m; }\n}\nlet l = new ConsoleLogger();\nl.log(\"ola\");\ntrait T { fn ola() { return \"trait\"; } }\nclass P { use T; fn novo() { this[\"x\"] = 1; } }\nlet p = new P();\necho p.ola();\n",
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"ola\ntrait\n");
    // Interface nao implementada = erro de compilacao.
    let output = run_temp_source(
        "ng_iface_bad.rhp",
        "interface L { fn log(m); }\nclass C implements L {\n  fn novo() { this[\"x\"] = 0; }\n}\n",
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("nao implementa"), "obteve: {stderr}");
}

#[test]
fn nextgen_trait_conflito_exige_override() {
    let output = run_temp_source(
        "ng_trait_conf.rhp",
        "trait A { fn m() { return 1; } }\ntrait B { fn m() { return 2; } }\nclass C { use A, B; fn novo() { this[\"x\"] = 0; } }\n",
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("conflito de trait"), "obteve: {stderr}");
}

#[test]
fn nextgen_excecao_tipada_por_hierarquia() {
    let output = run_temp_source(
        "ng_catch.rhp",
        "class ValidationException {\n  fn novo(m) { this[\"m\"] = m; }\n  fn message() { return this[\"m\"]; }\n}\nclass DatabaseException {\n  fn novo(m) { this[\"m\"] = m; }\n}\ntry {\n  throw new ValidationException(\"email invalido\");\n} catch ValidationException e {\n  echo e.message();\n} catch DatabaseException e {\n  echo \"banco\";\n}\ntry {\n  throw new DatabaseException(\"x\");\n} catch ValidationException e {\n  echo \"errado\";\n} catch DatabaseException e {\n  echo \"banco\";\n}\ntry {\n  echo json_parse(\"{oops}\");\n} catch e {\n  echo \"generico\";\n}\necho \"fim\";\n",
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"email invalido\nbanco\ngenerico\nfim\n");
}

#[test]
fn nextgen_generics_apagados() {
    let output = run_temp_source(
        "ng_gen.rhp",
        "class Caixa<T> {\n  fn novo(v) { this[\"v\"] = v; }\n  fn obter() { return this[\"v\"]; }\n}\nlet c = new Caixa<int>(100);\necho c.obter();\nfn primeiro<T>(items) { return items[0]; }\necho primeiro<int>([7, 8]);\n",
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"100\n7\n");
}

#[test]
fn nextgen_async_await_concorrente() {
    let output = run_temp_source(
        "ng_async.rhp",
        "async fn buscarA() { sleep(0.2); return \"A\"; }\nasync fn buscarB() { return 42; }\nlet a = buscarA();\nlet b = buscarB();\necho await a;\necho await b;\necho \"fim\";\n",
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"A\n42\nfim\n");
}

#[test]
fn nextgen_extensao_nativa_demo() {
    let output = run_temp_source(
        "ng_ext.rhp",
        "import native \"demo\";\necho demo_eco(\"RHP\");\necho demo_soma(20, 22);\necho demo_len([1, 2, 3]);\n",
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"RHP\n42\n3\n");
    let output = run_temp_source("ng_ext_bad.rhp", "import native \"inexistente\";\n");
    assert!(!output.status.success());
}

#[test]
fn nextgen_namespace_e_autoload() {
    // Autoload: namespace -> diretorio relativo ao programa.
    let dir = std::env::temp_dir().join(format!("rhp-test-ns-{}", std::process::id()));
    let appdir = dir.join("App").join("Controllers");
    std::fs::create_dir_all(&appdir).expect("mkdir");
    std::fs::write(
        appdir.join("UsuarioController.rhp"),
        "class App\\Controllers\\UsuarioController {\n  fn novo() { this[\"x\"] = 1; }\n  fn index() { return \"Usuarios\"; }\n}\n",
    )
    .expect("write");
    std::fs::write(
        dir.join("main.rhp"),
        "import App\\Controllers\\UsuarioController;\nlet c = new App\\Controllers\\UsuarioController();\necho c.index();\n",
    )
    .expect("write");
    let output = run_file(&dir.join("main.rhp"));
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"Usuarios\n");
    // Alias.
    std::fs::write(
        dir.join("main2.rhp"),
        "import App\\Controllers\\UsuarioController as Ctrl;\nlet c = new Ctrl();\necho c.index();\n",
    )
    .expect("write");
    let output = run_file(&dir.join("main2.rhp"));
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"Usuarios\n");
}

#[test]
fn nextgen_async_captura_ambiente() {
    // Closure async captura local plano; irmaos nao interferem.
    let output = run_temp_source(
        "ng_async_cap.rhp",
        "fn monta() {\n  let base = 40;\n  let f = async fn(x) { return base + x; };\n  let g = async fn(x) { return base * x; };\n  echo await f(2);\n  echo await g(2);\n}\nmonta();\n",
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"42\n80\n");
}

#[test]
fn nextgen_async_enxerga_funcoes_e_classes() {
    // Tarefa chama outra funcao de topo e usa classe definida no programa.
    let output = run_temp_source(
        "ng_async_code.rhp",
        "fn dobra(x) { return x * 2; }\nclass Ponto {\n  fn novo(x) { this[\"x\"] = x; }\n  fn get() { return this[\"x\"]; }\n}\nasync fn cria(v) {\n  let p = new Ponto(v);\n  return dobra(p.get());\n}\necho await cria(21);\n",
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"42\n");
}

#[test]
fn nextgen_async_instancia_com_classe() {
    // Instancia atravessa COM a classe (item metodos-async): sem corromper.
    let output = run_temp_source(
        "ng_async_inst.rhp",
        "class Ponto {\n  fn novo(x) { this[\"x\"] = x; }\n  fn get() { return this[\"x\"]; }\n}\nasync fn dobra(p) { return p.get() * 2; }\nlet p = new Ponto(21);\necho await dobra(p);\necho p.get();\n",
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"42\n21\n");
}

#[test]
fn nextgen_async_metodo_snapshot() {
    // Metodo async roda sobre snapshot: leitura ok, sem write-back.
    let output = run_temp_source(
        "ng_async_metodo.rhp",
        "class Contador {\n  fn novo() { this[\"n\"] = 10; }\n  async fn soma(x) { return this[\"n\"] + x; }\n  fn get() { return this[\"n\"]; }\n}\nlet c = new Contador();\nlet f = c.soma;\necho await c.soma(5);\necho await f(1);\necho c.get();\n",
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"15\n11\n10\n");
}

#[test]
fn nextgen_async_rejeita_nao_plano_e_reservado() {
    // Funcao como argumento segue rejeitada (nao serializavel).
    let output = run_temp_source(
        "ng_async_func.rhp",
        "async fn usa(f) { return 1; }\nfn g(x) { return x; }\necho await usa(g);\n",
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("nao transporta funcao"), "obteve: {stderr}");
    // Chave reservada do envelope nao passa (evita ambiguidade).
    let output = run_temp_source(
        "ng_async_tag.rhp",
        "async fn f(x) { return x; }\nlet m = {__rhp_instancia__: 1};\necho await f(m);\n",
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("chave reservada"), "obteve: {stderr}");
}

#[test]
fn nextgen_async_aninhado() {
    // Tarefa gera outra tarefa (worker com programa-fonte).
    let output = run_temp_source(
        "ng_async_nest.rhp",
        "async fn interna(x) { return x * 10; }\nasync fn externa(y) {\n  let h = interna(y);\n  return await h;\n}\necho await externa(4);\n",
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"40\n");
}

#[test]
fn nextgen_framework_via_pkg() {
    // RHP Web distribuido como pacote v0 (vendorizado em `pacotes/`).
    let rhp_bin = env!("CARGO_BIN_EXE_rhp");
    let dir = std::env::temp_dir().join(format!("rhp-test-pkgweb-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("projdir");
    let run = |args: &[&str]| {
        std::process::Command::new(rhp_bin)
            .args(args)
            .current_dir(&dir)
            .output()
            .expect("rhp pkg")
    };
    assert!(run(&["init"]).status.success());
    let origem = workspace_root().join("framework");
    let origem = origem.to_string_lossy().into_owned();
    let out = run(&["pkg", "add", "rhp_web", origem.as_str()]);
    assert!(
        out.status.success(),
        "pkg add falhou: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(dir.join("pacotes/rhp_web/web.rhp").is_file());
    assert!(dir.join("rhp.lock").is_file());
    let out = run(&["pkg", "install", "--check"]);
    assert!(
        out.status.success(),
        "pkg install --check falhou: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    std::fs::write(
        dir.join("main.rhp"),
        "import { web_get, web_json, web_despachar } from \"pacotes/rhp_web/web.rhp\";\nweb_get(\"/users/:id\", fn(req) { return web_json({ id: req[\"params\"][\"id\"] }); });\nfn handler(req) { return web_despachar(req); }\nlet req = {metodo: \"GET\", caminho: \"/users/7\", query: {}, cabecalhos: {}, cookies: {}, corpo: \"\"};\necho handler(req)[\"corpo\"];\n",
    )
    .expect("write main");
    let output = run_file(&dir.join("main.rhp"));
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"{\"id\":\"7\"}\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nextgen_generic_bounds() {
    // Bound de classe satisfeito executa; violado falha em compilacao.
    let output = run_temp_source(
        "ng_bound_ok.rhp",
        "class Entidade {\n  fn novo() { this[\"id\"] = 1; }\n  fn id() { return this[\"id\"]; }\n}\nclass Usuario extends Entidade {\n  fn novo() { this[\"id\"] = 2; }\n}\nclass Repo<T: Entidade> {\n  fn novo() { this[\"x\"] = 0; }\n  fn descrever(v: T) { return v.id(); }\n}\nlet r = new Repo<Usuario>();\necho \"ok\";\n",
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"ok\n");
    let output = run_temp_source(
        "ng_bound_bad.rhp",
        "class Entidade {\n  fn novo() { this[\"id\"] = 1; }\n}\nclass Outra {\n  fn novo() { this[\"x\"] = 0; }\n}\nclass Repo<T: Entidade> {\n  fn novo() { this[\"x\"] = 0; }\n}\nlet r = new Repo<Outra>();\n",
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("nao satisfaz `T: Entidade`"),
        "obteve: {stderr}"
    );
    // Bound de interface via funcao generica.
    let output = run_temp_source(
        "ng_bound_iface.rhp",
        "interface Salva { fn salvar(); }\nclass Mem implements Salva {\n  fn novo() { this[\"x\"] = 0; }\n  fn salvar() { echo \"salvo\"; }\n}\nfn guarda<T: Salva>(v) { v.salvar(); }\nguarda<Mem>(new Mem());\n",
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"salvo\n");
    // Parametro de tipo visivel adia a checagem (instanciacao concreta roda).
    let output = run_temp_source(
        "ng_bound_defer.rhp",
        "class Caixa<T> {\n  fn novo(v) { this[\"v\"] = v; }\n  fn obter() { return this[\"v\"]; }\n}\nfn embrulha<T>(x) {\n  let c = new Caixa<T>(x);\n  return c.obter();\n}\necho embrulha(99);\n",
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"99\n");
}

#[test]
fn nextgen_async_metodo_trait_e_super() {
    // Trait com metodo async + super async (snapshot, sem write-back).
    let output = run_temp_source(
        "ng_async_trait_super.rhp",
        "trait Saudacao {\n  async fn ola(nome) { return \"ola \" + nome; }\n}\nclass Pessoa {\n  use Saudacao;\n  fn novo() { this[\"x\"] = 0; }\n}\nlet p = new Pessoa();\necho await p.ola(\"Rex\");\nclass Base {\n  fn novo() { this[\"x\"] = 1; }\n  async fn quem() { return \"base\"; }\n}\nclass Filha extends Base {\n  fn novo() { this[\"x\"] = 2; }\n  async fn quem2() { return await super.quem() + \"!\"; }\n}\nlet d = new Filha();\necho await d.quem2();\necho d[\"x\"];\n",
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"ola Rex\nbase!\n2\n");
}

#[test]
fn nextgen_await_timeout() {
    // Timeout dispara e a tarefa segue juntavel depois.
    let output = run_temp_source(
        "ng_timeout.rhp",
        "async fn lenta() { sleep(1); return \"devagar\"; }\nasync fn rapida() { return \"ligeira\"; }\nlet h = lenta();\ntry {\n  echo await_timeout(h, 0.2);\n} catch e {\n  echo \"timeout ok\";\n}\necho await h;\necho await_timeout(rapida(), 5);\n",
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"timeout ok\ndevagar\nligeira\n");
    // Timeout 0 em tarefa ja concluida devolve na hora (sem erro).
    let output = run_temp_source(
        "ng_timeout_zero.rhp",
        "async fn rapida() { return 42; }\nlet h = rapida();\nsleep(1);\necho await_timeout(h, 0);\n",
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"42\n");
    // Handle invalido e segundos invalidos: erro claro.
    let output = run_temp_source("ng_timeout_bad.rhp", "echo await_timeout(999, 1);\n");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("invalido ou ja consumido"),
        "obteve: {stderr}"
    );
    let output = run_temp_source(
        "ng_timeout_neg.rhp",
        "async fn f() { return 1; }\nlet h = f();\necho await_timeout(h, -1);\n",
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("segundos >= 0"), "obteve: {stderr}");
}

#[test]
fn nextgen_async_cancel() {
    // Cancelamento cooperativo: a tarefa sonda e desiste.
    let output = run_temp_source(
        "ng_cancel.rhp",
        "async fn trabalho() {\n  let i = 0;\n  while i < 100000000 {\n    if async_cancelled() { return \"cancelado\"; }\n    i = i + 1;\n  }\n  return \"ok\";\n}\nlet h = trabalho();\nasync_cancel(h);\necho await h;\n",
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"cancelado\n");
    // Fora de tarefa, nunca cancelado; handle invalido = erro.
    let output = run_temp_source(
        "ng_cancel_main.rhp",
        "echo async_cancelled();\nasync_cancel(999);\n",
    );
    assert!(!output.status.success());
    let out = run_temp_source("ng_cancelled_main.rhp", "echo async_cancelled();\n");
    assert!(out.status.success());
    assert_eq!(out.stdout, b"false\n");
}

#[test]
fn nextgen_await_all_race() {
    // all: junta em ordem; race: primeiro pronto vence.
    let output = run_temp_source(
        "ng_all_race.rhp",
        "async fn a() { sleep(0.3); return \"A\"; }\nasync fn b() { return \"B\"; }\nasync fn c() { sleep(0.1); return \"C\"; }\necho await_all([a(), b(), c()]);\necho await_race([a(), b(), c()]);\n",
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"[A, B, C]\nB\n");
    // all com falha: propaga (tarefa que falhou e consumida, resto segue).
    let output = run_temp_source(
        "ng_all_fail.rhp",
        "async fn boom() { throw \"x\"; }\ntry {\n  echo await_all([boom()]);\n} catch e {\n  echo \"all falhou ok\";\n}\n",
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"all falhou ok\n");
    // race vazio e nao-array: erro claro.
    let output = run_temp_source("ng_race_empty.rhp", "echo await_race([]);\n");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ao menos 1 tarefa"), "obteve: {stderr}");
}

#[test]
fn nextgen_framework_trie_test_suite() {
    // `rhp test framework/`: diferencial trie-vs-linear + comportamento.
    let out = Command::new(env!("CARGO_BIN_EXE_rhp"))
        .args([
            "test",
            &workspace_root().join("framework").to_string_lossy(),
        ])
        .output()
        .expect("rhp test framework");
    assert!(
        out.status.success(),
        "web_trie_test deveria passar: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
