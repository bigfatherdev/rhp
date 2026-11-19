# RFC 0030 (proposta) — Cancelamento, timeout e combinadores async

- ID: RFC 0030
- Status: aceita e implementada (nucleo; thread-pool segue adiado)
- Data: 2026-11-19
- Versao: unreleased (0.5.0 planejada)
- Depende de: RFC 0023 (async/await) + item 1 (captura/codigo compartilhado)

## 1. Problema

Tarefas de hoje so terminam por conclusao/erro (`await`/`async_join`
bloqueiam ate o fim). Sem `timeout`, sem cancelamento cooperativo e sem
combinadores (`all`/`race`), operacoes lentas travam o chamador e N tarefas
alternativas nao sao redutiveis a primeira pronta.

## 2. Objetivo e nao-objetivos

- Objetivo: `await_timeout(h, s)`, cancelamento cooperativo
  (`async_cancel(h)` + `async_cancelled()` na tarefa) e `async_all([...])` /
  `async_race([...])`, mantendo o modelo de threads isoladas.
- Nao-objetivos: preempcao/interrupcao de thread, `select` generico,
  thread-pool, backpressure automatica.

## 3. Proposta (implementada: ids de builtin 75–79; sem thread-pool)

```rhp
async fn lenta() { sleep(10); return "demorou"; }
let h = lenta();
try {
  echo await_timeout(h, 1);   // apos 1s: erro `timeout`
} catch e {
  echo "tempo esgotado";
}
echo await h; // a tarefa seguiu em voo; junta aqui ("demorou")

async fn trabalho() {
  let i = 0;
  while i < 1000000 {
    if async_cancelled() { return "cancelado"; }
    i = i + 1;
  }
  return "ok";
}
let h2 = trabalho();
async_cancel(h2);
echo await h2; // "cancelado"

let r = await_all([buscarA(), buscarB()]); // array de retornos, na ordem
let primeiro = await_race([buscarA(), buscarB()]); // primeiro pronto
```

- Semantica: `await_timeout` junta com prazo (estouro = erro capturavel,
  tarefa segue destacada); `async_cancel` so sinaliza (flag atomica por
  handle; a tarefa precisa sondar `async_cancelled()`); `await_all`
  junta em ordem (primeiro erro aborta o resultado, demais seguem
  destacadas); `await_race` devolve o primeiro (demais seguem destacadas).
- Handles one-shot (como `async_join`): reusar apos join = erro.

## 4. Alternativas consideradas

- Matar thread (`pthread_cancel`/panico injetado): rejeitado (Rust nao
  permite com seguranca; `unsafe` proibido; estado inconsistente).
- Executor com poll/wakeup: adiado (reescrita da VM; fora do escopo v1).

## 5. Compatibilidade

Aditivo (builtins/funcoes novas; `await`/`async_join` intactos). Handles
continuam one-shot.

## 6. Seguranca

Sem preempcao: cancelamento so cooperativo (sem corrupcao de estado).
Timeout nao mata a thread (destacada; documentar custo). Limites de
tarefas: documentar `join` pronto (sem spawn ilimitado).

## 7. Performance

Uma thread por tarefa (modelo vigente); `race`/`all` nao criam threads
extras. Sondagem `async_cancelled()` O(1) (flag).

## 8. Plano de implementacao

VM (`TaskHandles` + flag atomica, `AwaitTimeout`, combinadores no `step`),
builtins ou funcoes nativas, testes de tempo (flaky-free: prazos folgados),
docs de padroes (timeout + retry + cancel).

## 9. Plano de testes

Timeout estoura e e capturavel (e a tarefa segue juntavel); timeout 0 em
tarefa pronta devolve na hora; cancel cooperativo retorna; `async_cancelled`
fora de tarefa = `false`; `all` ordena + falha rapida; `race` pega o
primeiro pronto; handle reusado/inexistente = erro; args invalidos = erro.
Implementado: `nextgen_await_timeout`, `nextgen_async_cancel`,
`nextgen_await_all_race` (margens folgadas, sem flaky).

## 10. Migracao e rollout

Opcional; padroes novos nao afetam codigo existente. `await`/`async_join`
agora juntam via canal (mesmas mensagens de erro); retorno de instancia
volta decodificado (antes degradava em mapa — correcao).

## 11. Criterios de aceitacao

Timeout/cancel/combinadores funcionam com testes estaveis; sem regressoes.
