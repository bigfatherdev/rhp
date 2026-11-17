# [Added] — Tarefas assíncronas `async_spawn`/`async_join` (RFC 0019, Etapa 5)

- ID: 2026-11-16-feat-async-spawn-join
- Data: 2026-11-16
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (2 builtins novos, ids 66/67, sem renumerar)

## Resumo

`async_spawn("f")` roda `fn` de nível superior sem captura em thread
própria (recompila o programa, como `http_serve`) e devolve handle
`Int` imediatamente; `async_join(h)` bloqueia e devolve o retorno
(desserializado de JSON). Isolamento total (sem estado compartilhado);
erros da tarefa viram erro no `join`.

## Motivação

Etapa 5, item "async" (RFC 0019, aceita sob autorização prévia):
paralelismo real para operações lentas (2× `sleep(1)` em ~1s, não ~2s).

## O que mudou

- Builtins `AsyncSpawn`/`AsyncJoin` (ids 64/65 eram `next`/`exhausted`;
  novos = 66/67; teste anti-deriva estendido; aridade 1).
- `Vm.tasks` (`TaskHandles`, como `DbHandles`; `reset` descarta).
- Interceptados no `step_inner` (precisam da VM: programa + tabela),
  como HOFs (nunca passam pelo `call_builtin` sincrono; arm defensivo
  la retorna erro interno).
- Worker (`run_task`): recompila AST owned, acha `fn` por nome,
  valida (nao-gerador; env e sempre vazio no worker — validacao real
  no spawn), `push_call_frame` + `drive` sobre frame raiz com `Halt`
  (sem o qual `return` pareceria fora de funcao), `stringify_json`.
- `async_begin` (spawn: valida nome/funcao/gerador/captura, thread,
  handle) e `async_join` (one-shot; `join` bloqueia; `parse_json` do
  retorno; erro => `tarefa ... falhou: ...`).
- Funcional e `fn` (def) sem captura no spawn; closure/metodo-gerador
  rejeitados (`env` nao vazio / `is_generator`); `next` em gerador
  esgotado continua erro; HOFs rejeitam callback gerador (explicito).
- `LANGUAGE_SPEC.md` (§6: 4 linhas + `sleep` atualizada), `examples/async.rhp`
  (`42 lenta`, ~1s, em CI), `CHANGELOG.md`.

## Compatibilidade e migração

Aditivo; sem migração. Zero-arg (follow-up documentado: args via JSON).
`join` duplo = erro; tarefa sem `join` = thread detached ao fim
(documentado; `reset` descarta handles).

## Segurança e performance

Threads so executam codigo do proprio programa (recompilado e
validado). Sem estado compartilhado (sem data race por construcao;
fronteira JSON `String: Send`). `join` bloqueia a chamadora (sem
deadlock: one-shot, sem ciclos). Custo = 1 thread + 1 compilacao +
JSON ida/volta; ganho medido: 2x1s em ~1s (serial = ~2s).

## Testes e validação

- `tasks_run_in_parallel_and_join` (VM): resultado, paralelismo
  (200ms em < 350ms), erro->join, handle invalido/duplo, 4 validacoes
  de spawn, isolamento de global (com helper `run_with_program`, pois
  `run_src` nao define programa — a CLI sempre define).
- `cargo test --workspace`: 318 passed, 0 failed; `fmt --check` limpo;
  `clippy -D warnings` 0 warnings.
- `examples/async.rhp`: `42 lenta`, ~1s, exit 0.

## Limitações e riscos conhecidos

- Zero-arg (args = follow-up; documentado na RFC).
- Retorno restrito a JSON (func/classe/instancia/gerador = erro).
- Sem cancelamento/timeout de join; sem `await` sintatico (deliberado).
- Threads detached se o programa termina sem `join` (documentado).

## Referências

- RFC 0019 (`RFCs/0019-async-spawn-join.md`, aceita e implementada);
- `http_serve` (padrao de recompilacao por thread).
