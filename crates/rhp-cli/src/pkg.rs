//! Pacotes v0 (`rhp init`, `rhp pkg add/install`, `rhp.lock`).
//!
//! Escopo deliberado: manifestos locais (`rhp.toml`, subconjunto estrito
//! de TOML), vendoring em `pacotes/` + lockfile com hash por arquivo e
//! `rhp pkg install --check` para CI. Sem rede, sem registro, sem
//! resolucao de versoes (transitivas = copiadas junto, sem resolver).
//!
//! Formato aceito (`#` = comentario, resto = erro claro):
//!
//! ```text
//! nome = "meuapp"
//! versao = "0.1.0"
//! [dependencias]
//! tempo = "../tempo"
//! ```
//!
//! Lockfile (`rhp.lock`, gerado — nao editar):
//!
//! ```text
//! [[pacote]]
//! nome = "tempo"
//! origem = "../tempo"
//! arquivo "data.rhp" = "<hash hex>"
//! ```

use std::path::{Path, PathBuf};

/// Manifesto (`rhp.toml`).
#[derive(Debug, Clone, PartialEq)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    /// (nome, caminho) na ordem do arquivo.
    pub deps: Vec<(String, String)>,
}

/// Pacote travado no lockfile.
#[derive(Debug, Clone, PartialEq)]
pub struct LockedPackage {
    pub name: String,
    pub origem: String,
    /// (caminho relativo com `/`, hash hex).
    pub files: Vec<(String, String)>,
}

/// FNV-1a de 64 bits, hex (deteccao de alteracao; nao e seguranca).
pub fn file_hash(bytes: &[u8]) -> String {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut hash = OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    format!("{hash:016x}")
}

fn is_name(text: &str) -> bool {
    let mut chars = text.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn parse_string(text: &str, origin: &str, line: usize) -> Result<String, String> {
    let text = text.trim();
    if !(text.starts_with('"') && text.ends_with('"') && text.len() >= 2) {
        return Err(format!("{origin}:{line}: esperado string entre aspas"));
    }
    let mut out = String::new();
    let mut chars = text[1..text.len() - 1].chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('n') => out.push('\n'),
                Some(other) => {
                    return Err(format!("{origin}:{line}: escape invalido `\\{other}`"));
                }
                None => {
                    return Err(format!("{origin}:{line}: escape no fim da string"));
                }
            }
        } else {
            out.push(ch);
        }
    }
    Ok(out)
}

/// Divide em itens com numero da linha (comentarios `#` ignorados fora de strings).
fn tokenize(text: &str, origin: &str) -> Result<Vec<(usize, String)>, String> {
    let mut items = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = i + 1;
        let mut code = String::new();
        let mut in_string = false;
        let mut chars = raw.chars();
        while let Some(ch) = chars.next() {
            if in_string {
                code.push(ch);
                if ch == '\\' {
                    if let Some(next) = chars.next() {
                        code.push(next);
                    }
                } else if ch == '"' {
                    in_string = false;
                }
            } else if ch == '#' {
                break;
            } else {
                if ch == '"' {
                    in_string = true;
                }
                code.push(ch);
            }
        }
        if in_string {
            return Err(format!("{origin}:{line}: string nao terminada"));
        }
        let code = code.trim().to_string();
        if !code.is_empty() {
            items.push((line, code));
        }
    }
    Ok(items)
}

/// Analisa `rhp.toml` (subconjunto estrito; resto = erro claro).
pub fn parse_manifest(text: &str, origin: &str) -> Result<Manifest, String> {
    let mut name: Option<String> = None;
    let mut version: Option<String> = None;
    let mut deps: Vec<(String, String)> = Vec::new();
    let mut section = String::new();
    for (line, code) in tokenize(text, origin)? {
        if code.starts_with('[') {
            if !code.ends_with(']') {
                return Err(format!("{origin}:{line}: secao malformada"));
            }
            section = code[1..code.len() - 1].trim().to_string();
            if section != "dependencias" {
                return Err(format!(
                    "{origin}:{line}: secao desconhecida `{section}` (so `[dependencias]`)"
                ));
            }
            continue;
        }
        let (key, raw) = code
            .split_once('=')
            .map(|(k, v)| (k.trim().to_string(), v.trim()))
            .ok_or_else(|| format!("{origin}:{line}: esperado `chave = ...`"))?;
        let value = parse_string(raw, origin, line)?;
        if section.is_empty() {
            match key.as_str() {
                "nome" => {
                    if !is_name(&value) {
                        return Err(format!(
                            "{origin}:{line}: nome invalido `{value}` (use [a-z][a-z0-9_]*)"
                        ));
                    }
                    name = Some(value);
                }
                "versao" => version = Some(value),
                _ => {
                    return Err(format!(
                        "{origin}:{line}: chave desconhecida `{key}` (use `nome`, `versao`)"
                    ));
                }
            }
        } else if section == "dependencias" {
            if !is_name(&key) {
                return Err(format!(
                    "{origin}:{line}: nome de dependencia invalido `{key}`"
                ));
            }
            if value.is_empty() {
                return Err(format!("{origin}:{line}: caminho vazio para `{key}`"));
            }
            deps.push((key, value));
        }
    }
    let name = name.ok_or_else(|| format!("{origin}: falta `nome = ...`"))?;
    let version = version.ok_or_else(|| format!("{origin}: falta `versao = ...`"))?;
    Ok(Manifest {
        name,
        version,
        deps,
    })
}

pub fn render_manifest(manifest: &Manifest) -> String {
    let mut out = format!(
        "# gerado por `rhp init` / `rhp pkg add`; edicao manual permitida\nnome = \"{}\"\nversao = \"{}\"\n",
        manifest.name, manifest.version
    );
    if !manifest.deps.is_empty() {
        out.push_str("\n[dependencias]\n");
        for (name, path) in &manifest.deps {
            out.push_str(&format!("{name} = \"{path}\"\n"));
        }
    }
    out
}

/// Analisa `rhp.lock` (formato do `render_lockfile`; resto = erro).
pub fn parse_lockfile(text: &str, origin: &str) -> Result<Vec<LockedPackage>, String> {
    let mut packages: Vec<LockedPackage> = Vec::new();
    let mut current: Option<LockedPackage> = None;
    let mut current_line = 0usize;
    let flush = |current: &mut Option<LockedPackage>, packages: &mut Vec<LockedPackage>| {
        if let Some(package) = current.take() {
            packages.push(package);
        }
    };
    for (line, code) in tokenize(text, origin)? {
        if code == "[[pacote]]" {
            flush(&mut current, &mut packages);
            current = Some(LockedPackage {
                name: String::new(),
                origem: String::new(),
                files: Vec::new(),
            });
            current_line = line;
            continue;
        }
        let package = current
            .as_mut()
            .ok_or_else(|| format!("{origin}:{line}: esperado `[[pacote]]` antes de chaves"))?;
        if let Some(rest) = code.strip_prefix("arquivo ") {
            let (path, hash) = rest
                .split_once('=')
                .map(|(p, h)| (p.trim(), h.trim()))
                .ok_or_else(|| format!("{origin}:{line}: esperado `arquivo \"...\" = \"...\"`"))?;
            let path = parse_string(path, origin, line)?;
            let hash = parse_string(hash, origin, line)?;
            package.files.push((path, hash));
            continue;
        }
        let (key, raw) = code
            .split_once('=')
            .map(|(k, v)| (k.trim(), v.trim()))
            .ok_or_else(|| format!("{origin}:{line}: esperado `chave = ...`"))?;
        match key {
            "nome" => package.name = parse_string(raw, origin, line)?,
            "origem" => package.origem = parse_string(raw, origin, line)?,
            _ => {
                return Err(format!(
                    "{origin}:{line}: chave desconhecida `{key}` no lockfile"
                ));
            }
        }
    }
    flush(&mut current, &mut packages);
    for package in &packages {
        if package.name.is_empty() || package.origem.is_empty() {
            return Err(format!(
                "{origin}:{current_line}: pacote sem `nome` ou `origem`"
            ));
        }
    }
    Ok(packages)
}

pub fn render_lockfile(packages: &[LockedPackage]) -> String {
    let mut out = String::from("# gerado por `rhp pkg install`; nao edite\n");
    for package in packages {
        out.push_str(&format!(
            "\n[[pacote]]\nnome = \"{}\"\norigem = \"{}\"\n",
            package.name, package.origem
        ));
        for (path, hash) in &package.files {
            out.push_str(&format!("arquivo \"{path}\" = \"{hash}\"\n"));
        }
    }
    out
}

/// Lista arquivos regulares sob `dir`, recursivo e ordenado (relativos
/// com `/`). Ignora diretorios `.git`.
pub fn collect_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = std::fs::read_dir(&current)
            .map_err(|e| format!("nao foi possivel listar `{}`: {e}", current.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| {
                format!(
                    "nao foi possivel ler entrada em `{}`: {e}",
                    current.display()
                )
            })?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                if name == ".git" {
                    continue;
                }
                stack.push(path);
            } else if path.is_file() {
                let rel = path
                    .strip_prefix(dir)
                    .map_err(|_| "caminho fora do pacote".to_string())?
                    .to_path_buf();
                files.push(rel);
            }
        }
    }
    files.sort();
    Ok(files)
}

/// Copia arvore `origem` -> `destino` (cria; remove destino previo).
fn copy_tree(origem: &Path, destino: &Path) -> Result<(), String> {
    if destino.exists() {
        std::fs::remove_dir_all(destino)
            .map_err(|e| format!("nao foi possivel limpar `{}`: {e}", destino.display()))?;
    }
    let mut stack = vec![origem.to_path_buf()];
    while let Some(current) = stack.pop() {
        let rel = current
            .strip_prefix(origem)
            .map_err(|_| "caminho fora do pacote".to_string())?;
        let target = destino.join(rel);
        if current.is_dir() {
            if current.file_name().map(|n| n == ".git").unwrap_or(false) {
                continue;
            }
            std::fs::create_dir_all(&target)
                .map_err(|e| format!("nao foi possivel criar `{}`: {e}", target.display()))?;
            let entries = std::fs::read_dir(&current)
                .map_err(|e| format!("nao foi possivel listar `{}`: {e}", current.display()))?;
            for entry in entries {
                let entry = entry.map_err(|e| {
                    format!(
                        "nao foi possivel ler entrada em `{}`: {e}",
                        current.display()
                    )
                })?;
                stack.push(entry.path());
            }
        } else if current.is_file() {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("nao foi possivel criar `{}`: {e}", parent.display()))?;
            }
            std::fs::copy(&current, &target)
                .map_err(|e| format!("nao foi possivel copiar `{}`: {e}", current.display()))?;
        }
    }
    Ok(())
}

fn manifest_path(dir: &Path) -> PathBuf {
    dir.join("rhp.toml")
}

fn lock_path(dir: &Path) -> PathBuf {
    dir.join("rhp.lock")
}

fn packages_dir(dir: &Path) -> PathBuf {
    dir.join("pacotes")
}

fn read_manifest(dir: &Path) -> Result<(Manifest, String), String> {
    let path = manifest_path(dir);
    let text = std::fs::read_to_string(&path)
        .map_err(|_| format!("sem `rhp.toml` em `{}` (rode `rhp init`)", dir.display()))?;
    let origin = path.display().to_string();
    let manifest = parse_manifest(&text, &origin)?;
    Ok((manifest, origin))
}

/// `rhp init [dir]`: cria `rhp.toml` (recusa se existir).
pub fn cmd_init(dir: &Path) -> Result<String, String> {
    let path = manifest_path(dir);
    if path.exists() {
        return Err(format!("`rhp.toml` ja existe em `{}`", dir.display()));
    }
    let name = dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("app")
        .to_string();
    let name = if is_name(&name) {
        name
    } else {
        "app".to_string()
    };
    let manifest = Manifest {
        name: name.clone(),
        version: "0.1.0".to_string(),
        deps: Vec::new(),
    };
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("nao foi possivel criar `{}`: {e}", dir.display()))?;
    std::fs::write(&path, render_manifest(&manifest))
        .map_err(|e| format!("nao foi possivel escrever `{}`: {e}", path.display()))?;
    Ok(format!("iniciado pacote `{name}` em `{}`", dir.display()))
}

/// `rhp pkg add <nome> <caminho>`: registra dependencia e instala.
pub fn cmd_add(dir: &Path, name: &str, origem: &str) -> Result<String, String> {
    if !is_name(name) {
        return Err(format!(
            "nome de pacote invalido `{name}` (use [a-z][a-z0-9_]*)"
        ));
    }
    if origem.is_empty() {
        return Err("caminho da dependencia vazio".to_string());
    }
    let (mut manifest, _) = read_manifest(dir)?;
    if let Some(pos) = manifest.deps.iter().position(|(n, _)| n == name) {
        manifest.deps[pos] = (name.to_string(), origem.to_string());
    } else {
        manifest.deps.push((name.to_string(), origem.to_string()));
    }
    let path = manifest_path(dir);
    std::fs::write(&path, render_manifest(&manifest))
        .map_err(|e| format!("nao foi possivel escrever `{}`: {e}", path.display()))?;
    install_packages(dir, &manifest)?;
    Ok(format!("dependencia `{name}` <- `{origem}` instalada"))
}

/// Instala (copia + trava hashes). Usado por `add` e `install`.
fn install_packages(dir: &Path, manifest: &Manifest) -> Result<Vec<LockedPackage>, String> {
    let mut locked = Vec::new();
    for (name, origem) in &manifest.deps {
        let src = dir.join(origem);
        if !src.exists() {
            return Err(format!(
                "dependencia `{name}` nao encontrada em `{}`",
                src.display()
            ));
        }
        let dest = packages_dir(dir).join(name);
        if src.is_dir() {
            copy_tree(&src, &dest)?;
        } else if src.is_file() {
            if dest.exists() {
                std::fs::remove_dir_all(&dest).ok();
                std::fs::remove_file(&dest).ok();
            }
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("nao foi possivel criar `{}`: {e}", parent.display()))?;
            }
            std::fs::copy(&src, &dest)
                .map_err(|e| format!("nao foi possivel copiar `{}`: {e}", src.display()))?;
        } else {
            return Err(format!(
                "dependencia `{name}` nao e arquivo nem diretorio: `{}`",
                src.display()
            ));
        }
        // Arquivos instalados (relativos a `pacotes/<nome>`).
        let files: Vec<(String, String)> = if src.is_dir() {
            collect_files(&dest)?
                .into_iter()
                .map(|rel| {
                    let bytes = std::fs::read(dest.join(&rel))
                        .map_err(|e| format!("nao foi possivel ler `{}`: {e}", rel.display()))?;
                    let slash = rel.to_string_lossy().replace('\\', "/");
                    Ok((slash, file_hash(&bytes)))
                })
                .collect::<Result<_, String>>()?
        } else {
            let bytes = std::fs::read(&dest)
                .map_err(|e| format!("nao foi possivel ler `{}`: {e}", dest.display()))?;
            let file_name = dest
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(name)
                .to_string();
            vec![(file_name, file_hash(&bytes))]
        };
        locked.push(LockedPackage {
            name: name.clone(),
            origem: origem.clone(),
            files,
        });
    }
    let lock = lock_path(dir);
    std::fs::write(&lock, render_lockfile(&locked))
        .map_err(|e| format!("nao foi possivel escrever `{}`: {e}", lock.display()))?;
    Ok(locked)
}

/// `rhp pkg install [--check]`: instala (ou so verifica contra o lock).
pub fn cmd_install(dir: &Path, check_only: bool) -> Result<String, String> {
    let (manifest, _) = read_manifest(dir)?;
    if manifest.deps.is_empty() {
        return Ok("sem dependencias (nada a fazer)".to_string());
    }
    if !check_only {
        let locked = install_packages(dir, &manifest)?;
        let total: usize = locked.iter().map(|p| p.files.len()).sum();
        return Ok(format!(
            "{} pacote(s), {} arquivo(s) em `pacotes/`",
            locked.len(),
            total
        ));
    }
    let lock = lock_path(dir);
    let text = std::fs::read_to_string(&lock).map_err(|_| {
        format!(
            "sem `rhp.lock` em `{}` (rode `rhp pkg install`)",
            dir.display()
        )
    })?;
    let origin = lock.display().to_string();
    let locked = parse_lockfile(&text, &origin)?;
    if locked.len() != manifest.deps.len() {
        return Err(format!(
            "`rhp.lock` dessincronizado ({} pacote(s), manifesto tem {}); rode `rhp pkg install`",
            locked.len(),
            manifest.deps.len()
        ));
    }
    for package in &locked {
        let manifest_origem = manifest
            .deps
            .iter()
            .find(|(n, _)| n == &package.name)
            .map(|(_, o)| o.clone())
            .ok_or_else(|| format!("pacote `{}` no lock mas fora do manifesto", package.name))?;
        if manifest_origem != package.origem {
            return Err(format!(
                "origem de `{}` mudou (`{}` -> `{manifest_origem}`); rode `rhp pkg install`",
                package.name, package.origem
            ));
        }
        let base = packages_dir(dir).join(&package.name);
        for (rel, hash) in &package.files {
            let bytes = std::fs::read(base.join(rel))
                .map_err(|_| format!("arquivo sumiu em `pacotes/`: `{}`", rel))?;
            if &file_hash(&bytes) != hash {
                return Err(format!("`{rel}` do pacote `{}` foi alterado", package.name));
            }
        }
    }
    Ok(format!(
        "{} pacote(s) conferidos contra `rhp.lock`",
        locked.len()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest_sample() -> &'static str {
        "# comentario\nnome = \"app\"\nversao = \"0.1.0\"\n\n[dependencias]\ntempo = \"../tempo\"\n"
    }

    #[test]
    fn parses_and_renders_manifest() {
        let manifest = parse_manifest(manifest_sample(), "<teste>").expect("parse deve passar");
        assert_eq!(manifest.name, "app");
        assert_eq!(
            manifest.deps,
            vec![("tempo".to_string(), "../tempo".to_string())]
        );
        // Roundtrip preserva os dados.
        let rendered = render_manifest(&manifest);
        let again = parse_manifest(&rendered, "<teste2>").expect("reparse deve passar");
        assert_eq!(again, manifest);
    }

    #[test]
    fn rejects_bad_manifests() {
        for bad in [
            "versao = \"0.1.0\"\n",                                         // sem nome
            "nome = \"App\"\nversao = \"1\"\n",                             // nome maiusculo
            "nome = \"a\"\nversao = \"1\"\n[chave]\n",                      // secao desconhecida
            "nome = \"a\"\nversao = \"1\"\n[dependencias]\ntempo = \"\"\n", // caminho vazio
            "nome = a\nversao = \"1\"\n",                                   // string sem aspas
            "nome = \"a",                                                   // string aberta
        ] {
            assert!(
                parse_manifest(bad, "<t>").is_err(),
                "deveria falhar: {bad:?}"
            );
        }
    }

    #[test]
    fn lockfile_roundtrips() {
        let packages = vec![LockedPackage {
            name: "tempo".to_string(),
            origem: "../tempo".to_string(),
            files: vec![("data.rhp".to_string(), file_hash(b"echo 1;\n"))],
        }];
        let text = render_lockfile(&packages);
        let back = parse_lockfile(&text, "<t>").expect("lock deve passar");
        assert_eq!(back, packages);
    }

    #[test]
    fn installs_and_verifies_tree() {
        let dir = std::env::temp_dir().join(format!("rhp-pkg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let lib = dir.join("libtempo");
        std::fs::create_dir_all(lib.join("sub")).expect("subdir");
        std::fs::write(lib.join("data.rhp"), "export let x = 1;\n").expect("escrever");
        std::fs::write(lib.join("sub").join("u.rhp"), "export let y = 2;\n").expect("escrever");
        let app = dir.join("app");
        std::fs::create_dir_all(&app).expect("appdir");
        assert!(cmd_init(&app).is_ok());
        assert!(cmd_add(&app, "tempo", "../libtempo").is_ok());
        // Instalado + lock gravado.
        assert!(packages_dir(&app).join("tempo/data.rhp").is_file());
        assert!(lock_path(&app).is_file());
        // Check passa; apos adulterar, falha.
        assert!(cmd_install(&app, true).is_ok());
        std::fs::write(packages_dir(&app).join("tempo/data.rhp"), "adulterado\n")
            .expect("adulterar");
        assert!(cmd_install(&app, true).is_err());
    }
}
