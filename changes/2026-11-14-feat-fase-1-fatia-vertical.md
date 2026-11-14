# [Added] — Fase 1: fundacao + primeira fatia vertical (`hello.rhp` end-to-end)

- ID: 2026-11-14-feat-fase-1-fatia-vertical
- Data: 2026-11-14
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (projeto greenfield; sem programas anteriores para quebrar)

## Resumo

Implementação inicial do RHP (Fase 1): workspace Rust com `rhp-lexer`,
`rhp-parser` (AST), `rhp-bytecode`, `rhp-vm` e `rhp-cli` (binário `rhp`),
mais `hello.rhp` (`echo "Hello from RHP!";`). `rhp run hello.rhp` imprime
exatamente `Hello from RHP!`. Documentação fundadora criada com somente o
que foi implementado.

## Motivação

Cumprir o prompt `RHP_PROMPT_INICIAL_v2.md` §5 (entrega da Fase 1): fundação
governada + fatia vertical verificável lexer → parser → bytecode → VM → CLI.

## O que mudou

- Workspace Cargo (edição 2021, toolchain stable 1.100.0, zero deps externas).
- `rhp-lexer`: `echo`, string `"..."` (escapes `\" \\ \n \r \t`), `;`; erros
  com linha:coluna (string não terminada, escape inválido, caractere/
  identificador inesperado).
- `rhp-parser`: AST `Program`/`Stmt::Echo`/`Expr::StringLiteral`; gramática
  `Program := Stmt*`, `Stmt := "echo" Expr ";"`; erro de `;` ausente etc.
- `rhp-bytecode`: `Value` (`Null`, `Bool`, `Int`, `Float`, `Str`), `Op`
  (`Constant(u32)`, `Print`, `Halt`), `Chunk`, `compile()`, `validate()`.
- `rhp-vm`: VM em pilha, validação antes de executar, `Print` escreve
  `Display + "\n"`, `Halt` encerra; `VmError` sem panics.
- `rhp-cli`: `rhp run <arquivo.rhp>`, `--help`/`--version`; stderr
  `arquivo:linha:coluna: msg`; códigos 0 ok / 1 erro / 2 uso.
- Docs: `GOVERNANCE.md` (cópia byte-igual de `RHP_GOVERNANCE.md`), `AGENTS.md`,
  `ARCHITECTURE.md`, `ROADMAP.md`, `CHANGELOG.md`, `changes/README.md`,
  `LANGUAGE_SPEC.md`, `BYTECODE_SPEC.md`, `VM_SPEC.md` (só implementado),
  `MEMORY_MODEL.md` e `PHP_DIFFERENCES.md` (estrutura + "A definir").

## Detalhes técnicos

- Sem geradores de parser; lexer/parser escritos à mão; sem `unsafe`;
  sem `unwrap()`/`expect()` em produção (só em `#[cfg(test)]` e teste de
  integração); erros próprios por crate (`LexError`, `ParseError`,
  `CompileError`, `VmError`).
- Compilação: cada `echo "s";` → `Constant(Str)`, `Print`; final `Halt`;
  programa vazio → só `Halt`.
- Proposta (não implementada) de CI de `changes/` registrada em
  `ARCHITECTURE.md`, conforme exigido para a Fase 1.

## Arquivos ou áreas afetadas

- `Cargo.toml`, `hello.rhp`
- `crates/rhp-lexer/Cargo.toml`, `crates/rhp-lexer/src/lib.rs`
- `crates/rhp-parser/Cargo.toml`, `crates/rhp-parser/src/lib.rs`
- `crates/rhp-bytecode/Cargo.toml`, `crates/rhp-bytecode/src/lib.rs`
- `crates/rhp-vm/Cargo.toml`, `crates/rhp-vm/src/lib.rs`
- `crates/rhp-cli/Cargo.toml`, `crates/rhp-cli/src/main.rs`,
  `crates/rhp-cli/tests/integration.rs`
- `GOVERNANCE.md`, `AGENTS.md`, `ARCHITECTURE.md`, `ROADMAP.md`,
  `CHANGELOG.md`, `LANGUAGE_SPEC.md`, `BYTECODE_SPEC.md`, `VM_SPEC.md`,
  `MEMORY_MODEL.md`, `PHP_DIFFERENCES.md`, `changes/README.md`

## Compatibilidade e migração

Compatível (greenfield). Migração: não necessária.

## Segurança e performance

- Segurança: VM valida o chunk antes de executar; bytecode tratado como
  entrada não confiável; sem `unsafe`; sem execução de comandos externos;
  sem segredos no repo. Nenhum audit/fuzz executado (fora do escopo da Fase 1).
- Performance: não medida (sem baseline; sem alegações).

## Testes e validação

Todos executados em 2026-11-14, Ubuntu 26.04, Rust stable 1.100.0:

- `cargo fmt --check`: passou (`FMT_OK`).
- `cargo clippy --workspace --all-targets -- -D warnings`: passou após
  correção (`writeln!` em `rhp-vm`); resultado final `CLIPPY_OK`.
- `cargo test --workspace`: 31 testes, todos passaram, 0 falharam:
  `rhp-lexer` 8, `rhp-parser` 7, `rhp-bytecode` 5, `rhp-vm` 5, `rhp-cli`
  4 unit + 2 integração (inclui `rhp run hello.rhp` == `Hello from RHP!\n`
  e rejeição de `;` ausente).
- Manual: `./target/debug/rhp run hello.rhp` → `Hello from RHP!\n`
  (confirmado byte a byte via `od -c`); casos de erro cobertos:
  string não terminada, escape inválido, `;` ausente, expressão ausente,
  bytecode inválido/sem `Halt`, `Print` com pilha vazia.

## Limitações e riscos conhecidos

- Linguagem mínima: só `echo` de literal de string; resto é "não
  especificado ainda" (ver specs).
- Sem CI de `changes/`, sem fuzz/property tests, sem testes Windows,
  sem benchmarks — planejado para fases seguintes.
- Diretório ainda não é repositório Git (`git status` informa
  "not a git repository"); nenhum commit/push realizado (sem autorização).

## Referências

- Prompt: `RHP_PROMPT_INICIAL_v2.md`
- Governança: `GOVERNANCE.md` (copiado de `RHP_GOVERNANCE.md`)
- Sem RFC/ADR/commit (Fase 1 inicial; decisões do prompt registradas em
  `ARCHITECTURE.md`).
