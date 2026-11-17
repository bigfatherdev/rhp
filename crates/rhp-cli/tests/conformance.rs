//! Suite de conformidade: `tests/conformance/` fim a fim via `rhp run`.
//!
//! Etapa 3 (suporte): substitui pinagens ad-hoc por casos versionados
//! com resultado esperado (GOVERNANCE §8.2). Formato em
//! `tests/conformance/README.md`. Casos hermeticos e deterministicos;
//! todo comportamento pinado esta na LANGUAGE_SPEC (ou RFC/ADR aceita).

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

fn conformance_dir() -> PathBuf {
    workspace_root().join("tests").join("conformance")
}

fn sorted_entries() -> Vec<std::fs::DirEntry> {
    let mut entries: Vec<_> = std::fs::read_dir(conformance_dir())
        .expect("nao foi possivel listar tests/conformance/")
        .filter_map(Result::ok)
        .collect();
    entries.sort_by_key(|e| e.file_name());
    entries
}

#[test]
fn conformance_suite_pass_cases() {
    let mut count = 0;
    for entry in sorted_entries() {
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        if !name.ends_with(".rhp") || name.contains(".fail.") || name.ends_with("-lib.rhp") {
            continue;
        }
        let expected_path = path.with_extension("expected");
        assert!(
            expected_path.is_file(),
            "caso {name} sem {}.expected irmao",
            path.display()
        );
        count += 1;
        let output = run_file(&path);
        assert!(
            output.status.success(),
            "caso {name} falhou: stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        let expected = std::fs::read(&expected_path).expect("nao foi possivel ler .expected");
        assert_eq!(output.stdout, expected, "saida divergente no caso {name}");
    }
    assert!(count >= 18, "poucos casos validos: {count} (minimo 18)");
}

#[test]
fn conformance_suite_fail_cases() {
    let mut count = 0;
    for entry in sorted_entries() {
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        if !name.ends_with(".fail.rhp") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        let expected_path = path.with_file_name(format!("{stem}.expected"));
        assert!(
            expected_path.is_file(),
            "caso {name} sem {}.fail.expected irmao",
            path.display()
        );
        count += 1;
        let output = run_file(&path);
        assert!(
            !output.status.success(),
            "caso {name} deveria falhar, mas exit=0"
        );
        let needle =
            std::fs::read_to_string(&expected_path).expect("nao foi possivel ler .fail.expected");
        let needle = needle.trim();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(needle),
            "caso {name}: stderr nao contem {needle:?}, obteve: {stderr}"
        );
    }
    assert!(count >= 6, "poucos casos invalidos: {count} (minimo 6)");
}
