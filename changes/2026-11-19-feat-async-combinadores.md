# Added — Timeout, cancelamento cooperativo e combinadores async (RFC 0030)

- ID: 2026-11-19-feat-async-combinadores
- Data: 2026-11-19
- Versão: 0.5.0
- Tipo: Added
- Compatibilidade: Compatível (aditivo; worker com canal em vez de join)

## Resumo

Cinco builtins (ids 75–79): `await_timeout(h, s)`, `async_cancel(h)`,
`async_cancelled()`, `await_all([...])`, `await_race([...])`. Tarefas passam
a comunicar por canal (resultado) + flag atomica (cancelamento); thread
destacada apos enviar (sem `join`).

## Motivação

Item "cancel/timeout/pool" das limitacoes: sem prazo, sem cancelamento e sem
combinadores, operacoes lentas travavam o chamador (RFC 0030, nucleo).

## O que mudou

- Builtins 75–79 (nome, aridade, ids, teste anti-deriva estendido).
- `TaskThread { name, rx, cancel }`; `Vm.cancel_flag`; `spawn_task_thread`;
  `timeout_secs` (mesmos limites do `sleep`: 0–3600, finito).
- `await_timeout`: estouro = erro capturavel, tarefa segue no mapa (juntavel
  depois); `async_cancel` sinaliza; `async_cancelled` le a flag (fora de
  tarefa = `false`); `await_all` junta em ordem com falha rapida;
  `await_race` sonda (1ms) e devolve o primeiro pronto (panic conta como
  conclusao com erro; array vazio = erro).
- `async_join`/`await` preservados byte a byte nas mensagens (canal em vez
  de `join`).
- Docs: RFC 0030 (implementada, pool segue adiado), `LANGUAGE_SPEC.md` §10,
  `docs/REFERENCIA.md` (79 builtins), gramatica VSCode (nomes async).
- Testes: `nextgen_await_timeout` (inclui timeout-0 e erros),
  `nextgen_async_cancel`, `nextgen_await_all_race`.

## Detalhes técnicos

`crates/rhp-bytecode/src/lib.rs` (builtins/ids); `crates/rhp-vm/src/lib.rs`
+ `builtins.rs` (interceptacao); sem dependencias novas (std only).

## Compatibilidade e migração

Aditivo; nenhuma assinatura mudou. Pool (executor limitado) segue adiado
com justificativa (enfileirar mudaria `spawn` de nao-bloqueante para
bloqueante — exige RFC propria).

## Segurança e performance

Sem preempcao (cooperativo, sem corrupcao); timeout nao mata thread
(documentado; junte depois); sondagem do `race` dorme 1ms/rodada (sem hot
spin); 1 thread por tarefa (inalterado).

## Testes e validação

Integracao acima (margens folgadas, sem flaky: sleep 1s vs timeout 0.2s,
instantaneo vs 0.3s/0.1s); suite integral verde; `fmt`/`clippy` limpos.

## Limitações e riscos conhecidos

Sem pool/backpressure; `await_race` ocupado-aguarda com sleep de 1ms;
tarefas nunca juntadas vazam o handle ate o fim do programa (documentado:
sempre junte).

## Referências

RFC 0030 (aceita e implementada, nucleo); `docs/ASYNC.md`.
