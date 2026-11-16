# [Added] — HOFs com callback: `map`, `filter`, `usort` (RFC 0005, Fase 2)

- ID: 2026-11-16-feat-hofs-callback
- Data: 2026-11-16
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível

## Resumo
`map(arr, f)`, `filter(arr, f)` e `usort(arr, cmp)` (coleção primeiro,
convenção RHP): callbacks executados pela VM via protocolo de chamada
pendente (`Frame.pending` + retomada no `Return`). `usort` estável.

## Motivação
Prova do destravamento da RFC 0003 §9: primeira-classe sem ecossistema não
se paga. Segunda fase da leva aprovada (Fase 2 do programa RHP-100).
Branch: `feat/php-100-fase2`.

## O que mudou
- Bytecode: `Builtin::{Map, Filter, Usort}` (ids 50/51/52), nomes, aridade 2,
  `resolve`/`From`, teste anti-deriva estendido. Sem ops novos.
- VM: `PendingHof::{Map, Filter, Sort}`, `Frame.pending`,
  `push_call_frame` extraído de `call()` (sem avanço de `ip`; chamadores
  avançam exatamente uma vez — correção de bug achado em smoke: HOF
  avançava o `ip` a cada callback e saltava instrução), `hof_begin`,
  `advance_hof` (insertion sort passo a passo, estável, `< 0` troca),
  interceptação no `CallBuiltin`, retomada no `Return`. Braços defensivos
  em `call_builtin` (inalcançáveis via `step`).
- Regras: callback de `map`/`filter` declara 1 param; comparador 2 params
  com retorno `Int` (senão erro); `filter` exige `Bool` (sem truthiness);
  array vazio/unitário não chama; `throw`/`exit` no callback propagam
  (estado pendente descartado com o frame); `max_steps`/limites vigentes;
  aninhamento (`map` em callback) funciona (`pending` por frame).
- Specs: `LANGUAGE_SPEC.md` (tabela), `BYTECODE_SPEC.md` (ids 50–52 +
  protocolo), `VM_SPEC.md` (pendente/retomada), `PHP_DIFFERENCES.md`
  (nomes curtos, coleção primeiro, 1-arg, `Bool` estrito, estabilidade).

## Detalhes técnicos
- Alternativas rejeitadas na RFC 0005: sub-VM fresca por callback (quebra
  semântica de globais, `db_*` sem handles, `max_steps` burlável por volume);
  loop inline no compilador (`push` copia O(n) ⇒ O(n²)); sobrecarregar
  `sort` (quebra aridade fixa checada em compilação).
- `usort` O(n²) comparações no pior caso (assumido, v1 documentado).
- Arquivos: `crates/rhp-bytecode/src/lib.rs`, `crates/rhp-vm/src/lib.rs`,
  `crates/rhp-vm/src/builtins.rs`, `RFCs/0005-hofs-callback.md` (+ specs).

## Compatibilidade e migração
Aditivo (3 builtins; `map`/`filter`/`usort` como variáveis continuam
legíveis — só a posição de chamada tem precedência, regra vigente).
Nenhuma migração.

## Segurança e performance
- Sem `unsafe`; callbacks rodam no `step` (limites vigentes); erro de tipo
  do callback vira `VmError` (capturável por `try`); sem `Send` novo.
- Uma ativação por elemento/comparação; sem medição de bench dedicada.

## Testes e validação
- `cargo fmt --check`: limpo. `cargo clippy --workspace --all-targets`:
  0 warnings/errors.
- `cargo test --workspace`: 236 passed, 0 failed (232 + 4 novos).
- Novos: bytecode `compiles_hofs_with_callback` (ids 50/51/52, aridade);
  VM `runs_map_with_callback` (dobra, captura, vazio, aninhado, erros de
  tipo/aridade, `throw` com `try`), `runs_filter_with_callback` (pares,
  vazio, `Bool` estrito), `runs_usort_with_comparator` (ordem, reverso,
  estabilidade, vazio/unitário, comparador não-int, não-array).
- Smoke manual: estabilidade de empates, string sort, 9-elementos, `throw`
  capturado, aninhado, `max`? não — limites cobertos pela suíte vigente.

## Limitações e riscos conhecidos
- `usort` O(n²); sem comparador parcial/pré-decorado (futuro).
- Callback com aridade diferente = erro de runtime (RHP exige aridade exata).
- Sem `sort` com comparador (separado como no PHP, por decisão).

## Referências
- `RFCs/0005-hofs-callback.md` (aceita)
- Branch `feat/php-100-fase2`
