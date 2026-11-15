# [Added] — CP6: `rhp check`, JSON de diagnósticos e DevTools

- ID: 2026-11-14-feat-devtools-check
- Data: 2026-11-14
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (comandos e arquivos novos; `run`/`fmt` inalterados)

## Resumo

`rhp check [--json] <arquivo>` (lex+imports+compile sem executar, com
relatório máquina-legível); gramática TextMate versionada com teste de
consistência; `docs/DEBUGGER_DESIGN.md` (desenho, sem implementação);
`LoadError` estruturado com `Box` (clippy `result_large_err` resolvido por
arquitetura, não por `allow`).

## Motivação

Missão Next Generation CP6 (NG-201 parcial, NG-301): substrato para
editores/LSP + verificação sem execução (também pedida em §9 da missão
anterior). Sem debugger improvisado, sem comandos-stub.

## O que mudou

- `rhp-cli`: `check` humano + `--json`; `LoadError::{Rich(Box), Plain}`;
  `json_escape` manual; USAGE e `--help` atualizados.
- `editors/vscode/`: `package.json`, `language-configuration.json`,
  `syntaxes/rhp.tmLanguage.json` (keywords, builtins, strings/escapes,
  comentários, números, chamadas, definições).
- `docs/DEBUGGER_DESIGN.md`, `docs/CLI_REFERENCE.md`, `ARCHITECTURE.md`,
  ADR 0010.
- Testes: `check` ok/falha, `--json` válido com `code`, gramática cobre
  keywords+builtins.

## Arquivos ou áreas afetadas

- `crates/rhp-cli/{src/main.rs,tests/integration.rs,Cargo.toml}`, `editors/`,
  `docs/{CLI_REFERENCE,DEBUGGER_DESIGN}.md`, `ARCHITECTURE.md`,
  `ADRs/0010-devtools-check-grammar.md`.

## Compatibilidade e migração

Compatível. Migração: não necessária (`check` é aditivo).

## Segurança e performance

`check` não executa código (só lex/parse/compile). JSON manual sem
dependências; escaping total de controle.

## Testes e validação

- `cargo test --workspace`: 141 testes, 0 falhas (15 de integração,
  incluindo `check`, `--json` e gramática).
- `cargo fmt --check` ok; `cargo clippy --workspace --all-targets -- -D warnings` ok.

## Limitações e riscos conhecidos

Gramática sem engine de teste (consistência textual); JSON sem schema
formal (documentado pelo exemplo); LSP/debugger só desenhados.

## Referências

- Auditoria: `reports/NEXT_GENERATION_AUDIT.md` (NG-201 parcial, NG-301).
  ADR: `ADRs/0010-devtools-check-grammar.md`.
