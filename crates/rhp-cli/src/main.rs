//! Binario `rhp` — CLI oficial (Fase 5: +`fmt`).
//!
//! Uso: `rhp run <arquivo.rhp>` | `rhp fmt [--check] <arquivo.rhp>`

use std::collections::HashMap;
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use rhp_parser::{AssignTarget, Expr, ImportName, Program, Stmt, StmtNode};

mod lsp;
mod pkg;

const USAGE: &str = "uso: rhp run [--max-steps N] [--max-heap-bytes N] [--trace] <arquivo.rhp> [args] | rhp repl | rhp lsp | rhp fmt [--check] <arquivo> | rhp check [--json] <arquivo> | rhp test [dir|arquivo] | rhp doctor | rhp init [dir] | rhp pkg add <nome> <caminho> | rhp pkg install [dir] [--check] | rhp build <arquivo.rhp> [--check]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match args.as_slice() {
        // `--max-steps` antes do `run` generico: senao `--max-steps` seria
        // interpretado como caminho do programa.
        [_, cmd, flag, value, path, rest @ ..] if cmd == "run" && flag == "--max-steps" => {
            match value.parse::<u64>() {
                Ok(max_steps) => {
                    match run_file_limited(
                        path,
                        rest,
                        &mut std::io::stdout(),
                        rhp_vm::VmLimits {
                            max_steps: Some(max_steps),
                            ..Default::default()
                        },
                        false,
                    ) {
                        Ok(()) => ExitCode::SUCCESS,
                        Err(Fail::Exit(code)) => ExitCode::from(code as u8),
                        Err(Fail::Msg(message)) => {
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
        // `--max-heap-bytes` (RFC 0015): teto de heap opt-in; `0` e erro
        // de uso (teto zero rejeitaria ate programa vazio).
        [_, cmd, flag, value, path, rest @ ..] if cmd == "run" && flag == "--max-heap-bytes" => {
            match value.parse::<u64>() {
                Ok(max_heap_bytes) if max_heap_bytes > 0 => {
                    match run_file_limited(
                        path,
                        rest,
                        &mut std::io::stdout(),
                        rhp_vm::VmLimits {
                            max_heap_bytes: Some(max_heap_bytes),
                            ..Default::default()
                        },
                        false,
                    ) {
                        Ok(()) => ExitCode::SUCCESS,
                        Err(Fail::Exit(code)) => ExitCode::from(code as u8),
                        Err(Fail::Msg(message)) => {
                            let _ = writeln!(std::io::stderr(), "rhp: {message}");
                            ExitCode::from(1)
                        }
                    }
                }
                _ => {
                    let _ = writeln!(
                        std::io::stderr(),
                        "rhp: valor invalido para --max-heap-bytes: `{value}` (use inteiro > 0)"
                    );
                    ExitCode::from(2)
                }
            }
        }
        // `--trace` (Etapa 4, DevX): traco de execucao em stderr (`ip` +
        // op a cada passo). Exclusivo com os tetos (mesma limitacao do
        // parser da CLI para `--max-steps`).
        [_, cmd, flag, path, rest @ ..] if cmd == "run" && flag == "--trace" => {
            match run_file_limited(
                path,
                rest,
                &mut std::io::stdout(),
                rhp_vm::VmLimits::default(),
                true,
            ) {
                Ok(()) => ExitCode::SUCCESS,
                Err(Fail::Exit(code)) => ExitCode::from(code as u8),
                Err(Fail::Msg(message)) => {
                    let _ = writeln!(std::io::stderr(), "rhp: {message}");
                    ExitCode::from(1)
                }
            }
        }
        [_, cmd, path, rest @ ..] if cmd == "run" => {
            match run_file_with_args(path, rest, &mut std::io::stdout()) {
                Ok(()) => ExitCode::SUCCESS,
                Err(Fail::Exit(code)) => ExitCode::from(code as u8),
                Err(Fail::Msg(message)) => {
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
        [_, cmd] if cmd == "repl" => match run_repl() {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                let _ = writeln!(std::io::stderr(), "rhp: {message}");
                ExitCode::from(1)
            }
        },
        [_, cmd] if cmd == "lsp" => {
            let stdin = std::io::stdin();
            let input = std::io::BufReader::new(stdin.lock());
            let output = std::io::stdout();
            match lsp::run(input, output) {
                Ok(()) => ExitCode::SUCCESS,
                Err(message) => {
                    let _ = writeln!(std::io::stderr(), "rhp: {message}");
                    ExitCode::from(1)
                }
            }
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
        [_, cmd] if cmd == "init" => match pkg::cmd_init(Path::new(".")) {
            Ok(message) => {
                println!("{message}");
                ExitCode::SUCCESS
            }
            Err(message) => {
                let _ = writeln!(std::io::stderr(), "rhp: {message}");
                ExitCode::from(1)
            }
        },
        [_, cmd, dir] if cmd == "init" => match pkg::cmd_init(Path::new(dir)) {
            Ok(message) => {
                println!("{message}");
                ExitCode::SUCCESS
            }
            Err(message) => {
                let _ = writeln!(std::io::stderr(), "rhp: {message}");
                ExitCode::from(1)
            }
        },
        [_, cmd, sub, name, origem] if cmd == "pkg" && sub == "add" => {
            match pkg::cmd_add(Path::new("."), name, origem) {
                Ok(message) => {
                    println!("{message}");
                    ExitCode::SUCCESS
                }
                Err(message) => {
                    let _ = writeln!(std::io::stderr(), "rhp: {message}");
                    ExitCode::from(1)
                }
            }
        }
        [_, cmd, sub] if cmd == "pkg" && sub == "install" => {
            match pkg::cmd_install(Path::new("."), false) {
                Ok(message) => {
                    println!("{message}");
                    ExitCode::SUCCESS
                }
                Err(message) => {
                    let _ = writeln!(std::io::stderr(), "rhp: {message}");
                    ExitCode::from(1)
                }
            }
        }
        [_, cmd, sub, flag] if cmd == "pkg" && sub == "install" && flag == "--check" => {
            match pkg::cmd_install(Path::new("."), true) {
                Ok(message) => {
                    println!("{message}");
                    ExitCode::SUCCESS
                }
                Err(message) => {
                    let _ = writeln!(std::io::stderr(), "rhp: {message}");
                    ExitCode::from(1)
                }
            }
        }
        [_, cmd, sub, dir, flag] if cmd == "pkg" && sub == "install" && flag == "--check" => {
            match pkg::cmd_install(Path::new(dir), true) {
                Ok(message) => {
                    println!("{message}");
                    ExitCode::SUCCESS
                }
                Err(message) => {
                    let _ = writeln!(std::io::stderr(), "rhp: {message}");
                    ExitCode::from(1)
                }
            }
        }
        [_, cmd, sub, dir] if cmd == "pkg" && sub == "install" => {
            match pkg::cmd_install(Path::new(dir), false) {
                Ok(message) => {
                    println!("{message}");
                    ExitCode::SUCCESS
                }
                Err(message) => {
                    let _ = writeln!(std::io::stderr(), "rhp: {message}");
                    ExitCode::from(1)
                }
            }
        }
        [_, cmd, path] if cmd == "build" => match build_bundle(path, None) {
            Ok(out) => {
                println!("{out}");
                ExitCode::SUCCESS
            }
            Err(message) => {
                let _ = writeln!(std::io::stderr(), "rhp: {message}");
                ExitCode::from(1)
            }
        },
        [_, cmd, path, out] if cmd == "build" => match build_bundle(path, Some(out)) {
            Ok(message) => {
                println!("{message}");
                ExitCode::SUCCESS
            }
            Err(message) => {
                let _ = writeln!(std::io::stderr(), "rhp: {message}");
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

/// `rhp build <arquivo> [saida]`: resolve imports, funde num programa so
/// (via linker) e escreve formatado — bundle autocontido para deploy
/// (sem `import`s; executa igual ao original). Padrao:
/// `<base>.bundle.rhp` ao lado da entrada.
fn build_bundle(entry: &str, out: Option<&str>) -> Result<String, String> {
    let program = load_program(entry).map_err(|e| e.human())?;
    // Validacao completa antes de escrever (mesmos gates do `run`).
    rhp_bytecode::compile(&program).map_err(|e| {
        let (code, help) = rhp_diagnostic::compile_code(&e.to_string());
        with_code(code, help, &format!("{entry}: erro de compilacao: {e}"))
    })?;
    let bundled = format!(
        "// bundle gerado por `rhp build {entry}` — nao edite\n{}",
        rhp_fmt::format(&program)
    );
    let dest = match out {
        Some(path) => path.to_string(),
        None => {
            let stem = Path::new(entry)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("saida");
            match Path::new(entry).parent() {
                Some(parent) if !parent.as_os_str().is_empty() => parent
                    .join(format!("{stem}.bundle.rhp"))
                    .display()
                    .to_string(),
                _ => format!("{stem}.bundle.rhp"),
            }
        }
    };
    std::fs::write(&dest, bundled)
        .map_err(|e| format!("nao foi possivel escrever `{dest}`: {e}"))?;
    Ok(format!("bundle escrito em `{dest}`"))
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
            // `exit(codigo)` em teste nao mata o corredor: vira falha do arquivo.
            vm.run(&chunk, &mut sink).map_err(|e| match e.exit_code() {
                Some(code) => LoadError::Plain(format!("exit({code})")),
                None => {
                    let mut message = format!("erro de execucao: {e}");
                    for name in e.trace() {
                        message.push_str(&format!("\n  em {name}"));
                    }
                    LoadError::Plain(message)
                }
            })
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

/// Falha de execucao: mensagem de erro ou pedido de `exit(codigo)`.
/// `Exit` vira codigo de saida do processo no `rhp run` (sem mensagem);
/// no `rhp test`, vira falha do arquivo (o corredor possui o exit code).
#[derive(Debug)]
enum Fail {
    Msg(String),
    Exit(i32),
}

impl Fail {
    /// Mensagem para assercoes de teste e diagnosticos (inclui `exit(N)`).
    #[cfg(test)]
    fn into_message(self) -> String {
        match self {
            Fail::Msg(message) => message,
            Fail::Exit(code) => format!("exit({code})"),
        }
    }
}

/// Executa um arquivo `.rhp`, repassando argumentos extras para `argv()`.
fn run_file_with_args(path: &str, args: &[String], output: &mut dyn Write) -> Result<(), Fail> {
    run_file_limited(path, args, output, rhp_vm::VmLimits::default(), false)
}

/// REPL interativo (Etapa 4, DevX): le linhas do stdin, compila e executa
/// cada uma com `Vm::run_continue` (globais/funcoes sobrevivem; erro nao
/// mata a sessao). Prompt `> ` so em terminal (com pipe, sem ruido).
/// EOF (ou linha `sair`) encerra com codigo 0.
fn run_repl() -> Result<(), String> {
    use std::io::{BufRead, IsTerminal};
    let interactive = std::io::stdin().is_terminal();
    let mut vm = rhp_vm::Vm::new();
    let mut lines = std::io::BufReader::new(std::io::stdin()).lines();
    loop {
        if interactive {
            print!("> ");
            let _ = std::io::stdout().flush();
        }
        let Some(line) = lines.next() else {
            break;
        };
        let line = line.map_err(|e| format!("falha lendo stdin: {e}"))?;
        let src = line.trim();
        if src.is_empty() || src == "sair" {
            if src == "sair" {
                break;
            }
            continue;
        }
        let program = rhp_parser::parse_source(src).map_err(|e| format!("repl:{e}"))?;
        let chunk = rhp_bytecode::compile(&program).map_err(|e| {
            let (code, help) = rhp_diagnostic::compile_code(&e.to_string());
            with_code(code, help, &format!("repl: erro de compilacao: {e}"))
        })?;
        let mut out = std::io::stdout();
        if let Err(e) = vm.run_continue(&chunk, &mut out) {
            if e.exit_code().is_some() {
                break;
            }
            let (code, help) = rhp_diagnostic::runtime_code(&e.to_string());
            let mut message = format!("repl: erro de execucao: {e}");
            for name in e.trace() {
                message.push_str(&format!("\n  em {name}"));
            }
            eprintln!("rhp: {}", with_code(code, help, &message));
        }
    }
    Ok(())
}

/// Executa um arquivo `.rhp` com tetos opcionais (`VmLimits`).
fn run_file_limited(
    path: &str,
    args: &[String],
    output: &mut dyn Write,
    limits: rhp_vm::VmLimits,
    trace: bool,
) -> Result<(), Fail> {
    let program = load_program(path).map_err(|e| Fail::Msg(e.human()))?;
    run_program(&program, path, args, output, limits, trace)
}

/// Pipeline programa -> bytecode -> VM (erros com codigo estavel).
fn run_program(
    program: &Program,
    origin: &str,
    args: &[String],
    output: &mut dyn Write,
    limits: rhp_vm::VmLimits,
    trace: bool,
) -> Result<(), Fail> {
    let chunk = rhp_bytecode::compile(program).map_err(|e| {
        let (code, help) = rhp_diagnostic::compile_code(&e.to_string());
        Fail::Msg(with_code(
            code,
            help,
            &format!("{origin}: erro de compilacao: {e}"),
        ))
    })?;
    let mut vm = rhp_vm::Vm::new();
    vm.set_limits(limits);
    vm.set_trace(trace);
    vm.set_argv(args.to_vec());
    vm.set_program(Arc::new(program.clone()));
    vm.run(&chunk, output).map_err(|e| match e.exit_code() {
        Some(code) => Fail::Exit(code),
        None => {
            let (code, help) = rhp_diagnostic::runtime_code(&e.to_string());
            // Stack trace (Etapa 4): pilha de chamadas, da mais interna
            // para a mais externa. Ausente no nivel superior (sem ruido).
            let mut message = format!("{origin}: erro de execucao: {e}");
            for name in e.trace() {
                message.push_str(&format!("\n  em {name}"));
            }
            Fail::Msg(with_code(code, help, &message))
        }
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
    run_program(
        &program,
        origin,
        &[],
        output,
        rhp_vm::VmLimits::default(),
        false,
    )
    .map_err(|fail| match fail {
        Fail::Msg(message) => message,
        Fail::Exit(code) => format!("exit({code})"),
    })
}

/// Teto de tamanho de fonte por arquivo (protecao contra OOM no loader).
pub const MAX_SOURCE_BYTES: u64 = 8 * 1024 * 1024;

/// Modulo coletado (fase 1 do loader); a fase 2 (`link_modules`) renomeia
/// e funde em um programa so.
struct ModuleFile {
    canonical: PathBuf,
    shown: String,
    statements: Vec<StmtNode>,
    trailing_comments: Vec<String>,
    /// Imports na ordem do fonte, ja com caminho resolvido.
    imports: Vec<ResolvedImport>,
    /// Re-exports (`export { } from`) na ordem do fonte, resolvidos.
    reexports: Vec<ResolvedImport>,
}

/// `import` (bare ou seletivo) com caminho canonico resolvido.
struct ResolvedImport {
    canonical: PathBuf,
    shown: String,
    names: Option<Vec<ImportName>>,
}

/// Carrega o programa de entrada com `import`s resolvidos.
///
/// Semantica: busca em profundidade a partir do arquivo de entrada; cada
/// modulo executa uma vez, antes do arquivo que o importa; caminhos sao
/// relativos ao arquivo que contem o `import` e confinados a arvore do
/// arquivo de entrada; importar o mesmo arquivo varias vezes o inclui uma
/// unica vez; ciclo, arquivo fora da arvore, arquivo grande demais e
/// `import` fora do nivel superior sao erros.
///
/// Privacidade (ADR 0009): `export` opt-in por definicao; sem `export` =
/// privado. `import "x"` (bare) mantem a semantica legada (todos os nomes
/// visiveis); `import { a as b } from "x"` so vincula nomes exportados.
fn load_program(entry: &str) -> Result<Program, LoadError> {
    let entry_path = Path::new(entry);
    let canonical_entry = std::fs::canonicalize(entry_path)
        .map_err(|e| LoadError::Plain(format!("modulo nao encontrado `{entry}`: {e}")))?;
    let root: PathBuf = canonical_entry
        .parent()
        .map_or_else(|| PathBuf::from("."), std::path::Path::to_path_buf);
    let mut visiting: Vec<PathBuf> = Vec::new();
    let mut by_canonical: HashMap<PathBuf, usize> = HashMap::new();
    let mut modules: Vec<ModuleFile> = Vec::new();
    collect_module(
        entry_path,
        &root,
        &mut visiting,
        &mut by_canonical,
        &mut modules,
    )?;
    let (statements, trailing_comments) = link_modules(&modules).map_err(LoadError::Plain)?;
    Ok(Program {
        statements,
        trailing_comments,
    })
}

/// Resolve uma dependencia (`import` ou re-export): junta ao diretorio base,
/// canonicaliza, aplica a jaula da arvore e coleta recursivamente (ciclos
/// viram "importacao circular"). Mesma regra para ambos (RFC 0011).
#[allow(clippy::too_many_arguments)]
fn resolve_dep(
    base_dir: &Path,
    root: &Path,
    raw: &str,
    names: Option<Vec<ImportName>>,
    visiting: &mut Vec<PathBuf>,
    by_canonical: &mut HashMap<PathBuf, usize>,
    modules: &mut Vec<ModuleFile>,
) -> Result<ResolvedImport, LoadError> {
    let dep = base_dir.join(raw);
    let dep_shown = dep.display().to_string();
    let dep_canonical = std::fs::canonicalize(&dep)
        .map_err(|e| LoadError::Plain(format!("modulo nao encontrado `{dep_shown}`: {e}")))?;
    if !dep_canonical.starts_with(root) {
        return Err(LoadError::Plain(format!(
            "import fora da arvore do programa: `{dep_shown}` (permitido apenas sob `{}`)",
            root.display()
        )));
    }
    collect_module(&dep, root, visiting, by_canonical, modules)?;
    Ok(ResolvedImport {
        canonical: dep_canonical,
        shown: dep_shown,
        names,
    })
}

fn collect_module(
    path: &Path,
    root: &Path,
    visiting: &mut Vec<PathBuf>,
    by_canonical: &mut HashMap<PathBuf, usize>,
    modules: &mut Vec<ModuleFile>,
) -> Result<usize, LoadError> {
    let shown = path.display().to_string();
    let canonical = std::fs::canonicalize(path)
        .map_err(|e| LoadError::Plain(format!("modulo nao encontrado `{shown}`: {e}")))?;
    if !canonical.starts_with(root) {
        return Err(LoadError::Plain(format!(
            "import fora da arvore do programa: `{shown}` (permitido apenas sob `{}`)",
            root.display()
        )));
    }
    if let Some(&idx) = by_canonical.get(&canonical) {
        return Ok(idx);
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
    // Imports e re-exports do nivel superior (bare + seletivos); aninhados = erro.
    // Mesma jaula/ciclo/teto para ambos (re-export tambem cria dependencia).
    let (raw_imports, raw_reexports) = collect_imports(&program.statements)?;
    let mut imports = Vec::new();
    for import in raw_imports {
        let (raw, names) = import;
        imports.push(resolve_dep(
            &base_dir,
            root,
            &raw,
            names,
            visiting,
            by_canonical,
            modules,
        )?);
    }
    let mut reexports = Vec::new();
    for reexport in raw_reexports {
        let (raw, names) = reexport;
        reexports.push(resolve_dep(
            &base_dir,
            root,
            &raw,
            Some(names),
            visiting,
            by_canonical,
            modules,
        )?);
    }
    visiting.pop();
    let idx = modules.len();
    by_canonical.insert(canonical.clone(), idx);
    modules.push(ModuleFile {
        canonical,
        shown,
        statements: program.statements,
        trailing_comments: program.trailing_comments,
        imports,
        reexports,
    });
    Ok(idx)
}

/// Liga os modulos coletados em um programa so (fase 2, ADR 0009).
///
/// - Ordem de execucao = pos-ordem DFS (dependencias antes, como antes).
/// - Modulos importados tem nomes de nivel superior renomeados com prefixo
///   reservado `__rhp_mod{N}__` (identificador valido, mas proibido no
///   fonte do usuario; bundle segue reparseavel). O entry mantem nomes
///   simples (globais do programa).
/// - Cada `import`/`import from` vira `let`s de alias no inicio da secao
///   do importador (nunca reexecuta o modulo): bare vincula todos os nomes
///   (legado); seletivo so exportados (privacidade real).
/// - Repetir o mesmo import nao reemite nada; nomes ja vinculados sao
///   pulados (colisao real continua erro, como hoje).
fn link_modules(modules: &[ModuleFile]) -> Result<(Vec<StmtNode>, Vec<String>), String> {
    let index_of: HashMap<&PathBuf, usize> = modules
        .iter()
        .enumerate()
        .map(|(i, m)| (&m.canonical, i))
        .collect();
    let mut mangled: Vec<HashMap<String, String>> = vec![HashMap::new(); modules.len()];
    let mut exports: Vec<HashSet<String>> = vec![HashSet::new(); modules.len()];
    let mut tops: Vec<Vec<String>> = vec![Vec::new(); modules.len()];
    // Passo 1: exports + nomes de topo por modulo.
    for (idx, module) in modules.iter().enumerate() {
        for stmt in &module.statements {
            let def_name = match &stmt.stmt {
                Stmt::Let { name, .. } | Stmt::Const { name, .. } => Some(name.clone()),
                Stmt::FnDef { name, .. } => Some(name.clone()),
                Stmt::Export { def } => match &def.stmt {
                    Stmt::FnDef { name, .. }
                    | Stmt::Let { name, .. }
                    | Stmt::Const { name, .. } => Some(name.clone()),
                    _ => {
                        return Err(format!(
                            "`export` invalido em `{}` (so `fn`, `let`, `const`)",
                            module.shown
                        ));
                    }
                },
                Stmt::Import(_) | Stmt::ImportNames { .. } => None,
                _ => None,
            };
            if let Some(name) = def_name {
                if name.starts_with(RESERVED_PREFIX) {
                    return Err(format!(
                        "nome reservado ao linker `{name}` em `{}` (renomeie)",
                        module.shown
                    ));
                }
                // Duplicatas passam adiante: o `DeclareName` acusa em
                // runtime, como no merge legado.
                tops[idx].push(name);
            }
            if let Stmt::Export { def } = &stmt.stmt {
                let name = match &def.stmt {
                    Stmt::FnDef { name, .. }
                    | Stmt::Let { name, .. }
                    | Stmt::Const { name, .. } => name.clone(),
                    _ => {
                        return Err(format!(
                            "`export` invalido em `{}` (so `fn`, `let`, `const`)",
                            module.shown
                        ));
                    }
                };
                if !exports[idx].insert(name.clone()) {
                    return Err(format!("export duplicado `{name}` em `{}`", module.shown));
                }
            }
        }
        // Re-exports (RFC 0011): nomes devem existir nos exports do dep
        // (mesmo erro do import seletivo) e passam aos exports deste modulo
        // (transitivo: deps tem indice menor, ja processados).
        for reexp in &module.reexports {
            let Some(&dep_idx) = index_of.get(&reexp.canonical) else {
                return Err(format!(
                    "modulo nao coletado `{}` (erro interno do linker)",
                    reexp.shown
                ));
            };
            let Some(names) = &reexp.names else {
                return Err(format!(
                    "re-export sem nomes em `{}` (erro interno do linker)",
                    module.shown
                ));
            };
            for selected in names {
                if !exports[dep_idx].contains(&selected.name) {
                    return Err(format!(
                        "`{}` nao e exportado por `{}`",
                        selected.name, reexp.shown
                    ));
                }
                let visible = selected.alias.clone().unwrap_or(selected.name.clone());
                if !exports[idx].insert(visible.clone()) {
                    return Err(format!(
                        "export duplicado `{visible}` em `{}`",
                        module.shown
                    ));
                }
            }
        }
    }
    // Passo 2: prefixos (entry sem prefixo) e fusao em pos-ordem.
    let mut out: Vec<StmtNode> = Vec::new();
    let mut trailing: Vec<String> = Vec::new();
    // Nome visivel global -> (modulo de origem, nome original): mesma origem
    // = repeticao (pula, ex.: diamante com a mesma origem); origem distinta
    // = colisao (erro de loader). Global pois aliases vivem no escopo global
    // do bundle (RFC 0011: viabiliza barris sem redeclare em runtime).
    let mut bound: HashMap<String, (usize, String)> = HashMap::new();
    // Repasse de re-export: (modulo reexportador, nome visivel) ->
    // (modulo de origem, nome original). Cadeias sempre descem de indice
    // (dep < importador), logo terminam; ciclos ja sao erro na coleta.
    let mut forward: HashMap<(usize, String), (usize, String)> = HashMap::new();
    for (idx, module) in modules.iter().enumerate() {
        let is_entry = idx + 1 == modules.len();
        if !is_entry {
            let prefix = hidden_prefix(idx);
            for name in &tops[idx] {
                mangled[idx].insert(name.clone(), format!("{prefix}{name}"));
            }
        }
        // Aliases dos imports deste modulo (prefixo da secao do importador;
        // nunca reexecuta o modulo). Mesmo `emit` serve aos re-exports.
        // `bound` e global por nome visivel: aliases vivem no escopo global
        // do bundle (RFC 0011: barris e diamantes com a mesma origem nao
        // reemitem; origem distinta = colisao, erro de loader).
        let mut emit = |visible: &str,
                        dep: usize,
                        origin: &str,
                        out: &mut Vec<StmtNode>|
         -> Result<(), String> {
            match bound.get(visible) {
                Some((same_dep, same_origin)) if *same_dep == dep && *same_origin == origin => {}
                Some(_) => {
                    return Err(format!(
                        "colisao de importacao: `{visible}` ja vinculado (de outro modulo)"
                    ));
                }
                None => {
                    bound.insert(visible.to_string(), (dep, origin.to_string()));
                    out.push(alias_let(visible, origin));
                }
            }
            Ok(())
        };
        // Atravessa re-exports (RFC 0011): (dep, nome) pode ser um repasse;
        // resolve ate a origem definitiva (cadeia sempre desce de indice).
        let resolve = |mut target: (usize, String),
                       forward: &HashMap<(usize, String), (usize, String)>| {
            while let Some(next) = forward.get(&target) {
                target = next.clone();
            }
            target
        };
        for import in &module.imports {
            let Some(&dep_idx) = index_of.get(&import.canonical) else {
                return Err(format!(
                    "modulo nao coletado `{}` (erro interno do linker)",
                    import.shown
                ));
            };
            match &import.names {
                None => {
                    // Bare (legado): vincula TODOS os nomes de topo e os
                    // re-exportados (superficie publica; RFC 0011).
                    for original in tops[dep_idx].clone() {
                        let origin = mangled_name(dep_idx, &original, &mangled);
                        emit(&original, dep_idx, &origin, &mut out)?;
                    }
                    for reexp in &modules[dep_idx].reexports {
                        let Some(names) = &reexp.names else {
                            continue;
                        };
                        for selected in names {
                            let visible = selected.alias.clone().unwrap_or(selected.name.clone());
                            let (rdep, rname) = resolve((dep_idx, selected.name.clone()), &forward);
                            let origin = mangled_name(rdep, &rname, &mangled);
                            emit(&visible, rdep, &origin, &mut out)?;
                        }
                    }
                }
                Some(names) => {
                    for selected in names {
                        if !exports[dep_idx].contains(&selected.name) {
                            return Err(format!(
                                "`{}` nao e exportado por `{}`",
                                selected.name, import.shown
                            ));
                        }
                        let visible = selected.alias.clone().unwrap_or(selected.name.clone());
                        let (rdep, rname) = resolve((dep_idx, selected.name.clone()), &forward);
                        let origin = mangled_name(rdep, &rname, &mangled);
                        emit(&visible, rdep, &origin, &mut out)?;
                    }
                }
            }
        }
        // Re-exports (RFC 0011): registra o repasse para importadores futuros
        // e emite aliases `let visivel = origem` neste modulo (para o corpo
        // do barril e consistencia; nunca reexecuta o modulo).
        for reexp in &module.reexports {
            let Some(&dep_idx) = index_of.get(&reexp.canonical) else {
                return Err(format!(
                    "modulo nao coletado `{}` (erro interno do linker)",
                    reexp.shown
                ));
            };
            let Some(names) = &reexp.names else {
                return Err(format!(
                    "re-export sem nomes em `{}` (erro interno do linker)",
                    module.shown
                ));
            };
            for selected in names {
                let visible = selected.alias.clone().unwrap_or(selected.name.clone());
                let (rdep, rname) = resolve((dep_idx, selected.name.clone()), &forward);
                forward.insert((idx, visible.clone()), (rdep, rname.clone()));
                let origin = mangled_name(rdep, &rname, &mangled);
                emit(&visible, rdep, &origin, &mut out)?;
            }
        }
        // Proprios statements: `export` desembrulha a definicao (que e
        // renomeada e fundida como as demais); imports/marcas somem.
        // A decoracao (comentarios) atravessa o renomeador junto (Etapa 4).
        let own: Vec<StmtNode> = module
            .statements
            .iter()
            .filter_map(|node| match &node.stmt {
                Stmt::Import(_) | Stmt::ImportNames { .. } | Stmt::ReExport { .. } => None,
                Stmt::Export { def } => {
                    let mut inner = def.as_ref().clone();
                    inner.leading = node.leading.iter().chain(&inner.leading).cloned().collect();
                    Some(inner)
                }
                _ => Some(node.clone()),
            })
            .collect();
        trailing.extend(module.trailing_comments.iter().cloned());
        if is_entry {
            out.extend(own);
        } else {
            let top_set: HashSet<String> = tops[idx].iter().cloned().collect();
            out.extend(rename_module(&own, &top_set, &hidden_prefix(idx)));
        }
    }
    Ok((out, trailing))
}

/// Nome efetivo no modulo fundido (entry = simples; demais = mangled).
fn mangled_name(dep_idx: usize, original: &str, mangled: &[HashMap<String, String>]) -> String {
    mangled[dep_idx]
        .get(original)
        .cloned()
        .unwrap_or_else(|| original.to_string())
}

/// `let visivel = origem;` (ponte do linker; sem saida nem efeito).
fn alias_let(visible: &str, origin: &str) -> StmtNode {
    StmtNode::from(Stmt::Let {
        name: visible.to_string(),
        ann: None,
        init: Some(rhp_parser::Expr::Var(origin.to_string())),
    })
}

/// Prefixo de isolamento entre modulos: identificador valido (para o
/// bundle continuar legivel e reparseavel), mas RESERVADO — nenhum nome
/// de nivel superior (em qualquer modulo, inclusive o entry) pode comecar
/// com ele; o linker rejeita com erro claro.
fn hidden_prefix(idx: usize) -> String {
    format!("__rhp_mod{idx}__")
}

/// Prefixo reservado ao linker (ver `hidden_prefix`).
const RESERVED_PREFIX: &str = "__rhp_mod_";

/// Renomeia os nomes de nivel superior (`top`) de um modulo com `prefix`
/// (isolamento; ver `hidden_prefix`).
///
/// Espelha as regras de escopo do compilador: referencias que resolvem
/// para o topo sao renomeadas; sombras lexicas (`let` interno, params,
/// var de `for`/`catch`) preservam o nome simples.
fn rename_module(statements: &[StmtNode], top: &HashSet<String>, prefix: &str) -> Vec<StmtNode> {
    let mut renamer = Renamer {
        top,
        prefix,
        scopes: vec![HashSet::new()],
    };
    statements.iter().map(|s| renamer.stmt(s)).collect()
}

struct Renamer<'a> {
    top: &'a HashSet<String>,
    prefix: &'a str,
    /// Sombras lexicas por nivel (o nivel 0 e o topo; vinculos do topo
    /// vivem em `top`, nao aqui).
    scopes: Vec<HashSet<String>>,
}

impl Renamer<'_> {
    fn mangled(&self, name: &str) -> String {
        format!("{}{}", self.prefix, name)
    }

    /// Referencia (`Var`, callee, alvo de atribuicao): renomeia se resolve
    /// ao topo sem sombra intermediaria.
    fn reference(&self, name: &str) -> String {
        if self.scopes.iter().any(|s| s.contains(name)) {
            return name.to_string();
        }
        if self.top.contains(name) {
            return self.mangled(name);
        }
        name.to_string()
    }

    /// Vinculo novo (`let`/`const`/`fn`, parametro, var de laco/catch):
    /// no topo renomeia (e o proprio vinculo de `top`); dentro, registra
    /// sombra e mantem simples.
    fn declare(&mut self, name: &str) -> String {
        if self.scopes.len() == 1 {
            if self.top.contains(name) {
                return self.mangled(name);
            }
            return name.to_string();
        }
        self.scopes
            .last_mut()
            .expect("escopo ativo")
            .insert(name.to_string());
        name.to_string()
    }

    fn stmt(&mut self, node: &StmtNode) -> StmtNode {
        let stmt = match &node.stmt {
            Stmt::Echo { expr } => Stmt::Echo {
                expr: self.expr(expr),
            },
            Stmt::Let { name, ann, init } => {
                // `init` antes do vinculo (como o compilador avalia).
                let renamed_init = init.as_ref().map(|e| self.expr(e));
                let renamed = self.declare(name);
                Stmt::Let {
                    name: renamed,
                    ann: *ann,
                    init: renamed_init,
                }
            }
            Stmt::Const { name, ann, init } => {
                let renamed_init = self.expr(init);
                let renamed = self.declare(name);
                Stmt::Const {
                    name: renamed,
                    ann: *ann,
                    init: renamed_init,
                }
            }
            Stmt::Static { name, init } => {
                let renamed_init = self.expr(init);
                let renamed = self.declare(name);
                Stmt::Static {
                    name: renamed,
                    init: renamed_init,
                }
            }
            Stmt::Destructure {
                target,
                init,
                is_const,
            } => {
                let renamed_init = self.expr(init);
                let target = match target {
                    rhp_parser::DestructureTarget::Positional(names) => {
                        rhp_parser::DestructureTarget::Positional(
                            names.iter().map(|n| self.declare(n)).collect(),
                        )
                    }
                    rhp_parser::DestructureTarget::ByKey(pairs) => {
                        rhp_parser::DestructureTarget::ByKey(
                            pairs
                                .iter()
                                .map(|(k, n)| (k.clone(), self.declare(n)))
                                .collect(),
                        )
                    }
                };
                Stmt::Destructure {
                    target,
                    init: renamed_init,
                    is_const: *is_const,
                }
            }
            Stmt::Break => Stmt::Break,
            Stmt::BreakN(n) => Stmt::BreakN(*n),
            Stmt::Continue => Stmt::Continue,
            Stmt::ContinueN(n) => Stmt::ContinueN(*n),
            Stmt::Assign { target, expr } => {
                let renamed_target = match target {
                    AssignTarget::Var(name) => AssignTarget::Var(self.reference(name)),
                    AssignTarget::Index { name, index } => AssignTarget::Index {
                        name: self.reference(name),
                        index: self.expr(index),
                    },
                };
                Stmt::Assign {
                    target: renamed_target,
                    expr: self.expr(expr),
                }
            }
            Stmt::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let cond = self.expr(cond);
                self.scopes.push(HashSet::new());
                let then_branch = then_branch.iter().map(|s| self.stmt(s)).collect();
                self.scopes.pop();
                let else_branch = else_branch.as_ref().map(|branch| {
                    self.scopes.push(HashSet::new());
                    let renamed: Vec<StmtNode> = branch.iter().map(|s| self.stmt(s)).collect();
                    self.scopes.pop();
                    renamed
                });
                Stmt::If {
                    cond,
                    then_branch,
                    else_branch,
                }
            }
            Stmt::While { cond, body } => {
                let cond = self.expr(cond);
                self.scopes.push(HashSet::new());
                let body = body.iter().map(|s| self.stmt(s)).collect();
                self.scopes.pop();
                Stmt::While { cond, body }
            }
            Stmt::For { var, iter, body } => {
                let iter = self.expr(iter);
                self.scopes.push(HashSet::new());
                let renamed_var = self.declare(var);
                let body = body.iter().map(|s| self.stmt(s)).collect();
                self.scopes.pop();
                Stmt::For {
                    var: renamed_var,
                    iter,
                    body,
                }
            }
            Stmt::ForC {
                init,
                cond,
                step,
                body,
            } => {
                // Um escopo para init+cond+passo (paridade com a VM);
                // corpo em escopo aninhado.
                self.scopes.push(HashSet::new());
                let init = init.as_ref().map(|s| Box::new(self.stmt(s)));
                let cond = cond.as_ref().map(|e| self.expr(e));
                self.scopes.push(HashSet::new());
                let body = body.iter().map(|s| self.stmt(s)).collect();
                self.scopes.pop();
                let step = step.as_ref().map(|s| Box::new(self.stmt(s)));
                self.scopes.pop();
                Stmt::ForC {
                    init,
                    cond,
                    step,
                    body,
                }
            }
            Stmt::Switch {
                discriminant,
                cases,
            } => {
                let discriminant = self.expr(discriminant);
                let cases = cases
                    .iter()
                    .map(|case| {
                        let cond = case.cond.as_ref().map(|e| self.expr(e));
                        // Escopo proprio por case (paridade com a VM).
                        self.scopes.push(HashSet::new());
                        let body = case.body.iter().map(|s| self.stmt(s)).collect();
                        self.scopes.pop();
                        rhp_parser::SwitchCase { cond, body }
                    })
                    .collect();
                Stmt::Switch {
                    discriminant,
                    cases,
                }
            }
            Stmt::FnDef {
                name,
                params,
                variadic,
                return_ann,
                body,
            } => {
                let renamed = self.declare(name);
                // Defaults sao constantes (sem Vars validos); renomeia no
                // escopo externo antes de vincular os params (como `let`).
                let params = params
                    .iter()
                    .map(|p| rhp_parser::Param {
                        name: p.name.clone(),
                        ann: p.ann,
                        default: p.default.as_ref().map(|e| self.expr(e)),
                    })
                    .collect::<Vec<_>>();
                self.scopes.push(HashSet::new());
                for param in &params {
                    self.declare(&param.name);
                }
                if let Some(rest) = variadic {
                    self.declare(rest);
                }
                let body = body.iter().map(|s| self.stmt(s)).collect();
                self.scopes.pop();
                Stmt::FnDef {
                    name: renamed,
                    params,
                    variadic: variadic.clone(),
                    return_ann: *return_ann,
                    body,
                }
            }
            Stmt::Return { expr } => Stmt::Return {
                expr: expr.as_ref().map(|e| self.expr(e)),
            },
            Stmt::Throw { expr } => Stmt::Throw {
                expr: self.expr(expr),
            },
            Stmt::Try {
                body,
                catch_var,
                catch_body,
                finally_body,
            } => {
                self.scopes.push(HashSet::new());
                let body = body.iter().map(|s| self.stmt(s)).collect();
                self.scopes.pop();
                let (renamed_var, renamed_catch) = match catch_body {
                    None => (None, None),
                    Some(catch_body) => {
                        self.scopes.push(HashSet::new());
                        let renamed_var = catch_var.as_deref().map(|v| self.declare(v));
                        let renamed_catch = catch_body.iter().map(|s| self.stmt(s)).collect();
                        self.scopes.pop();
                        (renamed_var, Some(renamed_catch))
                    }
                };
                let renamed_finally = finally_body.as_ref().map(|fin| {
                    self.scopes.push(HashSet::new());
                    let renamed: Vec<StmtNode> = fin.iter().map(|s| self.stmt(s)).collect();
                    self.scopes.pop();
                    renamed
                });
                Stmt::Try {
                    body,
                    catch_var: renamed_var,
                    catch_body: renamed_catch,
                    finally_body: renamed_finally,
                }
            }
            Stmt::Block(stmts) => {
                self.scopes.push(HashSet::new());
                let renamed = stmts.iter().map(|s| self.stmt(s)).collect();
                self.scopes.pop();
                Stmt::Block(renamed)
            }
            Stmt::ExprStmt { expr } => Stmt::ExprStmt {
                expr: self.expr(expr),
            },
            // O linker remove antes de renomear; defesa em profundidade.
            Stmt::Import(_) | Stmt::ImportNames { .. } | Stmt::Export { .. } => node.stmt.clone(),
            Stmt::ReExport { .. } => node.stmt.clone(),
        };
        StmtNode {
            leading: node.leading.clone(),
            trailing: node.trailing.clone(),
            blank_before: node.blank_before,
            stmt,
        }
    }

    /// Renomeia um argumento de chamada (RFC 0009): forma preservada,
    /// expressoes internas renomeadas.
    fn call_arg(&mut self, arg: &rhp_parser::CallArg) -> rhp_parser::CallArg {
        match arg {
            rhp_parser::CallArg::Pos(expr) => rhp_parser::CallArg::Pos(self.expr(expr)),
            rhp_parser::CallArg::Named(name, expr) => {
                rhp_parser::CallArg::Named(name.clone(), self.expr(expr))
            }
            rhp_parser::CallArg::Spread(expr) => rhp_parser::CallArg::Spread(self.expr(expr)),
        }
    }

    fn expr(&mut self, expr: &Expr) -> Expr {
        match expr {
            Expr::StringLiteral(_) | Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Null => {
                expr.clone()
            }
            Expr::Var(name) => Expr::Var(self.reference(name)),
            Expr::Array(items) => Expr::Array(items.iter().map(|e| self.expr(e)).collect()),
            Expr::Map(entries) => Expr::Map(
                entries
                    .iter()
                    .map(|(k, v)| (k.clone(), self.expr(v)))
                    .collect(),
            ),
            Expr::Index { base, index } => Expr::Index {
                base: Box::new(self.expr(base)),
                index: Box::new(self.expr(index)),
            },
            Expr::Unary { op, expr } => Expr::Unary {
                op: *op,
                expr: Box::new(self.expr(expr)),
            },
            Expr::Ternary {
                cond,
                then_branch,
                else_branch,
            } => Expr::Ternary {
                cond: Box::new(self.expr(cond)),
                then_branch: Box::new(self.expr(then_branch)),
                else_branch: Box::new(self.expr(else_branch)),
            },
            Expr::Binary { op, left, right } => Expr::Binary {
                op: *op,
                left: Box::new(self.expr(left)),
                right: Box::new(self.expr(right)),
            },
            Expr::Call { name, args } => Expr::Call {
                name: self.reference(name),
                args: args.iter().map(|a| self.call_arg(a)).collect(),
            },
            Expr::CallValue { callee, args } => Expr::CallValue {
                callee: Box::new(self.expr(callee)),
                args: args.iter().map(|a| self.call_arg(a)).collect(),
            },
            Expr::Match { subject, arms } => Expr::Match {
                subject: Box::new(self.expr(subject)),
                arms: arms
                    .iter()
                    .map(|arm| rhp_parser::MatchArm {
                        cond: arm.cond.as_ref().map(|e| self.expr(e)),
                        value: self.expr(&arm.value),
                    })
                    .collect(),
            },
            Expr::Closure(data) => {
                let params = data
                    .params
                    .iter()
                    .map(|p| rhp_parser::Param {
                        name: p.name.clone(),
                        ann: p.ann,
                        default: p.default.as_ref().map(|e| self.expr(e)),
                    })
                    .collect::<Vec<_>>();
                self.scopes.push(HashSet::new());
                for param in &params {
                    self.declare(&param.name);
                }
                if let Some(rest) = &data.variadic {
                    self.declare(rest);
                }
                let body = data.body.iter().map(|s| self.stmt(s)).collect();
                self.scopes.pop();
                Expr::Closure(Box::new(rhp_parser::ClosureData {
                    params,
                    variadic: data.variadic.clone(),
                    return_ann: data.return_ann,
                    body,
                }))
            }
        }
    }
}
/// Imports brutos do nivel superior: (caminho, seletivo ou `None`).
type RawImports = Vec<(String, Option<Vec<ImportName>>)>;
/// Re-exports brutos: (caminho, nomes). `export * from` nao existe (RFC 0011).
type RawReExports = Vec<(String, Vec<ImportName>)>;

fn collect_imports(statements: &[StmtNode]) -> Result<(RawImports, RawReExports), LoadError> {
    let mut imports = Vec::new();
    let mut reexports = Vec::new();
    for node in statements {
        match &node.stmt {
            Stmt::Import(path) => imports.push((path.clone(), None)),
            Stmt::ImportNames { path, names } => imports.push((path.clone(), Some(names.clone()))),
            Stmt::ReExport { path, names } => reexports.push((path.clone(), names.clone())),
            Stmt::Export { .. } => {}
            _ => {}
        }
    }
    check_no_nested_imports(statements)?;
    Ok((imports, reexports))
}

/// Rejeita `import`/`export` dentro de blocos/funcoes (a semantica seria
/// icar o modulo para o topo, surpreendente e nao documentada).
/// Varre as expressoes diretas de um statement a procura de corpos de
/// closure com `import`/`export` (unico aninhamento via expressao).
fn check_stmt_expr_imports(stmt: &Stmt) -> Result<(), LoadError> {
    match stmt {
        Stmt::Echo { expr }
        | Stmt::Const { init: expr, .. }
        | Stmt::Assign { expr, .. }
        | Stmt::ExprStmt { expr } => check_expr_imports(expr),
        Stmt::Let { init, .. } => {
            if let Some(expr) = init {
                check_expr_imports(expr)?;
            }
            Ok(())
        }
        Stmt::Return { expr } => {
            if let Some(expr) = expr {
                check_expr_imports(expr)?;
            }
            Ok(())
        }
        Stmt::Throw { expr } => check_expr_imports(expr),
        Stmt::Destructure { init, .. } => check_expr_imports(init),
        Stmt::Static { init, .. } => check_expr_imports(init),
        Stmt::If { cond, .. } | Stmt::While { cond, .. } | Stmt::For { iter: cond, .. } => {
            check_expr_imports(cond)
        }
        Stmt::Switch {
            discriminant,
            cases,
        } => {
            check_expr_imports(discriminant)?;
            for case in cases {
                if let Some(cond) = &case.cond {
                    check_expr_imports(cond)?;
                }
                check_block_imports(&case.body, "em `switch`")?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Desce recursivamente ate corpos de closure e aplica a mesma regra de
/// `check_block_imports` (erro claro em vez de ignorar silencioso).
fn check_expr_imports(expr: &rhp_parser::Expr) -> Result<(), LoadError> {
    use rhp_parser::Expr as E;
    match expr {
        E::Closure(data) => check_block_imports(&data.body, "em closure"),
        E::CallValue { callee, args } => {
            check_expr_imports(callee)?;
            for arg in args {
                check_expr_imports(arg.value())?;
            }
            Ok(())
        }
        E::Match { subject, arms } => {
            check_expr_imports(subject)?;
            for arm in arms {
                if let Some(cond) = &arm.cond {
                    check_expr_imports(cond)?;
                }
                check_expr_imports(&arm.value)?;
            }
            Ok(())
        }
        E::Array(items) => {
            for item in items {
                check_expr_imports(item)?;
            }
            Ok(())
        }
        E::Call { args, .. } => {
            for arg in args {
                check_expr_imports(arg.value())?;
            }
            Ok(())
        }
        E::Map(entries) => {
            for (_, v) in entries {
                check_expr_imports(v)?;
            }
            Ok(())
        }
        E::Index { base, index } => {
            check_expr_imports(base)?;
            check_expr_imports(index)
        }
        E::Unary { expr, .. } => check_expr_imports(expr),
        E::Binary { left, right, .. } => {
            check_expr_imports(left)?;
            check_expr_imports(right)
        }
        E::Ternary {
            cond,
            then_branch,
            else_branch,
        } => {
            check_expr_imports(cond)?;
            check_expr_imports(then_branch)?;
            check_expr_imports(else_branch)
        }
        _ => Ok(()),
    }
}

fn check_no_nested_imports(statements: &[StmtNode]) -> Result<(), LoadError> {
    for node in statements {
        match &node.stmt {
            Stmt::Import(_)
            | Stmt::ImportNames { .. }
            | Stmt::Export { .. }
            | Stmt::ReExport { .. } => {}
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
            Stmt::For { body, .. } => check_block_imports(body, "em `for`")?,
            Stmt::ForC {
                init,
                cond,
                step,
                body,
            } => {
                // `init`/`step` sao `let`/`const`/atribuicao/expressao:
                // reutiliza o varredor de expressoes.
                if let Some(init) = init {
                    check_stmt_expr_imports(&init.stmt)?;
                }
                if let Some(cond) = cond {
                    check_expr_imports(cond)?;
                }
                if let Some(step) = step {
                    check_stmt_expr_imports(&step.stmt)?;
                }
                check_block_imports(body, "em `for`")?;
            }
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    check_block_imports(&case.body, "em `switch`")?;
                }
            }
            Stmt::Try {
                body,
                catch_body,
                finally_body,
                ..
            } => {
                check_block_imports(body, "em `try`")?;
                if let Some(catch_body) = catch_body {
                    check_block_imports(catch_body, "em `catch`")?;
                }
                if let Some(finally_body) = finally_body {
                    check_block_imports(finally_body, "em `finally`")?;
                }
            }
            Stmt::FnDef {
                body, name, params, ..
            } => {
                check_block_imports(body, &format!("na funcao `{name}`"))?;
                for param in params {
                    if let Some(default) = &param.default {
                        check_expr_imports(default)?;
                    }
                }
            }
            Stmt::Block(body) => check_block_imports(body, "em bloco")?,
            // Closure e o unico caminho de `import` aninhado via expressao
            // (RFC 0003); rejeita com o mesmo erro claro das funcoes.
            other => check_stmt_expr_imports(other)?,
        }
    }
    Ok(())
}

fn check_block_imports(body: &[StmtNode], context: &str) -> Result<(), LoadError> {
    for node in body {
        match &node.stmt {
            Stmt::Import(path) => {
                return Err(LoadError::Plain(format!(
                    "import {context} nao suportado (`{path}`): use `import` apenas no nivel superior do arquivo"
                )));
            }
            Stmt::ImportNames { path, .. } => {
                return Err(LoadError::Plain(format!(
                    "import {context} nao suportado (`{path}`): use `import` apenas no nivel superior do arquivo"
                )));
            }
            Stmt::Export { .. } => {
                return Err(LoadError::Plain(format!(
                    "export {context} nao suportado: use `export` apenas no nivel superior do arquivo"
                )));
            }
            Stmt::ReExport { path, .. } => {
                return Err(LoadError::Plain(format!(
                    "re-export {context} nao suportado (`{path}`): use `export {{ }} from` apenas no nivel superior do arquivo"
                )));
            }
            _ => {}
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
            .expect_err("deve falhar")
            .into_message();
        assert!(err.contains("modulo nao encontrado"));
    }

    #[test]
    fn selective_import_respects_privacy() {
        let dir = temp_dir("privado");
        write_temp_module(
            &dir,
            "mat.rhp",
            "export fn dobro(x) {\n  return ajuda(x) + ajuda(x);\n}\nfn ajuda(x) {\n  return x;\n}\nlet segredo = 7;\n",
        );
        let main = write_temp_module(
            &dir,
            "main.rhp",
            "import { dobro } from \"mat.rhp\";\necho dobro(21);\n",
        );
        let mut buf: Vec<u8> = Vec::new();
        run_file_with_args(&main.to_string_lossy(), &[], &mut buf).expect("deve passar");
        assert_eq!(buf, b"42\n");
        // Privado nao vaza: nem funcao auxiliar, nem `let` sem export.
        let sneaky = write_temp_module(
            &dir,
            "sneaky.rhp",
            "import { dobro } from \"mat.rhp\";\necho ajuda(1);\n",
        );
        let mut buf: Vec<u8> = Vec::new();
        let err = run_file_with_args(&sneaky.to_string_lossy(), &[], &mut buf)
            .expect_err("deve falhar")
            .into_message();
        assert!(err.contains("nao definida"), "obteve: {err}");
        let sneaky2 = write_temp_module(
            &dir,
            "sneaky2.rhp",
            "import { segredo } from \"mat.rhp\";\necho segredo;\n",
        );
        let mut buf: Vec<u8> = Vec::new();
        let err = run_file_with_args(&sneaky2.to_string_lossy(), &[], &mut buf)
            .expect_err("deve falhar")
            .into_message();
        assert!(err.contains("nao e exportado"), "obteve: {err}");
    }

    #[test]
    fn selective_import_supports_alias_and_mixed_modes() {
        let dir = temp_dir("alias");
        write_temp_module(
            &dir,
            "mat.rhp",
            "export fn dobro(x) {\n  return x + x;\n}\nexport let base = 40;\nlet oculto = 1;\n",
        );
        // Alias + execucao unica mesmo com bare + seletivo juntos.
        let main = write_temp_module(
            &dir,
            "main.rhp",
            "import \"mat.rhp\";\nimport { dobro as d, base as b } from \"mat.rhp\";\necho d(1) + b + base;\n",
        );
        let mut buf: Vec<u8> = Vec::new();
        run_file_with_args(&main.to_string_lossy(), &[], &mut buf).expect("deve passar");
        assert_eq!(buf, b"82\n");
        // Bare repetido nao reexecuta (efeito colateral uma vez so).
        write_temp_module(&dir, "eco.rhp", "echo \"carregou\";\nexport let v = 1;\n");
        let main2 = write_temp_module(
            &dir,
            "main2.rhp",
            "import \"eco.rhp\";\nimport \"eco.rhp\";\nimport { v } from \"eco.rhp\";\necho v;\n",
        );
        let mut buf: Vec<u8> = Vec::new();
        run_file_with_args(&main2.to_string_lossy(), &[], &mut buf).expect("deve passar");
        assert_eq!(buf, b"carregou\n1\n");
    }

    #[test]
    fn selective_import_detects_collisions_and_cycles() {
        let dir = temp_dir("colisao");
        write_temp_module(&dir, "a.rhp", "export let x = 1;\n");
        write_temp_module(&dir, "b.rhp", "export let x = 2;\n");
        // Nomes distintos: sem colisao.
        let ok = write_temp_module(
            &dir,
            "ok.rhp",
            "import { x as xa } from \"a.rhp\";\nimport { x as xb } from \"b.rhp\";\necho xa + xb;\n",
        );
        let mut buf: Vec<u8> = Vec::new();
        run_file_with_args(&ok.to_string_lossy(), &[], &mut buf).expect("deve passar");
        assert_eq!(buf, b"3\n");
        // Mesmo nome visivel: erro alto.
        let clash = write_temp_module(
            &dir,
            "clash.rhp",
            "import { x } from \"a.rhp\";\nimport { x } from \"b.rhp\";\n",
        );
        let mut buf: Vec<u8> = Vec::new();
        let err = run_file_with_args(&clash.to_string_lossy(), &[], &mut buf)
            .expect_err("deve falhar")
            .into_message();
        assert!(err.contains("colisao de importacao"), "obteve: {err}");
        // Ciclo via seletivo tambem e erro.
        write_temp_module(
            &dir,
            "c1.rhp",
            "import { v } from \"c2.rhp\";\nexport let u = 1;\n",
        );
        write_temp_module(
            &dir,
            "c2.rhp",
            "import { u } from \"c1.rhp\";\nexport let v = 2;\n",
        );
        let cyc = write_temp_module(&dir, "cyc.rhp", "import { u } from \"c1.rhp\";\n");
        let mut buf: Vec<u8> = Vec::new();
        let err = run_file_with_args(&cyc.to_string_lossy(), &[], &mut buf)
            .expect_err("deve falhar")
            .into_message();
        assert!(err.contains("circular"), "obteve: {err}");
    }

    #[test]
    fn rejects_nested_and_duplicate_export() {
        let dir = temp_dir("export");
        // `export` dentro de funcao: so nivel superior.
        let main = write_temp_module(&dir, "main.rhp", "fn f() {\n  export let x = 1;\n}\n");
        let mut buf: Vec<u8> = Vec::new();
        let err = run_file_with_args(&main.to_string_lossy(), &[], &mut buf)
            .expect_err("deve falhar")
            .into_message();
        assert!(err.contains("nivel superior"), "obteve: {err}");
        // Export duplicado: erro claro no link.
        write_temp_module(&dir, "dup.rhp", "export let x = 1;\nexport fn x() {\n}\n");
        let main2 = write_temp_module(&dir, "main2.rhp", "import { x } from \"dup.rhp\";\n");
        let mut buf: Vec<u8> = Vec::new();
        let err = run_file_with_args(&main2.to_string_lossy(), &[], &mut buf)
            .expect_err("deve falhar")
            .into_message();
        assert!(err.contains("duplicado"), "obteve: {err}");
    }

    #[test]
    fn rename_respects_shadowing() {
        use std::collections::HashSet as Set;
        // Sombra interna nao e renomeada; referencia ao topo sim.
        let stmts = rhp_parser::parse_source("let x = 1;\nfn f(x) {\n  return x;\n}\necho x;\n")
            .expect("parse deve passar")
            .statements;
        let top: Set<String> = ["x".to_string(), "f".to_string()].into_iter().collect();
        let renamed = rename_module(&stmts, &top, "__rhp_mod0__");
        // `fn f(x)` vira `fn M#f(M#x)`? Nao: param `x` e sombra (simples).
        match &renamed[1].stmt {
            Stmt::FnDef { name, params, .. } => {
                assert_eq!(name, "__rhp_mod0__f");
                assert_eq!(
                    params.iter().map(|p| p.name.clone()).collect::<Vec<_>>(),
                    ["x".to_string()]
                );
            }
            other => panic!("esperado FnDef, obteve {other:?}"),
        }
        // `echo x` no topo vira `echo M#x`.
        match &renamed[2].stmt {
            Stmt::Echo { expr } => match expr {
                rhp_parser::Expr::Var(name) => assert_eq!(name, "__rhp_mod0__x"),
                other => panic!("esperado Var, obteve {other:?}"),
            },
            other => panic!("esperado Echo, obteve {other:?}"),
        }
    }

    #[test]
    fn runs_closures_end_to_end() {
        let mut buf: Vec<u8> = Vec::new();
        run_source(
            "let f = fn (x) { return x + 1; }; echo f(41); echo (fn (x) { return x * 2; })(21);",
            "<teste>",
            &mut buf,
        )
        .expect("deve passar");
        assert_eq!(buf, b"42\n42\n");
    }

    #[test]
    fn runs_switch_end_to_end() {
        let mut buf: Vec<u8> = Vec::new();
        run_source(
            "switch 2 { case 1 { echo \"a\"; } default { echo \"d\"; } case 2 { echo \"b\"; break; } }",
            "<teste>",
            &mut buf,
        )
        .expect("deve passar");
        assert_eq!(buf, b"d\nb\n");
    }

    #[test]
    fn runs_defaults_and_variadic_end_to_end() {
        let mut buf: Vec<u8> = Vec::new();
        run_source(
            "fn f(a, b = 2, ...rest) { echo a + b; echo rest; } f(1); f(10, 20, 30);",
            "<teste>",
            &mut buf,
        )
        .expect("deve passar");
        assert_eq!(buf, b"3\n[]\n30\n[30]\n");
    }

    #[test]
    fn runs_destructuring_end_to_end() {
        let mut buf: Vec<u8> = Vec::new();
        run_source(
            "let [a, b] = [1, 2]; echo a + b; const {k: v} = {k: 40}; echo v + 2;",
            "<teste>",
            &mut buf,
        )
        .expect("deve passar");
        assert_eq!(buf, b"3\n42\n");
    }

    #[test]
    fn runs_nucleo2_end_to_end() {
        let mut buf: Vec<u8> = Vec::new();
        run_source(
            "echo match 2 { 1 => \"um\", 2 => \"dois\" }; let f = fn (x) => x * 2; echo f(21); for let i = 0; i < 2; i = i + 1 { echo i; }",
            "<teste>",
            &mut buf,
        )
        .expect("deve passar");
        assert_eq!(buf, b"dois\n42\n0\n1\n");
    }

    #[test]
    fn runs_mixed_calls_end_to_end() {
        let mut buf: Vec<u8> = Vec::new();
        run_source(
            "fn soma(a, b) { return a + b; } echo soma(y: 2, ...[30]); echo soma(b: 2, a: 40);",
            "<teste>",
            &mut buf,
        )
        .expect_err("y desconhecido deve falhar");
        let mut buf: Vec<u8> = Vec::new();
        run_source(
            "fn soma(a, b) { return a + b; } echo soma(b: 2, a: 40); echo soma(...[1, 2]);",
            "<teste>",
            &mut buf,
        )
        .expect("deve passar");
        assert_eq!(buf, b"42\n3\n");
    }

    #[test]
    fn runs_static_end_to_end() {
        let mut buf: Vec<u8> = Vec::new();
        run_source(
            "fn c() { static n = 0; n = n + 1; return n; } echo c(); echo c();",
            "<teste>",
            &mut buf,
        )
        .expect("deve passar");
        assert_eq!(buf, b"1\n2\n");
    }

    #[test]
    fn rejects_static_outside_function() {
        let dir = temp_dir("static-topo");
        let main = write_temp_module(&dir, "main.rhp", "static x = 1;\n");
        let mut buf: Vec<u8> = Vec::new();
        let err = run_file_with_args(&main.to_string_lossy(), &[], &mut buf)
            .expect_err("deve falhar")
            .into_message();
        assert!(err.contains("fora de funcao"), "obteve: {err}");
    }

    #[test]
    fn rename_mixed_calls() {
        use std::collections::HashSet as Set;
        // Nomes de parametros NAO sao renomeados; valores sao.
        let stmts = rhp_parser::parse_source("let x = 1;\necho f(x, y: x);\n")
            .expect("parse deve passar")
            .statements;
        let top: Set<String> = ["x".to_string(), "f".to_string()].into_iter().collect();
        let renamed = rename_module(&stmts, &top, "__rhp_mod0__");
        match &renamed[1].stmt {
            Stmt::Echo { expr } => match expr {
                rhp_parser::Expr::Call { args, .. } => {
                    assert!(matches!(&args[0],
                        rhp_parser::CallArg::Pos(rhp_parser::Expr::Var(n)) if n == "__rhp_mod0__x"));
                    assert!(matches!(&args[1],
                        rhp_parser::CallArg::Named(n, rhp_parser::Expr::Var(v))
                        if n == "y" && v == "__rhp_mod0__x"));
                }
                other => panic!("esperado Call, obteve {other:?}"),
            },
            other => panic!("esperado Echo, obteve {other:?}"),
        }
    }

    #[test]
    fn rejects_import_inside_match_arm() {
        let dir = temp_dir("match-import");
        let main = write_temp_module(
            &dir,
            "main.rhp",
            "echo match 1 { 1 => (fn () { import \"outro.rhp\"; return 1; })(), default => 0 };\n",
        );
        let mut buf: Vec<u8> = Vec::new();
        let err = run_file_with_args(&main.to_string_lossy(), &[], &mut buf)
            .expect_err("deve falhar")
            .into_message();
        assert!(err.contains("nivel superior"), "obteve: {err}");
    }

    #[test]
    fn rename_forc_declares_init() {
        use std::collections::HashSet as Set;
        let stmts =
            rhp_parser::parse_source("let t = 3;\nfor let i = 0; i < t; i = i + 1 { echo i; }\n")
                .expect("parse deve passar")
                .statements;
        let top: Set<String> = ["t".to_string()].into_iter().collect();
        let renamed = rename_module(&stmts, &top, "__rhp_mod0__");
        match &renamed[1].stmt {
            Stmt::ForC { init, .. } => match init.as_deref().map(|n| &n.stmt) {
                Some(Stmt::Let { name, .. }) => assert_eq!(name, "i"),
                other => panic!("esperado Let no init, obteve {other:?}"),
            },
            other => panic!("esperado ForC, obteve {other:?}"),
        }
    }

    #[test]
    fn rename_destructuring_declares_names() {
        use std::collections::HashSet as Set;
        let stmts = rhp_parser::parse_source("let x = [1, 2];\nlet [a, b] = x;\necho a;\n")
            .expect("parse deve passar")
            .statements;
        let top: Set<String> = ["x".to_string()].into_iter().collect();
        let renamed = rename_module(&stmts, &top, "__rhp_mod0__");
        match &renamed[1].stmt {
            Stmt::Destructure { target, init, .. } => {
                assert!(matches!(
                    init,
                    rhp_parser::Expr::Var(name) if name == "__rhp_mod0__x"
                ));
                match target {
                    rhp_parser::DestructureTarget::Positional(names) => {
                        assert_eq!(names, &["a".to_string(), "b".to_string()]);
                    }
                    other => panic!("esperado Positional, obteve {other:?}"),
                }
            }
            other => panic!("esperado Destructure, obteve {other:?}"),
        }
    }

    #[test]
    fn rejects_import_inside_default() {
        // Default com closure contendo `import`: mesmo erro claro.
        let dir = temp_dir("default-import");
        let main = write_temp_module(
            &dir,
            "main.rhp",
            "fn f(a = (fn () { import \"outro.rhp\"; return 1; })()) { return a; }\n",
        );
        let mut buf: Vec<u8> = Vec::new();
        let err = run_file_with_args(&main.to_string_lossy(), &[], &mut buf)
            .expect_err("deve falhar")
            .into_message();
        assert!(err.contains("nivel superior"), "obteve: {err}");
    }

    #[test]
    fn runs_reexport_chain_and_alias() {
        let dir = temp_dir("reexport");
        write_temp_module(
            &dir,
            "lib.rhp",
            "export let v = 41;\nexport fn f(x) { return x + 1; }\nlet privado = 1;\n",
        );
        write_temp_module(
            &dir,
            "bar.rhp",
            "export { v, f as inc } from \"lib.rhp\";\n",
        );
        write_temp_module(&dir, "chain.rhp", "export { v } from \"bar.rhp\";\n");
        let main = write_temp_module(
            &dir,
            "main.rhp",
            "import { v, inc } from \"bar.rhp\";\nimport { v as w } from \"chain.rhp\";\necho v + inc(0) + w;\n",
        );
        let mut buf: Vec<u8> = Vec::new();
        run_file_with_args(&main.to_string_lossy(), &[], &mut buf).expect("deve passar");
        assert_eq!(buf, b"83\n");
    }

    #[test]
    fn rejects_bad_reexports() {
        // Nome nao exportado pelo dep.
        let dir = temp_dir("reexport-erro");
        write_temp_module(&dir, "lib.rhp", "export let v = 1;\n");
        write_temp_module(&dir, "bar.rhp", "export { nope } from \"lib.rhp\";\n");
        let main = write_temp_module(&dir, "main.rhp", "import { nope } from \"bar.rhp\";\n");
        let mut buf: Vec<u8> = Vec::new();
        let err = run_file_with_args(&main.to_string_lossy(), &[], &mut buf)
            .expect_err("deve falhar")
            .into_message();
        assert!(err.contains("nao e exportado"), "obteve: {err}");
        // Ciclo via re-export.
        write_temp_module(&dir, "c1.rhp", "export { u } from \"c2.rhp\";\n");
        write_temp_module(&dir, "c2.rhp", "export { u } from \"c1.rhp\";\n");
        let main2 = write_temp_module(&dir, "m2.rhp", "import { u } from \"c1.rhp\";\n");
        let mut buf: Vec<u8> = Vec::new();
        let err = run_file_with_args(&main2.to_string_lossy(), &[], &mut buf)
            .expect_err("deve falhar")
            .into_message();
        assert!(err.contains("circular"), "obteve: {err}");
        // Re-export aninhado rejeitado.
        let nest = write_temp_module(
            &dir,
            "nest.rhp",
            "fn f() { export { v } from \"lib.rhp\"; }\n",
        );
        let main3 = write_temp_module(&dir, "m3.rhp", "import \"nest.rhp\";\n");
        let mut buf: Vec<u8> = Vec::new();
        let err = run_file_with_args(&main3.to_string_lossy(), &[], &mut buf)
            .expect_err("deve falhar")
            .into_message();
        assert!(err.contains("nivel superior"), "obteve: {err}");
        let _ = nest;
    }

    #[test]
    fn runs_fs_list_builtin() {
        // Ordenado; vazio => `[]`; inexistente => erro.
        let dir = temp_dir("fs-list");
        std::fs::write(dir.join("z.txt"), b"z").expect("setup");
        std::fs::write(dir.join("a.txt"), b"a").expect("setup");
        std::fs::create_dir(dir.join("sub")).expect("setup");
        let main = write_temp_module(
            &dir,
            "main.rhp",
            &format!("echo fs_list({:?});\n", dir.to_string_lossy()),
        );
        let mut buf: Vec<u8> = Vec::new();
        run_file_with_args(&main.to_string_lossy(), &[], &mut buf).expect("deve passar");
        let out = String::from_utf8(buf).expect("saida utf8");
        assert!(
            out.contains("a.txt") && out.contains("sub") && out.contains("z.txt"),
            "obteve: {out}"
        );
        assert!(
            out.find("a.txt").unwrap() < out.find("z.txt").unwrap(),
            "ordenado: {out}"
        );
        let empty_parent = temp_dir("fs-list-vazio");
        let empty = empty_parent.join("vazio");
        std::fs::create_dir(&empty).expect("setup");
        let main2 = write_temp_module(
            &empty_parent,
            "main.rhp",
            &format!("echo fs_list({:?});\n", empty.to_string_lossy()),
        );
        let mut buf2: Vec<u8> = Vec::new();
        run_file_with_args(&main2.to_string_lossy(), &[], &mut buf2).expect("deve passar");
        assert_eq!(buf2, b"[]\n");
        let main3 = write_temp_module(
            &dir,
            "bad.rhp",
            "echo fs_list(\"/caminho/que/nao/existe-xyz\");\n",
        );
        let mut buf3: Vec<u8> = Vec::new();
        let err = run_file_with_args(&main3.to_string_lossy(), &[], &mut buf3)
            .expect_err("deve falhar")
            .into_message();
        assert!(err.contains("fs_list"), "obteve: {err}");
    }

    #[test]
    fn rejects_import_inside_switch_case() {
        let dir = temp_dir("switch-import");
        let main = write_temp_module(
            &dir,
            "main.rhp",
            "switch 1 {\n  case 1 {\n    import \"outro.rhp\";\n  }\n}\n",
        );
        let mut buf: Vec<u8> = Vec::new();
        let err = run_file_with_args(&main.to_string_lossy(), &[], &mut buf)
            .expect_err("deve falhar")
            .into_message();
        assert!(err.contains("nivel superior"), "obteve: {err}");
    }

    #[test]
    fn rename_switch_cases_have_own_scope() {
        use std::collections::HashSet as Set;
        // `let v` em cases distintos nao colide; topo renomeado.
        let stmts = rhp_parser::parse_source(
            "let x = 1;\nswitch x {\n  case 1 {\n    let v = 1;\n    echo v;\n  }\n  case 2 {\n    let v = 2;\n    echo v;\n  }\n}\n",
        )
        .expect("parse deve passar")
        .statements;
        let top: Set<String> = ["x".to_string()].into_iter().collect();
        let renamed = rename_module(&stmts, &top, "__rhp_mod0__");
        match &renamed[1].stmt {
            Stmt::Switch {
                discriminant,
                cases,
            } => {
                assert!(matches!(
                    discriminant,
                    rhp_parser::Expr::Var(name) if name == "__rhp_mod0__x"
                ));
                assert_eq!(cases.len(), 2);
                for case in cases {
                    assert!(matches!(&case.body[0].stmt,
                        Stmt::Let { name, .. } if name == "v"));
                }
            }
            other => panic!("esperado Switch, obteve {other:?}"),
        }
    }

    #[test]
    fn rejects_import_inside_closure() {
        let dir = temp_dir("closure-import");
        let main = write_temp_module(
            &dir,
            "main.rhp",
            "let f = fn () {\n  import \"outro.rhp\";\n  return 1;\n};\n",
        );
        let mut buf: Vec<u8> = Vec::new();
        let err = run_file_with_args(&main.to_string_lossy(), &[], &mut buf)
            .expect_err("deve falhar")
            .into_message();
        assert!(err.contains("nivel superior"), "obteve: {err}");
    }

    #[test]
    fn rename_closures_shadow_params() {
        use std::collections::HashSet as Set;
        // Parametro sombreia o topo; livre do corpo e renomeada.
        let stmts = rhp_parser::parse_source("let x = 1;\nlet f = fn (x) {\n  return x;\n};\n")
            .expect("parse deve passar")
            .statements;
        let top: Set<String> = ["x".to_string(), "f".to_string()].into_iter().collect();
        let renamed = rename_module(&stmts, &top, "__rhp_mod0__");
        match &renamed[1].stmt {
            Stmt::Let {
                init: Some(expr), ..
            } => match expr {
                rhp_parser::Expr::Closure(data) => {
                    assert_eq!(
                        data.params
                            .iter()
                            .map(|p| p.name.clone())
                            .collect::<Vec<_>>(),
                        ["x".to_string()]
                    );
                    assert!(matches!(&data.body[0].stmt,
                        Stmt::Return { expr: Some(rhp_parser::Expr::Var(name)) }
                        if name == "x"));
                }
                other => panic!("esperada Closure, obteve {other:?}"),
            },
            other => panic!("esperado Let, obteve {other:?}"),
        }
    }

    #[test]
    fn reports_circular_import() {
        let dir = temp_dir("ciclo");
        write_temp_module(&dir, "a.rhp", "import \"b.rhp\";\n");
        let b = write_temp_module(&dir, "b.rhp", "import \"a.rhp\";\n");
        let mut buf: Vec<u8> = Vec::new();
        let err = run_file_with_args(&b.to_string_lossy(), &[], &mut buf)
            .expect_err("deve falhar")
            .into_message();
        assert!(err.contains("importacao circular"));
    }

    #[test]
    fn rejects_import_outside_entry_tree() {
        let dir = temp_dir("jaula");
        let sub = dir.join("sub");
        std::fs::create_dir_all(&sub).expect("nao foi possivel criar subdir");
        write_temp_module(&dir, "fora.rhp", "let smuggled = 1;\n");
        let main = write_temp_module(&sub, "main.rhp", "import \"../fora.rhp\";\n");
        let mut buf: Vec<u8> = Vec::new();
        let err = run_file_with_args(&main.to_string_lossy(), &[], &mut buf)
            .expect_err("deve falhar")
            .into_message();
        assert!(err.contains("fora da arvore"), "obteve: {err}");
    }

    #[test]
    fn rejects_oversized_source() {
        let dir = temp_dir("grande");
        let big = dir.join("big.rhp");
        let content = "echo 1;\n".repeat(2 * 1024 * 1024);
        std::fs::write(&big, content).expect("nao foi possivel escrever big.rhp");
        let mut buf: Vec<u8> = Vec::new();
        let err = run_file_with_args(&big.to_string_lossy(), &[], &mut buf)
            .expect_err("deve falhar")
            .into_message();
        assert!(err.contains("muito grande"), "obteve: {err}");
    }

    #[test]
    fn respects_step_limit_via_loader() {
        let dir = temp_dir("limite");
        let main = write_temp_module(&dir, "loop.rhp", "while true {\n}\n");
        let mut buf: Vec<u8> = Vec::new();
        let err = run_file_limited(
            &main.to_string_lossy(),
            &[],
            &mut buf,
            rhp_vm::VmLimits {
                max_steps: Some(500),
                ..Default::default()
            },
            false,
        )
        .expect_err("deve falhar")
        .into_message();
        assert!(err.contains("limite de instrucoes"), "obteve: {err}");
    }

    #[test]
    fn rejects_heap_over_budget_via_loader() {
        let dir = temp_dir("heap");
        let main = write_temp_module(
            &dir,
            "grande.rhp",
            "let a = []; let i = 0; while i < 10 { a = push(a, \"0123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789\"); i = i + 1; } echo len(a);\n",
        );
        let mut buf: Vec<u8> = Vec::new();
        let err = run_file_limited(
            &main.to_string_lossy(),
            &[],
            &mut buf,
            rhp_vm::VmLimits {
                max_heap_bytes: Some(512),
                ..Default::default()
            },
            false,
        )
        .expect_err("deve falhar")
        .into_message();
        assert!(err.contains("heap excedeu o limite"), "obteve: {err}");
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
}
