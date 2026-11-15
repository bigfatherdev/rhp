# [Added] — Robustez: corpus, property tests, exemplos e CI

- ID: 2026-11-14-test-robustez
- Data: 2026-11-14
- Versão: unreleased
- Tipo: Added (testes e ferramenta; zero mudança de runtime)
- Compatibilidade: Compatível

## Resumo

Corpus malformado (lexer/parser nunca entram em pânico), property test
aritmético com PRNG determinístico (200 expressões vs referência i64),
verificação de todos os `examples/*.rhp` e `check-changes.sh` com renames
+ nota de `fetch-depth`.

## Motivação

Auditoria AUD-211/212/213: sem fuzz/property, exemplos sem verificação
nomeada, script de CI com brechas.

## O que mudou

- `rhp-lexer`/`rhp-parser`: testes `malformed_inputs_never_panic` (21 + 27
  entradas; só `Ok`/`Err`, nunca panic).
- `rhp-vm`: `property_arithmetic_matches_reference` (LCG próprio, seed fixa,
  `+`/`*` sobre dígitos, profundidade 4).
- `rhp-cli` integração: `rhp_run_all_examples_succeed` (todos os
  `examples/*.rhp` com exit 0; `fib.rhp` pinado em `55`).
- `tools/check-changes.sh`: `--diff-filter=AR` + doc de `fetch-depth: 0`.

## Arquivos ou áreas afetadas

- Testes em 3 crates + 1 integração; `tools/check-changes.sh`.

## Compatibilidade e migração

Compatível (só testes/ferramenta). Migração: não necessária.

## Segurança e performance

Property test roda em ~ms; corpus idem. Sem dependências novas.

## Testes e validação

- `cargo test --workspace`: 120 testes, 0 falhas (inclui os novos).
- Script revalidado nos cenários anteriores (mantidos) + rename coberto.
- `cargo fmt --check` ok; `cargo clippy ... -D warnings` ok.

## Limitações e riscos conhecidos

Corpus é finito (não substitui fuzz contínuo); gerador cobre só `+`/`*`
de inteiros pequenos (sem divisão/estouro por desenho).

## Referências

- Auditoria: `reports/AUDIT_MASTER.md` (AUD-211–213).
