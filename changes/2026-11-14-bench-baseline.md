# [Added] — Benchmarks std-only + baseline (sem otimizações)

- ID: 2026-11-14-bench-baseline
- Data: 2026-11-14
- Versão: unreleased
- Tipo: Performance (medição; nenhuma mudança de código de produto)
- Compatibilidade: Compatível

## Resumo

Harness `cargo run --release -p rhp-cli --example bench_rhp` (só std) +
`docs/BENCHMARKS.md` com metodologia, baseline medido e análise de gargalos.
Nenhuma otimização aplicada — deliberadamente (sem necessidade provada;
`push` O(n²) é semântica documentada, mudar seria breaking).

## Motivação

Missão Fase 7: metodologia reproduzível antes de qualquer otimização.

## O que mudou

- `crates/rhp-cli/examples/bench_rhp.rs`: lex/parse/compile de 85 KB +
  `fib(20)` + `while` 200k + `push` 20k, com aquecimento e média/op.
- `docs/BENCHMARKS.md`: como rodar, tabela baseline, leitura dos gargalos,
  regras para otimizações futuras.

## Baseline (AMD EPYC, rustc 1.100.0, release, 2026-11-14)

lex 85 KB 0,873 ms · parse 5000 stmts 9,207 ms · compile 10,247 ms ·
fib(20) 99,328 ms · while 200k 370,917 ms · push 20k 10465,838 ms.

## Arquivos ou áreas afetadas

- `crates/rhp-cli/examples/bench_rhp.rs`, `docs/BENCHMARKS.md`

## Compatibilidade e migração

Compatível. Migração: não necessária.

## Segurança e performance

Este registro É a medição. Sem mudança de comportamento.

## Testes e validação

- Harness executa e imprime tabela (evidência acima); exemplo compila sob
  `cargo clippy --workspace --all-targets -- -D warnings`.
- `cargo test --workspace`: 120 testes, 0 falhas.

## Limitações e riscos conhecidos

Números valem para a máquina/modo citados; comparar só antes/depois iguais.
`push` O(n²) documentado como semântica (mudar exige RFC).

## Referências

- Auditoria: `reports/AUDIT_MASTER.md` (AUD-210).
