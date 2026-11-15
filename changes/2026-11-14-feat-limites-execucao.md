# [Added] — Limites de execução e módulos confinados

- ID: 2026-11-14-feat-limites-execucao
- Data: 2026-11-14
- Versão: unreleased
- Tipo: Added (limites opt-in) + Changed (regras de `import`)
- Compatibilidade: Compatível para programas válidos; `import` aninhado e
  fora da árvore (antes silenciosos) passam a erro documentado

## Resumo

`VmLimits::max_steps` opt-in (+ flag `rhp run --max-steps N`); `import`
confinado à subárvore do programa; teto de fonte 8 MiB; `import` só no
nível superior.

## Motivação

Auditoria AUD-205/206/207: loops sem teto, leitura fora da árvore,
hoisting surpreendente de imports aninhados.

## O que mudou

- `rhp-vm`: `VmLimits { max_steps }` (padrão ilimitado), contador por
  `step()`, erro `limite de instrucoes excedido`.
- `rhp-cli`: `--max-steps N`; loader com raiz canonicalizada
  (`starts_with`, erro fora-da-arvore), `MAX_SOURCE_BYTES`, rejeição de
  `import` aninhado com instrução de correção.
- Specs: `VM_SPEC.md` (limites), `LANGUAGE_SPEC.md` (jaula, topo, tetos),
  `docs/CLI_REFERENCE.md`.

## Arquivos ou áreas afetadas

- `crates/rhp-vm/src/lib.rs`, `crates/rhp-cli/src/main.rs` (+testes),
  `crates/rhp-cli/tests/integration.rs`, specs, `ADRs/0006-limites-execucao.md`

## Compatibilidade e migração

Opt-in não muda padrão. Imports aninhados/fora-da-árvore: mover para o
nível superior / para dentro da árvore.

## Segurança e performance

Contador u64 por instrução (custo desprezível). Sem heap nova no caminho
quente além do erro formatado.

## Testes e validação

- Falha-antes: loop com limite passava; `../fora.rhp` carregava; import
  aninhado passava; arquivo de 16 MiB lia.
- `cargo test --workspace`: 120 testes, 0 falhas (4 testes novos + 1 integração).
- `cargo fmt --check` ok; `cargo clippy ... -D warnings` ok.

## Limitações e riscos conhecidos

Pilha de operandos/heap sem teto além destes pontos; DNS sem timeout
(documentado); sem limite de taxa no HTTP.

## Referências

- Auditoria: `reports/AUDIT_MASTER.md` (AUD-205–207). ADR: `ADRs/0006-limites-execucao.md`.
