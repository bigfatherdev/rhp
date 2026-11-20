# [Fixed] — HOF com callback que lança corrompia estado posterior (RHP-AUD-IND-001)

- ID: 2026-11-20-fix-hof-stale-pending
- Data: 2026-11-20
- Versão: 0.6.2
- Tipo: Fixed (Security)
- Compatibilidade: Compatível

## Resumo

`map`/`filter`/`usort` cujo callback lançava (mesmo capturado) deixava o
`pending` do HOF armado; o próximo `Return` para aquele frame o consumia
como "retorno do callback": erro fantasma (ex.: `comparador de usort deve
retornar int` com código E errado) ou avanço espúrio com valor errado.

## Motivação

Auditoria independente pós-0.6.1: corrupção de estado determinística (P0),
invisível na suíte (nenhum teste combinava HOF+throw+return posterior).

## O que mudou

- `Frame.pending_cb` (índice do callback) + helper `push_hof_call`.
- `unwind` descarta pendings cujo callback foi truncado (`cb > depth`),
  preservando os vivos; `Return` descarta a amarra junto.
- Demais `pending_*` auditados: morrem com o frame (seguros por construção).

## Detalhes técnicos

Arquivos: `crates/rhp-bytecode/src/lib.rs` (campo), `crates/rhp-vm/src/lib.rs`
(helper + 5 armações + `Return` + `unwind` + regressão). Sem `unsafe`.

## Compatibilidade e migração

HOFs válidos inalterados. Migração: não necessária.

## Segurança e performance

Elimina erro fantasma/valor errado (+8 B/frame; O(1) por avanço). Bench sem
regressão atribuível.

## Testes e validação

- `runs_hof_throw_does_not_leak_stale_pending`: falha com o clear
  desativado, passa com ele; suíte 370/370; fmt/clippy limpos.
- Fuzz 1000 casos (seeds HOF/gerador/async) sem crash.

## Limitações e riscos conhecidos

`pending_gen` em unwinds exóticos sem evidência — sem mudança.

## Referências

- `audit/0.6.1/FINDINGS.md` (IND-001), `FIXES.md`, `SECURITY_AUDIT.md`.
