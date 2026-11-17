# [Added] — Geradores lazy: `yield`, `next`/`exhausted`, `for-in` (RFC 0018, Etapa 5)

- ID: 2026-11-16-feat-geradores
- Data: 2026-11-16
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (aditivo; `yield` vira keyword em 0.x)

## Resumo

`fn g() { yield 1; }` não executa na chamada (devolve `Generator`
lazy); `next(g)` retoma, `exhausted(g)` testa o fim; `for x in g`
consome (duas formas compiladas, escolhidas por teste de tipo em
runtime); `return` descarta e encerra; `type()` = `"gerador"`.

## Motivação

Etapa 5, item "geradores" (RFC 0018, aceita sob autorização prévia):
iterar sem materializar (contadores, paginação, pipelines).

## O que mudou

- Lexer: `yield` keyword.
- Parser: `Stmt::Yield { expr }` (`yield`/`yield expr`; fora de funcao
  = erro de compilacao), `describe`, `parse_yield`.
- Bytecode: `Op::Yield` (sai do frame sem continuacao; validador como
  `Return`), `FuncDef.is_generator` (varredura do corpo por `Yield`;
  closures aninhadas compilam separado), `Value::Generator(GenHandle)`,
  `GenState{func, frame, done}` + `Frame`/`Handler`/`PendingHof` movidos
  da VM (suspensao por valor, sem registros nem vazamentos),
  `Builtin::{Next, Exhausted}` (ids 64/65; teste anti-deriva estendido),
  `CallMethod`/`NewMixed`/validador inalterados na forma.
- VM: `activate` lazy (empilha `Generator` em vez de frame);
  `gen_resume` (empilha suspenso/primeiro, arma `pending_gen` no frame;
  sem `avanco` do chamador — `Yield`/`Return` avancam); `Yield`
  (entrega + salva frame); `Return`-em-gerador (marca `done`, entrega
  `Null`, descarta retorno); `pending_gen` no frame DO GERADOR (morre
  com ele no unwind — mesmo padrao anti-stale das RFCs 0014/0017);
  `construtor novo` gerador = erro; callback de HOF gerador = erro;
  `Index` em gerador = erro; `StoreIndex` idem (catch-all);
  `type()`/`Display` (`<gerador f>`)/`==` por identidade/`heap`
  (com `try_borrow` anti-ciclo)/`json` (erro como funcao);
  `check_type` sempre falso p/ geradores (match reestruturado);
  `fuzz_op` cobre `Yield`/`BuildClass`/`NewMixed`/`CallMethod`.
- `for-in` duas formas: teste `type(H_iter) == "gerador"` (ops
  existentes) escolhe lazy (`next`/`exhausted`, `tmp` via `StoreName`
  para nao quebrar o rastreamento linear de escopos) ou eager
  (inalterada). `break`/`continue` funcionam nas duas (um `LoopCtx`,
  alvos por forma). Prologo custa type+eq+jump por laco (iteracao
  eager intacta — sem regressao mensuravel).
- Fmt: `yield`/`yield expr` (+ teste + idempotencia); Renamer: `Yield`.
- `LANGUAGE_SPEC.md` (§2 for + bloco geradores), `examples/geradores.rhp`
  (`0 1 2 / 0 1`, exit 0, em CI), `CHANGELOG.md`.

## Compatibilidade e migração

Aditivo; sem migração. `yield` reservada. `next`/`exhausted` novos
builtins (ids apos os existentes, sem renumerar). Comportamentos:
chamar gerador nunca executa (lazy); `return` descarta; `next` apos o
fim = erro; resume apos erro interno = erro ("crie outro"); metodo
destacado de gerador = lazy sem write-back; `this` em metodo-gerador
vinculado na criacao.

## Segurança e performance

Sem `unsafe`; sem panics novos (`expect` eliminado no resume;
`try_borrow` no heap; validador cobre `Yield`/`BuildClass`/`NewMixed`/
`CallMethod`/`JumpIfTrue`; fuzz dedicado com 8000+600 casos).
Performance: lazy = 1 ativacao por `next` + passos ate o `yield`;
for-in nao-gerador = +type/eq/jump no prologo ( suite verde;
bench sem alvo for-in: `while`/`push`/`fib` inalterados na faixa de
ruido da maquina compartilhada).

## Testes e validação

- `generators_yield_resume_and_finish` (lazy provado via `echo` de
  efeito lateral, `exhausted`, `Null` final descartado, `yield` vazio,
  `yield null`, `try` preserva mensagem).
- `generators_for_in_and_control_flow` (`break`/`continue`, parada
  antecipada = resto nao executado, vazio, infinito + `break`,
  reuso de esgotado = zero iteracoes).
- `generators_reject_misuse` (9 erros: `next`/`exhausted` em nao-gerador,
  `yield` fora de funcao (compilacao), construtor/metodo/HOF gerador,
  `this` solto, `this` descartado, reentrancia).
- `preserves_yield` (fmt); `cargo test --workspace`: **315 passed,
  0 failed**; `fmt --check` limpo; `clippy -D warnings` 0 warnings.
- `examples/geradores.rhp`: `0 1 2 / 0 1`, exit 0.

## Limitações e riscos conhecidos

- Sem `for` sobre gerador infinito sem `break` (trava como `while true`).
- `next` apos erro interno nao retoma (crie outro gerador).
- Frame suspenso conta na heap estimada (com teto anti-ciclo).
- `==` por identidade (dois geradores iguais nao sao `==`).
- Serializar gerador em JSON = erro (como funcao).

## Referências

- RFC 0018 (`RFCs/0018-geradores.md`, aceita e implementada);
- `MEMORY_MODEL.md` (por valor; frame suspenso e movido, dono unico).
