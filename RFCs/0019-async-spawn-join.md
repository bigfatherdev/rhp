# RFC 0019 — Tarefas assíncronas (`async_spawn`/`async_join`; Etapa 5)

- ID: RFC-0019
- Data: 2026-11-16
- Estado: aceita (autorização prévia do programa Etapas 3–5) e implementada
- Contexto: Etapa 5, item "async". Primeira concorrência real do RHP
  (threads do SO, não runtime cooperativo).

## 1. Problema

Operações lentas (`sleep`, `http_fetch`, computação) bloqueiam tudo;
não há como rodá-las em paralelo (ex.: 2 fetches independentes).

## 2. Objetivo e não-objetivos

Objetivo: `async_spawn("f") -> Int` (handle) + `async_join(h)` (bloqueia
e devolve o retorno); tarefas isoladas (sem estado compartilhado);
erros da tarefa viram erro do `join`.

Não-objetivos: `async`/`await` sintático; args na criação (zero-arg
v1); closures/geradores/métodos como tarefa (só `fn` de nível superior
sem captura); cancelamento/timeout de join;通道 Go-style; `select`.

## 3. Proposta

- `async_spawn(nome)`: valida `Str`, funcao de nivel superior existente,
  nao-geradora, env vazio (sem captura); cria thread que recompila o
  `Arc<Program>` (padrao `http_serve`: owned+`Send`), executa e
  serializa o retorno em JSON (`json_stringify`; nao-serializavel =
  erro da tarefa). Devolve handle `Int` (como `db_open`).
- `async_join(h)`: consome o handle (one-shot; segundo `join` = erro
  `handle invalido`), bloqueia, desserializa (`json_parse`) e devolve;
  erro da tarefa vira `VmError("tarefa falhou: ...")`.
- Isolamento total: sem globais compartilhados; comunicacao so pelo
  valor de retorno (fronteira JSON: null/bool/int/float/string/array/
  mapa; resto = erro). `Vm::reset` descarta handles pendentes.
- Interceptados no `step_inner` (como HOFs): precisam da VM (programa
  + tabela de tarefas). Tabela `Vm.tasks` (como `DbHandles`).

## 4. Alternativas

- Cooperativo single-thread com `yield` points: sem I/O async para
  ceder, nao ha ganho — rejeitada (threads dao paralelismo real e
  mensuravel).
- Compartilhar `Rc<Chunk>`/valores entre threads: `Rc` nao e `Send`
  (recompilar por thread e o padrao ja auditado do `http_serve`) —
  rejeitada (insegura sem prova).
- Passar args via JSON: dobra a superficie v1 — adiada (follow-up
  documentado; zero-arg cobre o caso de uso).
- `await` sintatico sem runtime: acucar sem semantica — rejeitada.

## 5. Compatibilidade

Aditivo: 2 builtins novos (ids 66/67, sem renumerar). Sem mudanca em
programas existentes.

## 6. Segurança

Threads so executam codigo do proprio programa (recompilado do AST
confiavel, validado). Sem dados compartilhados (sem data race por
construcao). `join` bloqueia a thread chamadora (sem deadlock novo:
handle one-shot, sem ciclos possiveis — tarefa nao conhece handles).
Limites da tarefa: VM zerada (documentado; sem teto herdado).

## 7. Performance

Custo = 1 thread + 1 compilacao + JSON ida/volta. Ganho = paralelismo
real (2x `sleep(200ms)` em ~200ms, nao ~400ms; teste com margem folgada
contra flake de CI compartilhada).

## 8. Plano de implementação

1. Builtins `AsyncSpawn`/`AsyncJoin` (ids, nomes, aridade 1, teste
   anti-deriva).
2. `Vm.tasks` + interceptacao no `step_inner` (padrao HOF).
3. Worker: recompila, acha `fn`, roda, serializa.
4. Testes (resultado, paralelismo com margem, erro, args ausentes,
   gerador/closure/metodo rejeitados, handle invalido/duplo join).
5. Spec + exemplo + `changes/`.

## 9. Plano de testes

- `spawn`+`join` devolve valor (incl. tipos compostos).
- 2x `sleep(200ms)` paralelo termina < 350ms (serial seria 400ms+).
- Erro na tarefa vira erro no `join` (mensagem com prefixo).
- `spawn` de inexistente/nao-funcao/gerador = erro claro.
- `join` de handle invalido/duplo = erro.

## 10. Migração e rollout

Nenhuma migração (aditivo). Rollout normal.

## 11. Critérios de aceitação

- [x] RFC aceita antes do código (autorização prévia 2026-11-16).
- [ ] Paralelismo mensurável + 5 erros pinados.
- [ ] Suite verde; spec + exemplo + `changes/`.
