//! Binario `rhp` — CLI oficial (Fase 5: +`fmt`).
//!
//! Uso: `rhp run <arquivo.rhp>` | `rhp fmt [--check] <arquivo.rhp>`

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rhp_parser::{Program, Stmt};

const USAGE: &str = "uso: rhp run <arquivo.rhp> | rhp fmt [--check] <arquivo.rhp>";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match args.as_slice() {
        [_, cmd, path] if cmd == "run" => match run_file(path, &mut std::io::stdout()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                let _ = writeln!(std::io::stderr(), "rhp: {message}");
                ExitCode::from(1)
            }
        },
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
        [_, flag] if flag == "--help" || flag == "-h" => {
            println!("RHP (Fase 5: ferramentas)\n\n{USAGE}");
            ExitCode::SUCCESS
        }
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
    let formatted = rhp_fmt::format_source(&source).map_err(|e| format!("{path}:{e}"))?;
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

/// Executa um arquivo `.rhp` com imports resolvidos.
fn run_file(path: &str, output: &mut dyn Write) -> Result<(), String> {
    let program = load_program(path)?;
    run_program(&program, path, output)
}

/// Pipeline programa -> bytecode -> VM.
fn run_program(program: &Program, origin: &str, output: &mut dyn Write) -> Result<(), String> {
    let chunk =
        rhp_bytecode::compile(program).map_err(|e| format!("{origin}: erro de compilacao: {e}"))?;
    let mut vm = rhp_vm::Vm::new();
    vm.run(&chunk, output)
        .map_err(|e| format!("{origin}: erro de execucao: {e}"))
}

/// Pipeline fonte (sem arquivos) -> tokens -> parser -> bytecode -> VM.
///
/// Auxiliar de testes (a CLI sempre passa pelo carregador de modulos).
#[cfg(test)]
fn run_source(source: &str, origin: &str, output: &mut dyn Write) -> Result<(), String> {
    let program = rhp_parser::parse_source(source).map_err(|e| format!("{origin}:{e}"))?;
    run_program(&program, origin, output)
}

/// Carrega o programa de entrada com `import`s resolvidos.
///
/// Semantica: busca em profundidade a partir do arquivo de entrada; cada
/// modulo executa uma vez, antes do arquivo que o importa; caminhos sao
/// relativos ao arquivo que contem o `import`; importar o mesmo arquivo
/// varias vezes o inclui uma unica vez; ciclo e erro.
fn load_program(entry: &str) -> Result<Program, String> {
    let mut visiting: Vec<PathBuf> = Vec::new();
    let mut visited: HashSet<PathBuf> = HashSet::new();
    let mut statements: Vec<Stmt> = Vec::new();
    load_file(
        Path::new(entry),
        &mut visiting,
        &mut visited,
        &mut statements,
    )?;
    Ok(Program { statements })
}

fn load_file(
    path: &Path,
    visiting: &mut Vec<PathBuf>,
    visited: &mut HashSet<PathBuf>,
    out: &mut Vec<Stmt>,
) -> Result<(), String> {
    let shown = path.display().to_string();
    let canonical =
        std::fs::canonicalize(path).map_err(|e| format!("modulo nao encontrado `{shown}`: {e}"))?;
    if visited.contains(&canonical) {
        return Ok(());
    }
    if visiting.contains(&canonical) {
        let mut chain: Vec<String> = visiting.iter().map(|p| p.display().to_string()).collect();
        chain.push(canonical.display().to_string());
        return Err(format!("importacao circular: {}", chain.join(" -> ")));
    }
    visiting.push(canonical.clone());
    let source = std::fs::read_to_string(&canonical)
        .map_err(|e| format!("nao foi possivel ler `{shown}`: {e}"))?;
    let origin = canonical.display().to_string();
    let program = rhp_parser::parse_source(&source).map_err(|e| format!("{origin}:{e}"))?;
    let base_dir: PathBuf = canonical
        .parent()
        .map_or_else(|| PathBuf::from("."), std::path::Path::to_path_buf);
    for import in collect_imports(&program.statements) {
        load_file(&base_dir.join(import), visiting, visited, out)?;
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

/// Coleta todos os caminhos de `import` (qualquer nivel de aninhamento).
fn collect_imports(statements: &[Stmt]) -> Vec<String> {
    let mut imports = Vec::new();
    collect_imports_into(statements, &mut imports);
    imports
}

fn collect_imports_into(statements: &[Stmt], out: &mut Vec<String>) {
    for stmt in statements {
        match stmt {
            Stmt::Import(path) => out.push(path.clone()),
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                collect_imports_into(then_branch, out);
                if let Some(else_branch) = else_branch {
                    collect_imports_into(else_branch, out);
                }
            }
            Stmt::While { body, .. } | Stmt::Block(body) => {
                collect_imports_into(body, out);
            }
            Stmt::FnDef { body, .. } => collect_imports_into(body, out),
            _ => {}
        }
    }
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
        let err = run_file("/caminho/que/nao/existe-12345.rhp", &mut buf).expect_err("deve falhar");
        assert!(err.contains("modulo nao encontrado"));
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
        run_file(&main.to_string_lossy(), &mut buf).expect("deve passar");
        // Import duplicado inclui uma vez: sem erro de `ja declarada`.
        assert_eq!(buf, b"42\n");
    }

    #[test]
    fn reports_circular_import() {
        let dir = temp_dir("ciclo");
        write_temp_module(&dir, "a.rhp", "import \"b.rhp\";\n");
        let b = write_temp_module(&dir, "b.rhp", "import \"a.rhp\";\n");
        let mut buf: Vec<u8> = Vec::new();
        let err = run_file(&b.to_string_lossy(), &mut buf).expect_err("deve falhar");
        assert!(err.contains("importacao circular"));
    }
}
