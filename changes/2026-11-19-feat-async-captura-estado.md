# Added — Tarefas async com captura e codigo compartilhado (item 1)

- ID: 2026-11-19-feat-async-captura-estado
- Data: 2026-11-19
- Versão: 0.4.0
- Tipo: Added
- Compatibilidade: Compatível (aditivo + 1 correcao de fronteira)

## Resumo

Tarefas `async`/`await`/`async_spawn` passam a enxergar o codigo do programa
(funcoes de topo, classes, `import native`) e transportar captura de valores
planos. Globais mutaveis continuam sem atravessar (arquitetura permanente).

## Motivação

Item 1 das pendencias: tarefas exigiam funcoes autocontidas sem captura e
sem chamar outras funcoes (o worker recompilava so a funcao, sem topo), o
que inviabilizava composicao (`main` chamando `buscarA`) e closures com
locais.

## O que mudou

- Worker executa as definicoes de topo (`fn`, `class`, `interface`, `trait`,
  `namespace`, `import native`) antes da tarefa (filtro `defs_program`).
- Captura serializada: `env` cruza como objeto JSON; so valores planos
  atravessam (resto fica indisponivel; uso falha com `variavel nao definida`).
- `async_spawn`/`async fn` com `this` (metodo) seguem rejeitados; geradores
  seguem rejeitados.
- Nomes de closure unicos e deterministicos (`{closure:N}`) para resolucao
  exata no worker (antes, todas chamavam-se `{closure}`).
- Busca da definicao no worker: arvore de chunks por nome (recursiva, teto
  128) e depois globais; `async_spawn` ganha o codigo compartilhado de graca.
- Worker recebe `program` (permite `async` aninhado).
- Correcao de fronteira (bug): instancia como argumento agora e erro claro
  (`` `async` nao transporta instancia ``) em vez de degradar em mapa.

## Detalhes técnicos

`crates/rhp-vm/src/lib.rs` (`defs_program`, `find_def_by_name`,
`run_task_with_args`, `spawn_async_call`, `check_shareable`);
`crates/rhp-bytecode/src/lib.rs` (`closure_seq`, `fresh_closure_name`,
propagacao ida-e-volta nos compiladores aninhados); teste ajustado
(`{closure}` -> `{closure:0}`).

## Compatibilidade e migração

Compativel. Programas que dependiam da degradacao silenciosa
instancia->mapa agora recebem erro claro (correcao documentada). Nenhuma
migracao para o resto.

## Segurança e performance

Fronteira validada antes de serializar (sem corromper); sem `unsafe`;
recursao do worker limitada (teto 128); 1 thread/tarefa (inalterado); custo
extra de 1 compilacao + execucao de definicoes por tarefa.

## Testes e validação

- `cargo test -p rhp-vm --lib` (91 verdes), bytecode (40), CLI `nextgen_*`
  (14 verdes, +4 novos: captura, funcoes+classes, rejeicao de instancia,
  aninhado).
- Manual: composicao `main`->`buscarA`, closures irmaos, classe na tarefa,
  nativa na tarefa, erro de instancia.

## Limitações e riscos conhecidos

Globais mutaveis nao atravessam; metodos async rejeitados; sem
cancel/timeout/pool (ver RFC 0030, proposta).

## Referências

RFC 0023 (atualizada); `LANGUAGE_SPEC.md` §10; testes `nextgen_async_*`.
