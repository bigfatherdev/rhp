# Security — Remediação das 20 CVEs (auditoria + hardening Docker)

- ID: 2026-11-18-security-remediacao-20-cves
- Data: 2026-11-18
- Versão: unreleased
- Tipo: Security
- Compatibilidade: Compatível

## Resumo

Auditoria individual das 20 CVEs do escopo de segurança: 18 classificadas
como NÃO AFETADAS (com evidência por componente) e 2 MITIGADAS (SQLite
FTS5 via `libsqlite3` do sistema). `Dockerfile` (builder + runtime) passa
a executar `apt-get upgrade` para aplicar backports de segurança do Debian
no momento do build. Nenhuma semântica da linguagem, bytecode, VM ou
stdlib foi alterada.

## Motivação

Missão de segurança exigindo investigação, correção comprovada e evidência
para cada um dos 20 identificadores (SQLite, zlib, OpenSSL, util-linux,
PAM, glibc, dash, libgcrypt, APT, diffutils, rPath, GnuPG), nas superfícies
código Rust, FFI nativo, toolchain e imagem Docker.

## O que mudou

- `Dockerfile`: `apt-get upgrade -y --no-install-recommends` antes do
  `install` nos estágios `builder` e `runtime`.
- Novo `SECURITY_REMEDIATION.md`: inventário, análise por CVE (CWE, CVSS,
  causa-raiz, evidência de presença/ausência), testes e tabela-resumo.
- `CHANGELOG.md`: entrada `Security` em `[unreleased]`.

## Detalhes técnicos

- FFI nativo do RHP é só `libsqlite3` (`crates/rhp-vm/src/sqlite.rs`); API
  de sessão (`sqlite3changeset_*`) nunca é vinculada nem chamada — por isso
  CVE-2026-119379 é NÃO AFETADA no código. FTS5 é alcançável via SQL
  pass-through (`db_exec`/`db_query`) — por isso CVE-2026-119373/119374 são
  MITIGADAS via upgrade da lib do sistema (fix completo = SQLite ≥ 3.54.0
  no Debian).
- TLS é `rustls` puro (sem OpenSSL/QUIC); sem crates de zlib, libgcrypt,
  PAM, APT, dash, diffutils ou GnuPG; binário sem `RPATH`/`RUNPATH`, sem
  SUID, sem `RPATH $ORIGIN`. Detalhes e referências por CVE em
  `SECURITY_REMEDIATION.md`.

## Compatibilidade e migração

Compatível. Nenhuma migração necessária: mesma base `bookworm`, mesmo
usuário `rhp`, mesmos pacotes instalados (só atualizados). Rebuild da
imagem aplica os backports disponíveis.

## Segurança e performance

- Segurança: reduz a janela de exposição da imagem a backports Debian;
  nenhuma supressão adicionada; `cargo audit` sem novas vulnerabilidades
  Rust (só o aviso pré-existente `RUSTSEC-2025-0134`, sem fix disponível).
- Performance: sem impacto em runtime (mudança só no build da imagem);
  `upgrade` pode aumentar marginalmente o tempo de build Docker.

## Testes e validação

- `cargo test --workspace`: verde (0 falhas) antes e depois.
- `cargo clippy --workspace --all-targets -- -D warnings`: verde.
- `cargo fmt --all -- --check`: verde.
- `cargo audit`: 1 aviso permitido pré-existente (`RUSTSEC-2025-0134`).
- `cargo deny` / scanner de imagem / SBOM: não executados (ferramentas
  ausentes no ambiente) — registrados como limitação em
  `SECURITY_REMEDIATION.md`, não declarados como aprovados.

## Limitações e riscos conhecidos

- SQLite FTS5 (CVE-2026-119373/119374): eliminação total depende de backport
  Debian `bookworm` ≥ 3.54.0 ainda não publicado na data da auditoria;
  status MITIGADO, não CORRIGIDO. Recomendação: DBs só de origem confiável.
- Sem validação com Docker daemon nesta sessão; reexecutar build + scan no
  CI antes do release.

## Referências

- `SECURITY_REMEDIATION.md` (análise completa das 20 CVEs).
- NVD / CVE.org / OSV / advisories upstream citados por CVE no documento.
- Commits relacionados: alteração do `Dockerfile` nesta sessão.
