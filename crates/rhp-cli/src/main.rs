//! Binario `rhp` — CLI oficial (Fase 5: +`fmt`).
//!
//! Uso: `rhp run <arquivo.rhp>` | `rhp fmt [--check] <arquivo.rhp>`

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rhp_parser::{Program, Stmt};

const USAGE: &str = "uso: rhp run [--max-steps N] <arquivo.rhp> | rhp fmt [--check] <arquivo.rhp>";

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
        [_, cmd, flag, value, path] if cmd == "run" && flag == "--max-steps" => {
            match value.parse::<u64>() {
                Ok(max_steps) => {
                    match run_file_limited(path, &mut std::io::stdout(), Some(max_steps)) {
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
    run_file_limited(path, output, None)
}

/// Executa um arquivo `.rhp` com teto opcional de instrucoes.
fn run_file_limited(
    path: &str,
    output: &mut dyn Write,
    max_steps: Option<u64>,
) -> Result<(), String> {
    let program = load_program(path)?;
    run_program(&program, path, output, rhp_vm::VmLimits { max_steps })
}

/// Pipeline programa -> bytecode -> VM.
fn run_program(
    program: &Program,
    origin: &str,
    output: &mut dyn Write,
    limits: rhp_vm::VmLimits,
) -> Result<(), String> {
    let chunk =
        rhp_bytecode::compile(program).map_err(|e| format!("{origin}: erro de compilacao: {e}"))?;
    let mut vm = rhp_vm::Vm::new();
    vm.set_limits(limits);
    vm.run(&chunk, output)
        .map_err(|e| format!("{origin}: erro de execucao: {e}"))
}

/// Pipeline fonte (sem arquivos) -> tokens -> parser -> bytecode -> VM.
///
/// Auxiliar de testes (a CLI sempre passa pelo carregador de modulos).
#[cfg(test)]
fn run_source(source: &str, origin: &str, output: &mut dyn Write) -> Result<(), String> {
    let program = rhp_parser::parse_source(source).map_err(|e| format!("{origin}:{e}"))?;
    run_program(&program, origin, output, rhp_vm::VmLimits::default())
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
fn load_program(entry: &str) -> Result<Program, String> {
    let entry_path = Path::new(entry);
    let canonical_entry = std::fs::canonicalize(entry_path)
        .map_err(|e| format!("modulo nao encontrado `{entry}`: {e}"))?;
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
) -> Result<(), String> {
    let shown = path.display().to_string();
    let canonical =
        std::fs::canonicalize(path).map_err(|e| format!("modulo nao encontrado `{shown}`: {e}"))?;
    if !canonical.starts_with(root) {
        return Err(format!(
            "import fora da arvore do programa: `{shown}` (permitido apenas sob `{}`)",
            root.display()
        ));
    }
    if visited.contains(&canonical) {
        return Ok(());
    }
    if visiting.contains(&canonical) {
        let mut chain: Vec<String> = visiting.iter().map(|p| p.display().to_string()).collect();
        chain.push(canonical.display().to_string());
        return Err(format!("importacao circular: {}", chain.join(" -> ")));
    }
    let size = std::fs::metadata(&canonical)
        .map_err(|e| format!("nao foi possivel ler `{shown}`: {e}"))?
        .len();
    if size > MAX_SOURCE_BYTES {
        return Err(format!(
            "arquivo muito grande `{shown}` ({size} bytes, limite {MAX_SOURCE_BYTES})"
        ));
    }
    visiting.push(canonical.clone());
    let source = std::fs::read_to_string(&canonical)
        .map_err(|e| format!("nao foi possivel ler `{shown}`: {e}"))?;
    let origin = canonical.display().to_string();
    let program = rhp_parser::parse_source(&source).map_err(|e| format!("{origin}:{e}"))?;
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
fn collect_imports(statements: &[Stmt]) -> Result<Vec<String>, String> {
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
fn check_no_nested_imports(statements: &[Stmt]) -> Result<(), String> {
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

fn check_block_imports(body: &[Stmt], context: &str) -> Result<(), String> {
    for stmt in body {
        if let Stmt::Import(path) = stmt {
            return Err(format!(
                "import {context} nao suportado (`{path}`): use `import` apenas no nivel superior do arquivo"
            ));
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
        let err = run_file("/caminho/que/nao/existe-12345.rhp", &mut buf).expect_err("deve falhar");
        assert!(err.contains("modulo nao encontrado"));
    }

    #[test]
    fn rejects_nested_import() {
        let dir = temp_dir("aninhado");
        write_temp_module(&dir, "lib.rhp", "let x = 1;\n");
        let main = write_temp_module(&dir, "main.rhp", "fn f() {\n  import \"lib.rhp\";\n}\n");
        let mut buf: Vec<u8> = Vec::new();
        let err = run_file(&main.to_string_lossy(), &mut buf).expect_err("deve falhar");
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
        let err = run_file(&main.to_string_lossy(), &mut buf).expect_err("deve falhar");
        assert!(err.contains("fora da arvore"), "obteve: {err}");
    }

    #[test]
    fn rejects_oversized_source() {
        let dir = temp_dir("grande");
        let big = dir.join("big.rhp");
        let content = "echo 1;\n".repeat(2 * 1024 * 1024);
        std::fs::write(&big, content).expect("nao foi possivel escrever big.rhp");
        let mut buf: Vec<u8> = Vec::new();
        let err = run_file(&big.to_string_lossy(), &mut buf).expect_err("deve falhar");
        assert!(err.contains("muito grande"), "obteve: {err}");
    }

    #[test]
    fn respects_step_limit_via_loader() {
        let dir = temp_dir("limite");
        let main = write_temp_module(&dir, "loop.rhp", "while true {\n}\n");
        let mut buf: Vec<u8> = Vec::new();
        let err = run_file_limited(&main.to_string_lossy(), &mut buf, Some(500))
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
