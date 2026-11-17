# RFC 0018 — Geradores (`yield`, lazy) (Etapa 5)

- ID: RFC-0018
- Data: 2026-11-16
- Estado: aceita (autorização prévia do programa Etapas 3–5) e implementada
- Contexto: Etapa 5, item "geradores". Iteração lazy sem coleção
  intermediária; `for x in gen()` funciona como em arrays.

## 1. Problema

Sequências sob demanda (contadores, paginação, pipelines) hoje exigem
materializar arrays. Falta suspensão/retomada com estado.

## 2. Objetivo e não-objetivos

Objetivo: `yield v;` em `fn` torna a chamada lazy (`Value::Generator`);
`next(gen)`, `exhausted(gen)`; `for x in gen` consome (duas formas de
loop, escolhidas em runtime por teste de tipo com ops existentes);
`return` em gerador descarta e encerra (Null apos fim).

Não-objetivos: `yield from`/delegação, `send()`/bidirecional,
geradores infinitos com `len()`, serialização de gerador.

## 3. Proposta

- `FuncDef.is_generator` (chunk com `Yield`); chamar NÃO executa:
  devolve `Generator{func, frame: Some(quadro ativado), done: false}`.
- `next(gen)` (builtin interceptado como HOFs): empilha o frame salvo
  (ou ativa), arma `pending_gen` nele; `Yield(v)` salva o frame em
  `gen`, entrega `v` ao chamador; `Return`/fim marca `done`
  (valor descartado); erro no meio aborta o resume (estado = ultimo
  `yield`; `pending_gen` morre com o frame no unwind).
- `exhausted(gen)` builtin puro (sem frames): `done`.
- `for x in ITER`: testa `type(ITER) == "gerador"` (ops existentes) e
  escolhe forma lazy (`while !exhausted + next`, corpo compilado 2x)
  ou indexada (inalterada). `break`/`continue` funcionam nas duas
  (mesmo `LoopCtx`, alvos por forma).
- `this` em métodos-geradores: vinculado na criação (sem write-back;
  como método destacado).

## 4. Alternativas

- `next` retornando Null no fim (sentinela): quebra `yield null`
  explícito — rejeitada (erro `gerador esgotado` + `exhausted()`).
- IterList drenando gerador: trava em infinitos — rejeitada.
- `for-in` só arrays (while+next documentado): incompleto para o
  caso de uso principal — rejeitada (duas formas implementadas).

## 5. Compatibilidade

Aditivo: `yield`/`next`/`exhausted` novos (`yield` como keyword? `yield`
e contextual? — DECIDIDO: keyword, precedente `match`; `next` e
`exhausted` sao builtins). `for-in` existente inalterado para
array/mapa (mesmo bytecode quando ITER e literal? Nao: o teste de
tipo e emitido sempre — custo de 1 `type()` + comparacao por laco;
documentado; bench sem geradores mede o delta).

## 6. Segurança

Sem `unsafe`; `Rc<RefCell>` so para estado do gerador (frames movidos,
sem borrow cruzado); unwind limpa `pending_gen`; `for` lazy nao aloca
lista. Erros em vez de panic em todos os caminhos.

## 7. Performance

`next()`: 1 ativacao + passos ate o `yield` (sem overhead fora disso).
`for-in` nao-gerador: +`type()`+`==`+salto por laco (medir no bench;
aceitavel se <2%).

## 8. Plano de implementação

1. Lexer/parser: `yield` keyword, `parse` (só em funcao? checar no
   `compile`: `Yield` fora de funcao = erro).
2. Bytecode: `Op::Yield`, `FuncDef.is_generator`, validador, `Index`
   inalterado (gerador nao indexavel — erro claro).
3. VM: `Generator`, `next` interceptado, `exhausted` builtin,
   `pending_gen`, `Return`/`Yield`, unwind, `type()` =
   `"gerador"`, `Display`, `==` (identidade? estrutural impossivel —
   DECIDIDO: identidade por `Rc::ptr_eq` + done igual).
4. `for-in` duas formas; fmt (`yield`, `for` inalterado); exemplo.
5. Testes + spec + `changes/`.

## 9. Plano de testes

- Lazy real (corpo nao executa na chamada; prova via efeito lateral).
- `next`/`exhausted`, fim (erro em `next` apos fim), `yield null`,
  `return` descarta, params/defaults, `this` em metodo-gerador.
- `for-in` em gerador (incl. `break`/`continue`), finito e com parada
  antecipada (restante nao executado); finito vazio.
- Erro dentro do gerador; `try` externo captura; estado = ultimo yield.
- Suite verde; `fmt` idempotente.

## 10. Migração e rollout

Nenhuma migração (aditivo). `yield` vira keyword (0.x).

## 11. Critérios de aceitação

- [x] RFC aceita antes do código (autorização prévia 2026-11-16).
- [ ] Lazy comprovado + `for-in` nas duas formas + 8+ erros pinados.
- [ ] Suite verde; bench delta do `for` documentado.
