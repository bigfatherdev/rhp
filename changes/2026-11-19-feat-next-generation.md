# Next Generation — 8 fases (heranca, namespaces, async, tipadas, generics, extensoes, web)

- ID: 2026-11-19-feat-next-generation
- Data: 2026-11-19
- Versão: 0.4.0
- Tipo: Added
- Compatibilidade: Compatível (aditivo; 1 mudanca comportamental documentada no worker `http_serve`)

## Resumo

Evolucao completa do RHP: heranca/interfaces/traits/`super`; namespaces +
autoload; `async fn`/`await`; excecoes tipadas; generics por apagamento;
extensoes nativas estaticas (`demo`); framework web oficial (`framework/web.rhp`
+ exemplo); docs (specs, 7 RFCs, `docs/WEB.md`), changelog, roadmap e 10 testes
`nextgen_*` — suite integral verde sem regressoes.

## Motivação

RHP 0.2.0/0.3.0 tinha classes minimas, `async_spawn` sem args, `catch` unico,
sem namespaces, sem extensoes e sem framework — insuficiente para APIs, apps
web, sistemas concorrentes e projetos medios/grandes (objetivo do programa).

## O que mudou

- Fase 1 (RFC 0021): `extends` (pai definido antes), `implements` (aridade),
  `trait`/`use` (precedencia propria > trait > pai; conflito = erro), `super`.
  Ops `BuildClassEx`/`CallSuper`; `ClassDef.parent`.
- Fase 2 (RFC 0022): `namespace`, `\`, `import A\B [as C]`, autoload PSR-4
  (`A/B.rhp`) confinado a arvore; qualificadas globais (sem rename).
- Fase 3 (RFC 0023): `async fn` (gera tarefa, args posicionais JSON) + `await`
  (join/identidade); interoperavel com `async_join`. Autocontidas.
- Fase 4 (RFC 0024): `catch Tipo [var]` multiplo, `IsInstanceOf` por `parent`,
  `finally` sempre.
- Fase 5 (RFC 0025): `<T>`/`…<int>` apagados; `<` binario preservado.
- Fase 6 (RFC 0026): `import native`, `LoadNative`, `FuncDef.native_id`,
  registro estatico ABI 1, `demo` (`demo_eco/soma/len`).
- Fase 7 (RFC 0027): `framework/web.rhp` (rotas, `:params`, middlewares, JSON,
  404/405, seguranca, templates) + `framework/exemplo_api.rhp` + `docs/WEB.md`.
  Worker `http_serve` executa o topo por request (menos a chamada serve).
- Fase 8: specs (`LANGUAGE`/`BYTECODE`/`VM`/`ARCHITECTURE`), 7 RFCs, roadmap,
  changelog, gramatica VSCode, `docs/WEB.md`, 10 testes de integracao.

## Detalhes técnicos

Lexer (`extends`, `implements`, `interface`, `trait`, `use`, `super`, `async`,
`await`, `namespace`, `native`, `\`); parser (AST + validacoes + desambiguacao
`<`); bytecode (tabelas classes/interfaces/traits, `qualify`/`resolve_name`,
achatar heranca, `BuildClassEx`/`IsInstanceOf`/`Await`/`CallSuper`/`LoadNative`);
VM (`parent`, `pending_super`, `is_async` spawn, `Await`, `IsInstanceOf`,
nativas, `loaded_extensions`, worker com topo); CLI (linker: rename global p/
`\`, autoload, alias, validacao `demo`); fmt/lsp/diagnostico; framework em RHP.

## Arquivos ou áreas afetadas

- `crates/rhp-lexer/src/lib.rs`, `crates/rhp-parser/src/lib.rs`,
  `crates/rhp-bytecode/src/lib.rs`, `crates/rhp-vm/src/lib.rs`,
  `crates/rhp-vm/src/extensions.rs` (novo), `crates/rhp-vm/src/http_server.rs`
  (worker com topo), `crates/rhp-fmt/src/lib.rs`, `crates/rhp-diagnostic`,
  `crates/rhp-cli/src/main.rs`, `crates/rhp-cli/tests/integration.rs` (+10),
  `editors/vscode/syntaxes/rhp.tmLanguage.json`, `framework/web.rhp` (novo),
  `framework/exemplo_api.rhp` (novo), `RFCs/0021-0027.md` (novos),
  `LANGUAGE_SPEC.md`, `BYTECODE_SPEC.md`, `VM_SPEC.md`, `ARCHITECTURE.md`,
  `ROADMAP.md`, `CHANGELOG.md`, `docs/WEB.md` (novo).

## Compatibilidade e migração

Compativel e aditivo, exceto: (1) `super/async/await/...` viraram palavras
reservadas (variaveis com esses nomes quebram); (2) worker `http_serve`
executa o topo por request (antes: so `fn`s) — programas so com `fn`s +
`http_serve` comportam-se igual; topo com efeitos nao-idempotentes agora
repete por request (documentado; mover efeitos para handlers). Sem migracao
para codigo 0.3.0 tipico.

## Segurança e performance

- Seguranca: cadeia `parent` limitada (256); autoload confinado; nativas sem
  `unsafe`/`.so`, fronteira validada, ABI checada; web sem stack traces, com
  escape, teto 1 MiB e headers; threads 1-por-tarefa (fazer `join`/`await`).
- Performance: metodos achatados O(1); rotas O(R); generics custo zero;
  nativas diretas; sem benchmarks novos (sem otimizacao sem prova, cf.
  governanca). Suite: 325+ testes verdes.

## Testes e validação

- `cargo test --workspace`: tudo verde (40+43+2+47+5+28+17+50+91+2; inclui 10
  `nextgen_*` novos: heranca, pai/ciclo, super, interfaces/traits, conflito,
  tipadas, generics, async, nativas, autoload).
- Manual: herança/`super`/interfaces/traits, `catch` tipado, `Caixa<int>`,
  `async/await` concorrente, `demo_*`, autoload com alias, `framework/web.rhp`
  via HTTP real (`/` e `/users/42`).
- `cargo fmt --check` / `clippy` (ver auditoria final).

## Limitações e riscos conhecidos

- `async fn` autocontida (sem chamar funcoes de usuario/globais; worker sem
  topo do chamador); sem cancel/timeout/pool; 1 thread por tarefa.
- Metodos `async` e `new` async rejeitados; nomeados/spread em async rejeitados.
- Namespace: qualificacao explicita recomendada; `namespace` = ultimo vence.
- Generics apagados (sem checagem de tipos).
- Web: roteamento linear; topo idempotente requerido; sem ORM/sessoes.
- `http_serve` com topo com efeitos: mudanca comportamental documentada.

## Referências

RFCs 0021–0027; `LANGUAGE_SPEC.md` §§8–14; `framework/web.rhp`;
`docs/WEB.md`; commits da entrega.
