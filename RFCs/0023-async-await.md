# RFC 0023 — Async/await sobre tarefas isoladas

- ID: RFC 0023
- Status: aceita e implementada (+ atualizacao item 1 em 2026-11-19)
- Data: 2026-11-19
- Versao: unreleased (0.4.0 planejada)

## 1. Problema

`async_spawn`/`async_join` (RFC 0019) exigem funcoes sem captura/args por
nome em string — ergonomico para paralelismo simples, insuficiente para
`await buscarA()` com argumentos e composicao legivel.

## 2. Objetivo e nao-objetivos

- Objetivo: `async fn` (chamada gera tarefa, devolve handle) + `await`
  (junta; identidade para nao-tarefas), com args posicionais e I/O
  bloqueante isolado por thread.
- Nao-objetivos: executor com poll/wakeup, `select`/`cancel`/`timeout`
  (ver RFC 0030, proposta), milhares de tarefas (uma thread por tarefa;
  documentado), write-back em metodos async (snapshot; ver abaixo),
  globais mutaveis no worker (arquitetura: codigo compartilhado, estado
  nao).

## 3. Proposta

```rhp
async fn buscarA() { sleep(0.2); return "A"; }
async fn buscarB() { return 42; }
let a = buscarA();
let b = buscarB();
echo await a; // "A" (paralelo)
echo await b; // 42
```

- `FuncDef.is_async`: `call`/`call_mixed` com func async geram tarefa
  (valida, serializa args em JSON, thread com programa recompilado) em vez
  de empilhar frame; devolvem handle `Int` (compativel com `async_join`).
- `await v`: novo op `Await`; handle valido = join (erros propagam);
  outro valor = identidade.
- `async fn` com gerador = erro. Nomeados/spread em chamada mista async =
  erro.
- Captura: transporta valores planos e instancias (com classe, via envelope
  `{"__rhp_instancia__": {"classe", "campos"}}`; mapas com a chave reservada
  sao rejeitados no spawn). Funcao, classe e gerador nao atravessam (erro
  claro). O resto fica indisponivel (se o corpo usar, falha com `variavel
  nao definida`).
- Metodos `async` (item metodos-async): `async fn` em classe/trait; chamada
  (`obj.m()`, destacada ou `super.m()`) gera tarefa sobre um SNAPSHOT do
  `this` (reconstruido no worker com a classe local) — SEM write-back:
  mutacoes nao voltam ao chamador; devolva valores com `return`.
- Codigo compartilhado: o worker executa as definicoes de topo (funcoes,
  classes, `import native`) e resolve a definicao exata (closures tem nomes
  unicos `{closure:N}`); tarefas chamam outras funcoes de topo, usam
  classes e extensoes, e aninham `async` (worker com programa-fonte).
- Estado NAO compartilhado (arquitetura permanente): globais mutaveis (`let`
  no topo) nao atravessam — passe via args/captura. `static` por worker.

## 4. Alternativas consideradas

- VM com suspensao de frames (stackless): rejeitada (reescrita da VM,
  risco a estabilidade; modelo de threads isoladas ja validado).
- Runtime com epoll/thread-pool: adiado (std-only; thread por tarefa basta
  para medio porte com `join` pronto).

## 5. Compatibilidade

Compativel: `async`/`await` eram identificadores validos (variaveis com
esses nomes quebram — intencional). `async_spawn`/`async_join` intactos;
handles interoperam com `await`.

## 6. Seguranca

Args/retornos cruzam como JSON (fronteira `Send`; tipos nao-serializaveis
rejeitados). Sem compartilhamento de memoria entre tarefas. Threads
limitadas pelo uso (documentar `join` pronto; sem spawn ilimitado).

## 7. Performance

Uma thread por tarefa (como RFC 0019); I/O concorrente sem bloquear a VM
principal. Sem backpressure automatica (documentado).

## 8. Plano de implementacao

Lexer/parser (`async fn`, `await`, closures async) -> `FuncDef.is_async` ->
VM (`spawn_async_call`, `Await`, `run_task_with_args`) -> testes de
concorrencia.

## 9. Plano de testes

Concorrencia (sleep paralelo), propagacao de erro, `await` identidade,
rejeicao de metodo/gerador/nomeados/instancia, captura de locais,
chamada a outra funcao de topo, classe na tarefa, async aninhado.

## 10. Migracao e rollout

Aditivo; `async_spawn` segue disponivel (sem captura, como antes; agora com
codigo compartilhado no worker). Correcao de fronteira: instancia como arg
vira erro claro em vez de mapa silencioso.

## 11. Criterios de aceitacao

`await` suspende ate concluir sem bloquear outras tarefas; erros propagam;
sem crescimento ilimitado alem do numero de tarefas criadas.
