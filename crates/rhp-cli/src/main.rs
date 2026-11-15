//! Binario `rhp` — CLI oficial (Fase 5: +`fmt`).
//!
//! Uso: `rhp run <arquivo.rhp>` | `rhp fmt [--check] <arquivo.rhp>`

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rhp_parser::{Program, Stmt};

const USAGE: &str = "uso: rhp run [--max-steps N] <arquivo.rhp> [args] | rhp fmt [--check] <arquivo> | rhp check [--json] <arquivo> | rhp test [dir|arquivo] | rhp doctor";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match args.as_slice() {
        // `--max-steps` antes do `run` generico: senao `--max-steps` seria
        // interpretado como caminho do programa.
        [_, cmd, flag, value, path, rest @ ..] if cmd == "run" && flag == "--max-steps" => {
            match value.parse::<u64>() {
                Ok(max_steps) => {
                    match run_file_limited(path, rest, &mut std::io::stdout(), Some(max_steps)) {
                        Ok(()) => ExitCode::SUCCESS,
                        Err(message) => {
                            let _ = writeln!(std::io::stderr(), "rhp: {message}");
                            ExitCode::from(1)
                        }
                    }
                }
                Err(_) => {
                    let _ = writeln!(
                        std::io::stderr(),
                        "rhp: valor invalido para --max-steps: `{value}`"
                    );
                    ExitCode::from(2)
                }
            }
        }
        [_, cmd, path, rest @ ..] if cmd == "run" => {
            match run_file_with_args(path, rest, &mut std::io::stdout()) {
                Ok(()) => ExitCode::SUCCESS,
                Err(message) => {
                    let _ = writeln!(std::io::stderr(), "rhp: {message}");
                    ExitCode::from(1)
                }
            }
        }
        [_, cmd, path] if cmd == "fmt" => match fmt_file(path, false) {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                let _ = writeln!(std::io::stderr(), "rhp: {message}");
                ExitCode::from(1)
            }
        },
        [_, cmd, flag, path] if cmd == "fmt" && flag == "--check" => match fmt_file(path, true) {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                let _ = writeln!(std::io::stderr(), "rhp: {message}");
                ExitCode::from(1)
            }
        },
        [_, cmd, path] if cmd == "check" => match check_file(path, false) {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                let _ = writeln!(std::io::stderr(), "{message}");
                ExitCode::from(1)
            }
        },
        [_, cmd, flag, path] if cmd == "check" && flag == "--json" => {
            match check_file(path, true) {
                Ok(()) => ExitCode::SUCCESS,
                Err(message) => {
                    println!("{message}");
                    ExitCode::from(1)
                }
            }
        }
        [_, flag] if flag == "--help" || flag == "-h" => {
            println!("RHP (linguagem experimental)\n\n{USAGE}");
            ExitCode::SUCCESS
        }
        [_, cmd] if cmd == "test" => match run_tests(Path::new(".")) {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                let _ = writeln!(std::io::stderr(), "{message}");
                ExitCode::from(1)
            }
        },
        [_, cmd, path] if cmd == "test" => match run_tests(Path::new(path)) {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                let _ = writeln!(std::io::stderr(), "{message}");
                ExitCode::from(1)
            }
        },
        [_, cmd] if cmd == "doctor" => match run_doctor() {
            Ok(report) => {
                println!("{report}");
                ExitCode::SUCCESS
            }
            Err(message) => {
                let _ = writeln!(std::io::stderr(), "{message}");
                ExitCode::from(1)
            }
        },
        [_, flag] if flag == "--version" || flag == "-V" => {
            println!("rhp {}", env!("CARGO_PKG_VERSION"));
            ExitCode::from(0)
        }
        _ => {
            let _ = writeln!(std::io::stderr(), "rhp: {USAGE}");
            ExitCode::from(2)
        }
    }
}

/// Formata o arquivo no estilo canonico (`--check` so verifica).
fn fmt_file(path: &str, check: bool) -> Result<(), String> {
    let source =
        std::fs::read_to_string(path).map_err(|e| format!("nao foi possivel ler `{path}`: {e}"))?;
    let tokens = rhp_lexer::lex(&source).map_err(|e| {
        let diag = rhp_diagnostic::lex_diagnostic(path, &e);
        rhp_diagnostic::render(&diag, Some(&source))
    })?;
    let program = rhp_parser::parse(&tokens).map_err(|e| {
        let diag = rhp_diagnostic::parse_diagnostic(path, &e);
        rhp_diagnostic::render(&diag, Some(&source))
    })?;
    let formatted = rhp_fmt::format(&program);
    if check {
        if formatted == source {
            Ok(())
        } else {
            Err(format!(
                "{path}: fora do formato canonico (rode `rhp fmt {path}`)"
            ))
        }
    } else {
        std::fs::write(path, formatted)
            .map_err(|e| format!("nao foi possivel escrever `{path}`: {e}"))?;
        Ok(())
    }
}

/// Erro de carregamento: diagnostico estruturado (lex/parse) ou texto.
/// `Box` no `Rich` para nao estourar o limite de tamanho do `Err`.
enum LoadError {
    Rich(Box<RichError>),
    Plain(String),
}

struct RichError {
    human: String,
    diag: rhp_diagnostic::Diagnostic,
}

impl LoadError {
    fn rich(human: String, diag: rhp_diagnostic::Diagnostic) -> Self {
        Self::Rich(Box::new(RichError { human, diag }))
    }

    fn human(&self) -> String {
        match self {
            Self::Rich(error) => error.human.clone(),
            Self::Plain(message) => message.clone(),
        }
    }

    fn json(&self, path: &str) -> String {
        match self {
            Self::Rich(error) => diagnostic_json(path, false, Some(&error.diag)),
            Self::Plain(message) => {
                let (code, help) = rhp_diagnostic::runtime_code(message);
                let diag = rhp_diagnostic::Diagnostic::new(code, path, span_1_1(), message.clone());
                let diag = match help {
                    Some(help) => diag.with_help(help),
                    None => diag,
                };
                diagnostic_json(path, false, Some(&diag))
            }
        }
    }
}

fn span_1_1() -> rhp_diagnostic::Span {
    rhp_diagnostic::Span {
        line: 1,
        col: 1,
        len: 0,
    }
}

/// Escapa string para JSON (sem dependencias).
fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn diagnostic_json(path: &str, ok: bool, diag: Option<&rhp_diagnostic::Diagnostic>) -> String {
    match diag {
        Some(diag) => {
            let help_json = diag
                .help
                .as_deref()
                .map_or_else(|| "null".to_string(), json_string);
            format!(
                "{{\"ok\":{ok},\"file\":{path},\"diagnostics\":[{{\"code\":{code},\"line\":{line},\"col\":{col},\"len\":{len},\"message\":{message},\"help\":{help_json}}}]}}",
                path = json_string(path),
                code = json_string(diag.code),
                line = diag.span.line,
                col = diag.span.col,
                len = diag.span.len,
                message = json_string(&diag.message),
            )
        }
        None => format!(
            "{{\"ok\":{ok},\"file\":{},\"diagnostics\":[]}}",
            json_string(path)
        ),
    }
}

/// Checa um programa (lex + imports + compilacao), sem executar.
///
/// `json = true` imprime o relatorio em JSON (para ferramentas/LSP).
fn check_file(path: &str, json: bool) -> Result<(), String> {
    match load_program(path) {
        Ok(program) => match rhp_bytecode::compile(&program) {
            Ok(_) => {
                if json {
                    println!("{}", diagnostic_json(path, true, None));
                } else {
                    println!("{path}: ok");
                }
                Ok(())
            }
            Err(e) => {
                let (code, help) = rhp_diagnostic::compile_code(&e.to_string());
                let diag = rhp_diagnostic::code_diagnostic(
                    code,
                    help,
                    path,
                    &format!("erro de compilacao: {e}"),
                );
                Err(if json {
                    diagnostic_json(path, false, Some(&diag))
                } else {
                    rhp_diagnostic::render(&diag, None)
                })
            }
        },
        Err(err) => Err(if json { err.json(path) } else { err.human() }),
    }
}

/// Executa todos os testes `*_test.rhp` de um arquivo ou diretorio.
///
/// Convenção: arquivos terminados em `_test.rhp`; usam `assert` (falha =
/// erro de runtime). Relata por arquivo + resumo; exit != 0 se falhar.
fn run_tests(target: &Path) -> Result<(), String> {
    let mut files: Vec<PathBuf> = Vec::new();
    if target.is_file() {
        files.push(target.to_path_buf());
    } else if target.is_dir() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(target)
            .map_err(|e| format!("nao foi possivel listar `{}`: {e}", target.display()))?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file()
                    && path.extension().and_then(|ext| ext.to_str()) == Some("rhp")
                    && path
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .is_some_and(|stem| stem.ends_with("_test"))
            })
            .collect();
        entries.sort();
        files = entries;
    } else {
        return Err(format!(
            "caminho de teste inexistente: `{}`",
            target.display()
        ));
    }
    if files.is_empty() {
        return Err(format!(
            "nenhum teste `*_test.rhp` em `{}`",
            target.display()
        ));
    }
    let mut failures = 0usize;
    for file in &files {
        let mut sink = std::io::sink();
        match load_program(&file.to_string_lossy()).and_then(|program| {
            let chunk = rhp_bytecode::compile(&program)
                .map_err(|e| LoadError::Plain(format!("erro de compilacao: {e}")))?;
            let mut vm = rhp_vm::Vm::new();
            vm.run(&chunk, &mut sink)
                .map_err(|e| LoadError::Plain(format!("erro de execucao: {e}")))
        }) {
            Ok(()) => println!("ok {}", file.display()),
            Err(err) => {
                failures += 1;
                let rendered = err.human();
                let first = rendered.lines().next().unwrap_or("erro");
                println!("FALHOU {}: {first}", file.display());
            }
        }
    }
    if failures == 0 {
        println!("{} teste(s), 0 falhas", files.len());
        Ok(())
    } else {
        Err(format!("{} teste(s), {failures} falha(s)", files.len()))
    }
}

/// Diagnostico do ambiente: versao, toolchain e autoteste em memoria.
fn tool_version(cmd: &str) -> String {
    std::process::Command::new(cmd)
        .arg("--version")
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.lines().next().unwrap_or("?").to_string())
        .unwrap_or_else(|| "nao encontrado no PATH".to_string())
}

fn run_doctor() -> Result<String, String> {
    let mut lines = Vec::new();
    lines.push(format!("rhp {}", env!("CARGO_PKG_VERSION")));
    lines.push(format!("rustc: {}", tool_version("rustc")));
    lines.push(format!("cargo: {}", tool_version("cargo")));
    // Autoteste em memoria: lex + parse + compile + execucao.
    match rhp_parser::parse_source("echo 6 * 7;") {
        Err(e) => return Err(format!("autoteste falhou no parse: {e}")),
        Ok(program) => match rhp_bytecode::compile(&program) {
            Err(e) => return Err(format!("autoteste falhou na compilacao: {e}")),
            Ok(chunk) => {
                let mut vm = rhp_vm::Vm::new();
                let mut out = Vec::new();
                match vm.run(&chunk, &mut out) {
                    Err(e) => return Err(format!("autoteste falhou na execucao: {e}")),
                    Ok(()) if out == b"42\n" => {
                        lines.push("autoteste: ok (6 * 7 = 42)".to_string())
                    }
                    Ok(()) => {
                        return Err(format!(
                            "autoteste com saida inesperada: {}",
                            String::from_utf8_lossy(&out)
                        ));
                    }
                }
            }
        },
    }
    Ok(lines.join("\n"))
}

/// Executa um arquivo `.rhp`, repassando argumentos extras para `argv()`.
fn run_file_with_args(path: &str, args: &[String], output: &mut dyn Write) -> Result<(), String> {
    run_file_limited(path, args, output, None)
}

/// Executa um arquivo `.rhp` com teto opcional de instrucoes.
fn run_file_limited(
    path: &str,
    args: &[String],
    output: &mut dyn Write,
    max_steps: Option<u64>,
) -> Result<(), String> {
    let program = load_program(path).map_err(|e| e.human())?;
    run_program(
        &program,
        path,
        args,
        output,
        rhp_vm::VmLimits {
            max_steps,
            ..Default::default()
        },
    )
}

/// Pipeline programa -> bytecode -> VM (erros com codigo estavel).
fn run_program(
    program: &Program,
    origin: &str,
    args: &[String],
    output: &mut dyn Write,
    limits: rhp_vm::VmLimits,
) -> Result<(), String> {
    let chunk = rhp_bytecode::compile(program).map_err(|e| {
        let (code, help) = rhp_diagnostic::compile_code(&e.to_string());
        with_code(code, help, &format!("{origin}: erro de compilacao: {e}"))
    })?;
    let mut vm = rhp_vm::Vm::new();
    vm.set_limits(limits);
    vm.set_argv(args.to_vec());
    vm.run(&chunk, output).map_err(|e| {
        let (code, help) = rhp_diagnostic::runtime_code(&e.to_string());
        with_code(code, help, &format!("{origin}: erro de execucao: {e}"))
    })
}

/// Prefixa `Error[CODIGO]` + ajuda a uma mensagem de erro.
fn with_code(code: &str, help: Option<&str>, message: &str) -> String {
    match help {
        Some(help) => format!("Error[{code}]: {message}\nhelp: {help}"),
        None => format!("Error[{code}]: {message}"),
    }
}

/// Lex + parse com diagnosticos estruturados (snippet da fonte).
fn parse_with_diagnostics(origin: &str, source: &str) -> Result<Program, LoadError> {
    let tokens = rhp_lexer::lex(source).map_err(|e| {
        let diag = rhp_diagnostic::lex_diagnostic(origin, &e);
        LoadError::rich(rhp_diagnostic::render(&diag, Some(source)), diag)
    })?;
    rhp_parser::parse(&tokens).map_err(|e| {
        let diag = rhp_diagnostic::parse_diagnostic(origin, &e);
        LoadError::rich(rhp_diagnostic::render(&diag, Some(source)), diag)
    })
}

/// Pipeline fonte (sem arquivos) -> tokens -> parser -> bytecode -> VM.
///
/// Auxiliar de testes (a CLI sempre passa pelo carregador de modulos).
#[cfg(test)]
fn run_source(source: &str, origin: &str, output: &mut dyn Write) -> Result<(), String> {
    let program = rhp_parser::parse_source(source).map_err(|e| format!("{origin}:{e}"))?;
    run_program(&program, origin, &[], output, rhp_vm::VmLimits::default())
}

/// Teto de tamanho de fonte por arquivo (protecao contra OOM no loader).
pub const MAX_SOURCE_BYTES: u64 = 8 * 1024 * 1024;

/// Carrega o programa de entrada com `import`s resolvidos.
///
/// Semantica: busca em profundidade a partir do arquivo de entrada; cada
/// modulo executa uma vez, antes do arquivo que o importa; caminhos sao
/// relativos ao arquivo que contem o `import` e confinados a arvore do
/// arquivo de entrada; importar o mesmo arquivo varias vezes o inclui uma
/// unica vez; ciclo, arquivo fora da arvore, arquivo grande demais e
/// `import` fora do nivel superior sao erros.
fn load_program(entry: &str) -> Result<Program, LoadError> {
    let entry_path = Path::new(entry);
    let canonical_entry = std::fs::canonicalize(entry_path)
        .map_err(|e| LoadError::Plain(format!("modulo nao encontrado `{entry}`: {e}")))?;
    let root: PathBuf = canonical_entry
        .parent()
        .map_or_else(|| PathBuf::from("."), std::path::Path::to_path_buf);
    let mut visiting: Vec<PathBuf> = Vec::new();
    let mut visited: HashSet<PathBuf> = HashSet::new();
    let mut statements: Vec<Stmt> = Vec::new();
    load_file(
        entry_path,
        &root,
        &mut visiting,
        &mut visited,
        &mut statements,
    )?;
    Ok(Program { statements })
}

fn load_file(
    path: &Path,
    root: &Path,
    visiting: &mut Vec<PathBuf>,
    visited: &mut HashSet<PathBuf>,
    out: &mut Vec<Stmt>,
) -> Result<(), LoadError> {
    let shown = path.display().to_string();
    let canonical = std::fs::canonicalize(path)
        .map_err(|e| LoadError::Plain(format!("modulo nao encontrado `{shown}`: {e}")))?;
    if !canonical.starts_with(root) {
        return Err(LoadError::Plain(format!(
            "import fora da arvore do programa: `{shown}` (permitido apenas sob `{}`)",
            root.display()
        )));
    }
    if visited.contains(&canonical) {
        return Ok(());
    }
    if visiting.contains(&canonical) {
        let mut chain: Vec<String> = visiting.iter().map(|p| p.display().to_string()).collect();
        chain.push(canonical.display().to_string());
        return Err(LoadError::Plain(format!(
            "importacao circular: {}",
            chain.join(" -> ")
        )));
    }
    let size = std::fs::metadata(&canonical)
        .map_err(|e| LoadError::Plain(format!("nao foi possivel ler `{shown}`: {e}")))?
        .len();
    if size > MAX_SOURCE_BYTES {
        return Err(LoadError::Plain(format!(
            "arquivo muito grande `{shown}` ({size} bytes, limite {MAX_SOURCE_BYTES})"
        )));
    }
    visiting.push(canonical.clone());
    let source = std::fs::read_to_string(&canonical)
        .map_err(|e| LoadError::Plain(format!("nao foi possivel ler `{shown}`: {e}")))?;
    let origin = canonical.display().to_string();
    let program = parse_with_diagnostics(&origin, &source)?;
    let base_dir: PathBuf = canonical
        .parent()
        .map_or_else(|| PathBuf::from("."), std::path::Path::to_path_buf);
    for import in collect_imports(&program.statements)? {
        load_file(&base_dir.join(import), root, visiting, visited, out)?;
    }
    out.extend(
        program
            .statements
            .into_iter()
            .filter(|stmt| !matches!(stmt, Stmt::Import(_))),
    );
    visiting.pop();
    visited.insert(canonical);
    Ok(())
}

/// Coleta os `import`s do nivel superior; `import` aninhado e erro.
fn collect_imports(statements: &[Stmt]) -> Result<Vec<String>, LoadError> {
    let mut imports = Vec::new();
    for stmt in statements {
        if let Stmt::Import(path) = stmt {
            imports.push(path.clone());
        }
    }
    check_no_nested_imports(statements)?;
    Ok(imports)
}

/// Rejeita `import` dentro de blocos/funcoes (a semantica seria icar o
/// modulo para o topo, surpreendente e nao documentada).
fn check_no_nested_imports(statements: &[Stmt]) -> Result<(), LoadError> {
    for stmt in statements {
        match stmt {
            Stmt::Import(_) => {}
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                check_block_imports(then_branch, "em `if`")?;
                if let Some(else_branch) = else_branch {
                    check_block_imports(else_branch, "em `else`")?;
                }
            }
            Stmt::While { body, .. } => check_block_imports(body, "em `while`")?,
            Stmt::FnDef { body, name, .. } => {
                check_block_imports(body, &format!("na funcao `{name}`"))?;
            }
            Stmt::Block(body) => check_block_imports(body, "em bloco")?,
            _ => {}
        }
    }
    Ok(())
}

fn check_block_imports(body: &[Stmt], context: &str) -> Result<(), LoadError> {
    for stmt in body {
        if let Stmt::Import(path) = stmt {
            return Err(LoadError::Plain(format!(
                "import {context} nao suportado (`{path}`): use `import` apenas no nivel superior do arquivo"
            )));
        }
    }
    check_no_nested_imports(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_temp_module(dir: &Path, name: &str, content: &str) -> PathBuf {
        let file = dir.join(name);
        std::fs::write(&file, content).expect("nao foi possivel escrever modulo temporario");
        file
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rhp-test-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("nao foi possivel criar dir temporario");
        dir
    }

    #[test]
    fn runs_hello_source_to_buffer() {
        let mut buf: Vec<u8> = Vec::new();
        run_source("echo \"Hello from RHP!\";", "<teste>", &mut buf).expect("deve passar");
        assert_eq!(buf, b"Hello from RHP!\n");
    }

    #[test]
    fn reports_missing_semicolon_with_position() {
        let mut buf: Vec<u8> = Vec::new();
        let err = run_source("echo \"x\"", "<teste>", &mut buf).expect_err("deve falhar");
        assert!(err.contains("esperado `;`"));
    }

    #[test]
    fn reports_unterminated_string() {
        let mut buf: Vec<u8> = Vec::new();
        let err = run_source("echo \"abc", "<teste>", &mut buf).expect_err("deve falhar");
        assert!(err.contains("string nao terminada"));
    }

    #[test]
    fn reports_missing_file() {
        let mut buf: Vec<u8> = Vec::new();
        let err = run_file_with_args("/caminho/que/nao/existe-12345.rhp", &[], &mut buf)
            .expect_err("deve falhar");
        assert!(err.contains("modulo nao encontrado"));
    }

    #[test]
    fn rejects_nested_import() {
        let dir = temp_dir("aninhado");
        write_temp_module(&dir, "lib.rhp", "let x = 1;\n");
        let main = write_temp_module(&dir, "main.rhp", "fn f() {\n  import \"lib.rhp\";\n}\n");
        let mut buf: Vec<u8> = Vec::new();
        let err =
            run_file_with_args(&main.to_string_lossy(), &[], &mut buf).expect_err("deve falhar");
        assert!(err.contains("nivel superior"), "obteve: {err}");
    }

    #[test]
    fn rejects_import_outside_entry_tree() {
        let dir = temp_dir("jaula");
        let sub = dir.join("sub");
        std::fs::create_dir_all(&sub).expect("nao foi possivel criar subdir");
        write_temp_module(&dir, "fora.rhp", "let smuggled = 1;\n");
        let main = write_temp_module(&sub, "main.rhp", "import \"../fora.rhp\";\n");
        let mut buf: Vec<u8> = Vec::new();
        let err =
            run_file_with_args(&main.to_string_lossy(), &[], &mut buf).expect_err("deve falhar");
        assert!(err.contains("fora da arvore"), "obteve: {err}");
    }

    #[test]
    fn rejects_oversized_source() {
        let dir = temp_dir("grande");
        let big = dir.join("big.rhp");
        let content = "echo 1;\n".repeat(2 * 1024 * 1024);
        std::fs::write(&big, content).expect("nao foi possivel escrever big.rhp");
        let mut buf: Vec<u8> = Vec::new();
        let err =
            run_file_with_args(&big.to_string_lossy(), &[], &mut buf).expect_err("deve falhar");
        assert!(err.contains("muito grande"), "obteve: {err}");
    }

    #[test]
    fn respects_step_limit_via_loader() {
        let dir = temp_dir("limite");
        let main = write_temp_module(&dir, "loop.rhp", "while true {\n}\n");
        let mut buf: Vec<u8> = Vec::new();
        let err = run_file_limited(&main.to_string_lossy(), &[], &mut buf, Some(500))
            .expect_err("deve falhar");
        assert!(err.contains("limite de instrucoes"), "obteve: {err}");
    }

    #[test]
    fn loads_modules_once_in_dependency_order() {
        let dir = temp_dir("modulos");
        write_temp_module(&dir, "lib.rhp", "let base = 40;\n");
        let main = write_temp_module(
            &dir,
            "main.rhp",
            "import \"lib.rhp\";\nimport \"lib.rhp\";\necho base + 2;\n",
        );
        let mut buf: Vec<u8> = Vec::new();
        run_file_with_args(&main.to_string_lossy(), &[], &mut buf).expect("deve passar");
        // Import duplicado inclui uma vez: sem erro de `ja declarada`.
        assert_eq!(buf, b"42\n");
    }

    #[test]
    fn reports_circular_import() {
        let dir = temp_dir("ciclo");
        write_temp_module(&dir, "a.rhp", "import \"b.rhp\";\n");
        let b = write_temp_module(&dir, "b.rhp", "import \"a.rhp\";\n");
        let mut buf: Vec<u8> = Vec::new();
        let err = run_file_with_args(&b.to_string_lossy(), &[], &mut buf).expect_err("deve falhar");
        assert!(err.contains("importacao circular"));
    }
}
