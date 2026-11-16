# RFC 0005 — HOFs com callback: `map`, `filter`, `usort`

- ID: RFC-0005
- Data: 2026-11-16
- Estado: aceita (programa RHP-100; implementada na Fase 2)
- Contexto: closures (RFC 0003) destravaram primeira-classe; falta o uso
  canônico — callbacks em funções de ordem superior (PHP: `array_map`,
  `array_filter`, `usort`). É a "prova do destravamento" prevista na RFC 0003 §9.

## 1. Problema

`sort(arr)` existe com ordem fixa, mas não há como passar função: sem
`map`/`filter`/`usort`, closures são valores sem ecossistema. E o obstáculo
técnico: `call_builtin` é síncrono (`Result<Value>`, sem acesso aos frames),
logo um builtin não consegue invocar closure — exige protocolo de
reentrância na VM.

## 2. Objetivo e não-objetivos

Objetivo: `map(arr, f)`, `filter(arr, f)`, `usort(arr, cmp)` (coleção
primeiro, convenção RHP como `push`/`sort`) com callbacks executados pela VM.
Não-objetivos: callbacks com aridade flexível (RHP exige aridade exata —
callback de `map`/`filter` declara 1 param, comparador 2); `use`/`this`;
ordenação paralela; `sort` com comparador (separado como no PHP).

## 3. Decisão de desenho

- Novos builtins (aridade 2, ids 50/51/52): `map`, `filter`, `usort`.
- **Protocolo de chamada pendente** (sem ops novos): `CallBuiltin` desses
  três é interceptado no `step` (nunca chega ao `call_builtin` síncrono);
  o frame chamador guarda `pending: Option<PendingHof>` e empilha a
  ativação; o gancho no `Op::Return` retoma (`advance_hof`): coleta o
  retorno e empilha a próxima chamada ou finaliza (empilha o resultado).
- `Map{func, items, next, acc}`: 1 chamada por elemento, resultado como está.
- `Filter{...}`: callback deve retornar `Bool` (sem truthiness — RHP não tem;
  divergência PHP documentada); senão erro de runtime.
- `Sort{func, sorted, i, j}`: insertion sort dirigido passo a passo
  (estável: só troca com `< 0`); comparador deve retornar `Int`, senão erro.
  O(n²) comparações no pior caso — assumido e documentado (v1).
- Array vazio retorna `[]` sem chamar. Aridade do callback é checada a cada
  chamada (erro claro com o nome). `throw`/`exit` no callback propagam
  normalmente (estado pendente é descartado com o frame). `max_steps` e
  `max_stack_depth` continuam valendo (tudo passa pelo `step`).
- Recursão aninhada (`map` dentro de callback) funciona (`pending` é por frame).

## 4. Alternativas consideradas

- Sub-VM fresca por callback: simples, mas quebra semântica (globais viram
  snapshot, `db_*` sem handles, `max_steps` burlável por callbacks). Rejeitada.
- Loop inline no compilador: `push` copia O(n) ⇒ O(n²) por `map`; rejeitada.
- `sort` sobrecarregado com comparador opcional: quebra aridade fixa
  checada em compilação; rejeitada (PHP também separa `sort`/`usort`).

## 5. Compatibilidade

Aditiva (3 builtins novos; `map`/`filter`/`usort` como *variáveis* continuam
legíveis — só a posição de chamada tem precedência, regra vigente).

## 6. Segurança

Sem `unsafe`; callbacks rodam no `step` (limites vigentes); erro de tipo do
callback vira `VmError` (capturável por `try`); sem Send novo.

## 7. Performance

Uma ativação por elemento/comparação (frames reais); `usort` O(n²)
comparações. Sem otimização nesta fase (documentado).

## 8. Plano de implementação

1. Bytecode: variantes + nomes + aridade + ids + teste anti-deriva.
2. VM: `Frame.pending`, `PendingHof`, `push_call_frame` (extraído de `call`),
   interceptação no `CallBuiltin`, gancho no `Return`, `advance_hof`
   (map/filter/usort), braços inalcançáveis em `call_builtin` (defesa).
3. Specs (`LANGUAGE_SPEC` tabela, `BYTECODE_SPEC`, `VM_SPEC`,
   `PHP_DIFFERENCES`: ordem de args, 1-arg, `Bool` estrito, estabilidade).
4. Testes e2e + erros + `changes/` + commit (map/filter, depois usort).

## 9. Plano de testes

- `map`: dobra, captura (closure com env), vazio, não-array, não-função,
  aridade do callback, `throw` no callback com `try` fora, aninhado.
- `filter`: pares, `Bool` estrito (retorno int = erro), vazio.
- `usort`: ordena, estável (empates mantêm ordem), comparador não-int erro,
  vazio/unitário, com `try` em volta de erro.
- Suite sem regressão; `fmt`/`clippy` limpos.

## 10. Migração e rollout

Nenhuma (aditivo). Rollout no commit da Fase 2.

## 11. Critérios de aceitação

- [ ] Testes da §9 passam; suite sem regressão.
- [ ] `fmt`/`clippy`/`test` + `check-changes.sh` verdes.
- [ ] Specs + divergências em `PHP_DIFFERENCES.md`.
- [ ] Esta RFC marcada `aceita` antes do merge.
