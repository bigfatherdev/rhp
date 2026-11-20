# BASELINE.md — Linha de base (independente, pós-0.6.1 lançado)

Data: 2026-11-20. Branch: `audit/independent-061` (sobre `main` = 0.6.1).
Binário: `rhp 0.6.1` (debug local; release para bench).

## Versão confirmada

- `rhp --version` → `rhp 0.6.1`; 7 `Cargo.toml` = 0.6.1; `Cargo.lock` sincronizado.
- Tag `v0.6.1` no remoto; imagens `0.6.1`/`0.6`/`0`/`latest` publicadas.

## Suíte inicial (sem alterações)

`cargo test --workspace`: **367 passed, 0 failed, 0 ignored** (~31 s).
`cargo clippy --workspace --all-targets`: 0 warnings. `cargo fmt --check`: limpo.
Postgres de teste ativo (`rhp-pg-test`); sem esse container, 1 teste falha
(dependência ambiental conhecida, não regressão).

## Cobertura prévia (0.6.x)

367 testes: VM 93 (incl. fuzz LCG bytecode), parser 53, bytecode 41, lexer 17,
CLI 43+79 integração (HTTP 15, pg live, Docker 5/5 via script) + conformidade.
Fuzzers internos: lexer (bytes), parser (tokens), bytecode (hostil).

## Áreas sem cobertura adequada (alvos desta auditoria)

REPL/LSP (smoke manual), `pkg`/`build` (parcial), cancel/timeout/race async
(parcial), TLS cliente, erros em DB transacional, HOF+erro+retorno posterior,
result-set parcial, métodos HTTP além de GET/POST, restart de servidor,
`db_exec(SELECT)`, FS rename/delete/mkdir (inexistentes), USAGE do CLI.
