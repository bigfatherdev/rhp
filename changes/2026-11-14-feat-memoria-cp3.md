# [Added] — CP3: modelo de memoria, dedup e interning comprovados

- ID: 2026-11-14-feat-memoria-cp3
- Data: 2026-11-14
- Versão: unreleased
- Tipo: Added (documentacao + micro-otimizacao sem efeito observavel)
- Compatibilidade: Compatível (semantica identica; prova: suite + teste de pool)

## Resumo

Investigacao do modelo de memoria (missao §4): medicoes, comparacao de
CoW/RC/GC/hibridos, decisao de manter valor por copia + dedup de constantes
escalares e interning O(1) de nomes. `reports/MEMORY_MODEL.md` novo;
`MEMORY_MODEL.md` raiz atualizado (era stub da Fase 1).

## Motivação

Checkpoint 3 da missao Next Generation. Medir antes de otimizar; nada de
CoW/GC sem RFC.

## O que mudou

- `rhp-bytecode`: `ConstKey` + mapas de dedup (Str/Int/Float-bits/Bool/Null;
  compostos/funcoes fora); `intern_name` via `HashMap`.
- Teste: 2000 literais iguais => pool com 1 entrada.
- `reports/MEMORY_MODEL.md` (decisao + alternativas descartadas);
  `MEMORY_MODEL.md` (modelo real); nota de tradeoff em `docs/BENCHMARKS.md`.

## Arquivos ou áreas afetadas

- `crates/rhp-bytecode/src/lib.rs` (+teste), `reports/MEMORY_MODEL.md`,
  `MEMORY_MODEL.md`, `docs/BENCHMARKS.md`.

## Compatibilidade e migração

Compatível. `push` O(n²) mantido por ser semantica (mudar exige RFC).
Migração: não necessária.

## Segurança e performance

 Medicoes (release, AMD EPYC compartilhado): pool 15003 -> 5 entradas;
 `compile` 10,2 -> ~13,5 ms sob variancia (hash por literal); demais
 benches dentro da variancia. Tradeoff documentado como aceito.

## Testes e validação

- Falha-antes: teste de pool falhou (2000 entradas).
- `cargo test --workspace`: todos os testes verdes, 0 falhas
  (+1 teste de pool: 2000 literais iguais => 1 entrada).
- `cargo fmt --check` ok; `cargo clippy ... -D warnings` ok.

## Limitações e riscos conhecidos

CoW/arena/GC adiados com justificativa; sem teto geral de heap
(documentado em `SECURITY.md`).

## Referências

- Missao §4; auditoria NG-101/NG-102. Sem ADR nova (decisao cabe em
  `reports/MEMORY_MODEL.md` + ADR 0002 vigente).
