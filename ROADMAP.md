# ROADMAP.md — Planejamento e estado (Next Generation + limitacoes cuidadas)

## RHP 0.7.2 — Auditoria de engenharia (2026-11-21, P0/P1/P2 zerados)

- [x] 5 P0: for-in 2^N → linear, yield-em-try, Display abort, with_capacity
  redis/mysql (fakes + regressões, fuzz 1000 limpo).
- [x] P1/P2: `next` +512, HOF nativa/async, `web_finalizar`, filtro
  `http_serve` no topo, `?` MySQL, `+`/absoluta/obs-fold/NUL/`exit`-200,
  `catch` sem 256, `MakeFunc` no heap, CA-bundle vazio, userinfo.
- [x] Perf: `json_parse` O(n) (~43x); docs honestas (deps/`unsafe`/limites);
  REPL multilinha, `test` recursivo; suíte 395/395.
- [ ] 0.8.0 planejado: TLS MySQL (RFC 0034), builder de strings (RFC),
  `fs_chown`/ACL, prazo por handler (RFC), unificar `db_exec(SELECT)`,
  `i64::MIN`, fuzz-gate de CI, paginação de resultados.

## RHP 0.7.1 — Complementos (2026-11-20)

- [x] `fs_chmod`/`fs_stat` (85 builtins) + padrão `join` documentado.
- [x] RFC 0034 (MySQL TLS, proposta p/ 0.8.0).
- [ ] 0.8.0 planejado: `and`/`or` e `in` seguem rejeitados (RFCs 0031/0032);
  TLS (RFC 0034), builder de strings, `fs_chown`/ACL, unificar `db_exec`.

## RHP 0.7.0 — Lógica, pertinência e caching_sha2 (2026-11-20, RFCs 0031–0033)

- [x] `&&`/`||` com curto-circuito (E3003 estrito; `AssertBool`; precedência documentada).
- [x] `contains(array, valor)` (igualdade por valor).
- [x] MySQL `caching_sha2_password` (fast + full RSA; live 8.0 default).
- [x] `tools/fuzz-smoke.sh` versionado; suíte + bench + docs.
- [ ] Futuro (fora do escopo 0.7.0): `and`/`or`, `in`, TLS no MySQL, builder de strings, `fs_chmod`/`stat`, unificar `db_exec(SELECT)`.

## RHP 0.6.2 — Auditoria independente (2026-11-20, P0/P1 zerados)

- [x] HOF `pending` obsoleto → amarra + descarte no unwind + regressão.
- [x] Dreno de erro pg/mysql → sync preservado + regressões live/mock.
- [x] `fs_rename`/`fs_remove`/`fs_mkdir` (83 builtins) + USAGE `build`.
- [x] Fuzz 3500 limpo; suíte 370/370; bench sem regressão.
- [ ] P2 planejado 0.7.0 (exige RFC): `&&`/`||`, pertinência, `caching_sha2`.

## RHP 0.6.1 — Auditoria full-system (2026-11-20, P0/P1 zerados)

- [x] SIGABRT do parser com cadeias planas → teto 64 + regressão.
- [x] MySQL inoperante (seq por comando) → funcional + mock asserta seq.
- [x] 23 mensagens de tipo 2-arg citam ambos os tipos.
- [x] Fuzz 2500 casos limpo; bench sem regressão; suíte 367/367.
- [ ] P2 planejado 0.7.0: `&&`/`||`, pertinência em array, `caching_sha2`.

## RHP 0.6.0 — Engenharia de correcao e robustez (2026-11-20, implementado e validado)

- [x] `return`/`yield` em `try` capturam erro do operando (ordem do compilador corrigida; 8 casos).
- [x] `http_serve` com `host` configuravel (default local; `0.0.0.0` opt-in) + erro claro de bind.
- [x] HTTP: 400s corretos, teto de headers, CL estrito, corpo incompleto/chunked rejeitados, prazo total, accept resiliente, percent-decode UTF-8.
- [x] Handler como valor + `max_passos`/`max_heap_bytes` + anti-reentrada (sem hangs).
- [x] Corpo `chunked` (RFC 9112), `"localhost"` = 127.0.0.1, reasons 408/413/422/502/504.
- [x] Provas Docker reais (container↔container, nginx reverso, port forwarding, controle negativo).
- [x] Suite 354 testes + fmt/clippy limpos; baseline de bench 0.6.0; docs e changelog sincronizados.

## Limitacoes cuidadas (2026-11-19, implementado e validado)

- [x] Bounds de generics (RFC 0029 implementada): `type_args` preservados,
  checagem em compilacao, apagamento mantido.
- [x] Metodos `async` + transporte de `Instance` com classe (snapshot, sem
  write-back; chave reservada `__rhp_instancia__`).
- [x] RFC 0030 (nucleo): `await_timeout`, `async_cancel`/`async_cancelled`,
  `await_all`, `await_race` (ids 75–79); pool segue adiado (documentado).
- [x] Router do RHP Web com trie ordenada + teste diferencial + benchmark.
- [x] Globais mutaveis: arquitetura permanente documentada (`docs/ASYNC.md`).

## Next Generation — 8 fases (2026-11-19, implementado e validado)

- [x] Fase 0: auditoria completa (matriz estado/dependencias/risco/estrategia).
- [x] Fase 1: heranca (`extends`, `super`), interfaces (`implements`,
  verificacao de aridade), traits (`use`, precedencia propria > trait > pai)
  — RFC 0021.
- [x] Fase 2: namespaces (`namespace`, `A\B`) + autoload (`import A\B`,
  `as`, PSR-4 `A/B.rhp` na arvore) — RFC 0022.
- [x] Fase 3: `async fn` + `await` sobre tarefas isoladas (args, erros
  propagados; autocontidas) — RFC 0023.
- [x] Fase 4: excecoes tipadas (`catch Tipo [var]`, hierarquia, `finally`)
  — RFC 0024.
- [x] Fase 5: generics por apagamento (`<T>`, `<int>`, `T` sem checagem) —
  RFC 0025.
- [x] Fase 6: extensoes nativas estaticas (`import native`, ABI 1, `demo`) —
  RFC 0026.
- [x] Fase 7: RHP Web (`framework/web.rhp`: rotas, params, middleware, JSON,
  404/405, seguranca, templates; exemplo funcional) — RFC 0027.
- [x] Fase 8: docs (specs, RFCs, guias), compatibilidade (suite verde),
  auditoria final.

## Itens 1–4 (2026-11-19, implementado e validado)

- [x] Item 1: async com captura (valores planos) + codigo compartilhado no
  worker (funcoes, classes, `import native`; nomes de closure unicos
  `{closure:N}`); rejeicao clara de instancia como argumento (antes degradava
  em mapa). Testes `nextgen_async_captura_ambiente`,
  `nextgen_async_enxerga_funcoes_e_classes`, `nextgen_async_rejeita_instancia`,
  `nextgen_async_aninhado`.
- [x] Item 2: RHP Web via `rhp pkg` (`pkg add rhp_web <repo>/framework` ->
  `pacotes/rhp_web/`; teste `nextgen_framework_via_pkg`; guia `docs/WEB.md`).
- [x] Item 3: RFCs proposta 0028 (`[autoload]` + `use function`), 0029
  (bounds `<T: X>`), 0030 (cancel/timeout/`all`/`race`) — nao implementadas.
- [x] Item 4: suite final verde (340 testes), `fmt --check` limpo, `clippy`
  sem avisos; release `0.4.0`.

## Auditoria e fortalecimento (2026-11-14, ramos `feat/engineering-audit` e `feat/next-generation`)

- [x] CHECKPOINT 1 (auditoria integral) e CHECKPOINT 1 (maturidade):
  `reports/AUDIT_MASTER.md`, `reports/NEXT_GENERATION_AUDIT.md`.
- [x] Correcoes P0/P1, P2 de nucleo, `VmLimits`, jaula/teto de imports.
- [x] Robustez: corpus malformado, property test, verificacao de exemplos.
- [x] Benchmarks + baseline (sem otimizacoes sem prova).
- [x] Docs, CLI (`check`), ADRs 0005–0010.
- [x] CP2 Diagnosticos: `rhp-diagnostic` (E0xxx–E3xxx + render).
- [x] CP3 Memoria: decisao por valor + dedup/interning + `reports/MEMORY_MODEL.md`.
- [x] CP4 Bytecode/VM: fluxo de pilha + `VmLimits` estendido + `builtins.rs`.
- [x] CP5 Modulos: ADR 0009 (semantica, sem codigo).
- [x] CP6 DevTools: `rhp check [--json]`, gramatica VSCode, desenho de debugger.
- [x] CP7 Seguranca: limites documentados/testados, sem sandbox ficticia.
- [x] CP8 Performance: harness + baseline; nenhuma otimizacao sem prova.
- [x] CP9 Documentacao: specs/ADRs/README/CHANGELOG consistentes.
- [ ] Pendente (requer aprovacao): RFC de tipos, re-export/gerenciador de
  pacotes (Fase D cobre pacotes v0), Redis com servidor real, async
  (runtime), classes/generics, LSP, `rhp fmt` preservando comentarios e
  linhas em branco, otimizacoes com protocolo antes/depois. (Concluído
  desde então: exports base, PG com servidor real.)

- [x] CHECKPOINT 1: `reports/AUDIT_MASTER.md` (1 P0, 5 P1, 13 P2, 10 P3, 7 P4).
- [x] CHECKPOINT 3: P0/P1 corrigidos (teto de profundidade 128, fmt roundtrip,
      `\u{...}`, teto HTTP 1 MiB, DNS documentado) + P2 de núcleo (params
      duplicados, `int()` estrito, `+` unário, jaula/topo de imports, teto de
      fonte, `VmLimits`/`--max-steps`, pins de spec).
- [x] Robustez: corpus malformado, property test determinístico, verificação
  de exemplos, `check-changes.sh` com renames.
- [x] Benchmarks std-only + baseline (`docs/BENCHMARKS.md`); sem otimizações
  (sem necessidade provada; `push` O(n²) é semântica documentada).
- [x] CHECKPOINT 2: README reestruturado + `docs/` + `CONTRIBUTING.md` +
  `SECURITY.md`.
- [ ] Pendente (requer aprovação/planejamento): P2 restantes de baixo risco,
  RFC de tipos, re-export/gerenciador de pacotes (Fase D cobre pacotes v0),
  Redis com servidor real, async (runtime), classes/generics, LSP,
  otimizações com protocolo antes/depois. (Concluído desde então: exports
  base, PG com servidor real, `rhp fmt` idempotente atual.)

## Fase 7 — Paridade PHP, lote imediato (em andamento, 2026-11-15)

Auditoria: `reports/PHP_PARITY_AUDIT.md` (referencia PHP 8.5.12).

- [x] Sintaxe lote 2: bitwise, ternario, `??`, `break`/`continue`, `const`
  (`changes/2026-11-15-feat-sintaxe-lote2.md`).
- [x] Stdlib: 20 builtins — strings, matematica, mapas — ids 10–29
  (`changes/2026-11-15-feat-stdlib-lote1.md`; nota: o registro lote2
  diz "19, ids 10-28" por engano — vale o correto, sem reescrever o
  registro anterior).
- [x] I/O e ambiente: `fs_read/fs_write/fs_append/fs_exists`, `argv()`,
  `env()`, args repassados (`changes/2026-11-15-feat-io-env.md`).
- [x] `rhp test` + `rhp doctor`
  (`changes/2026-11-15-feat-cli-test-doctor.md`).
- [x] Laco `for-in`, `sort()`, `exit()` + correcao do escopo de
  `break`/`continue`
  (`changes/2026-11-15-feat-laco-for-sort-exit.md`,
  `changes/2026-11-15-fix-laco-escopo-break.md`). 168 testes verdes.
- [x] JSON std-only: `json_parse`/`json_stringify` sem dependencias, com
  teto de 128 niveis e erros com posicao
  (  `changes/2026-11-15-feat-json.md`).
- [ ] Pendente (ordem tecnica; o resto do lote imediato saiu nas Fases
  A–C): namespaces/exports (restante: re-export), pacotes, tipos, OO,
  async/DBs (restante: Redis), LSP/debugger completos (todos exigem
  RFC/ADR e gates, cf. auditoria §21).
- [x] Lote nucleo-2 (RFC 0008 aceita e implementada: `match` estrito,
  Elvis `?:`, arrow `=>`, `for(;;)`, `break`/`continue N`).
- [x] `switch` com fallthrough (RFC 0004 aceita e implementada:
  `case`/`default`, `break`/`continue`, escopo por ramo).
- [x] HOFs com callback (RFC 0005 aceita e implementada: `map`, `filter`,
  `usort` estavel via protocolo de chamada pendente).
- [x] Parametros default e variadicos (RFC 0006 aceita e implementada:
  defaults constantes dobrados, `...rest`).
- [x] Destructuring raso (RFC 0007 aceita e implementada: `let [a, b]` /
  `let {x}` / `let {k: v}` + `const`).
- [x] Chamadas mistas (RFC 0009 aceita e implementada: nomeados + spread).
- [x] `static` em funcao (RFC 0010 aceita e implementada: estado por definicao).
- [x] Re-export seletivo (RFC 0011 aceita e implementada: transitivo, sem `export *`).
- [x] Builtins `rand`/`rand_range`/`fs_list` (RFC 0012 aceita e implementada).
- [x] Hashing de senhas (RFC 0013 + ADR 0013 aceitas e implementadas: `hash_password` Argon2id + `verify_password` `$argon2id$`/bcrypt, teto anti-DoS).
- [x] Regex, hashing e HMAC (RFC 0014 + ADR 0014 aceitas e implementadas: `regex_*` sem lookaround/backrefs, `sha256`, `hmac_sha256`/`hmac_verify`).
- [x] Closures de primeira classe (RFC 0003 aceita e implementada:
  `fn (params) { }`, captura por valor, `MakeFunc`, `CallValue`).

## Etapas 3–5 — Suporte, DevX e grandes (pós PHP-100 funcional)

- [x] Etapa 3 (suporte, concluída): suite de conformidade
  (`tests/conformance/`), property tests std-only ampliados, teto de heap
  (`max_heap_bytes`, RFC 0015), fuzz determinístico std-only em CI (+1 bug
  real corrigido), otimização `Rc<Chunk>` nos frames (fib -4% com prova).
- [x] Etapa 4 (DevX, concluída): `rhp fmt` preserva comentários e linhas
  em branco, stack traces, REPL com estado, `rhp run --trace`,
  `rhp lsp` (JSON-RPC stdio + diagnostics).
- [x] Etapa 5 (grandes, concluída sob autorização prévia, cada item com
  RFC): tipos com checagem runtime (RFC 0016), OO mínima (RFC 0017),
  geradores lazy + `for-in` (RFC 0018), async spawn/join (RFC 0019),
  TCP/Redis/MySQL (RFC 0020, mocks in-process).

## Fases A–D — RHP para sites/sistemas (plano aprovado, em andamento)

- [x] A1: RFC 0001 — exceções (`RFCs/0001-excecoes.md`, aceita e implementada).
- [x] A2: exceções implementadas (lexer/parser/bytecode/VM/fmt/CLI/specs).
- [x] A3: namespaces/exports (ADR 0009 implementada: `export` + seletivo).
- [x] A4: datas/horas std-only (`now`, `sleep`, `date_str`).
- [x] A5: rede v2 (`http_fetch`: redirects, headers, timeout).
  **Fase A concluída** (fundação do runtime de sistemas).
- [x] B1: ADR 0011 — persistência (SQLite FFI, PG protocolo).
- [x] B2: SQLite (`db_open/exec/query/close` via `libsqlite3`).
- [x] B3: PostgreSQL com servidor real (`db_*` via protocolo próprio).
  **Fase B concluída** (persistência).
- [x] C1: RFC 0002 — concorrência + servidor.
- [x] C2: `http_serve` (thread por conexão, handler stateless, 500 sem vazar).
- [x] C3: cookies, `template()` com escape, padrão sessão-via-banco.
- [x] D1: ADR 0012 — TLS (rustls puro + raízes embutidas).
- [x] D2: `https://` no `http_fetch` (upgrade seguido, downgrade recusado,
  `RHP_CA_BUNDLE`).
- [x] D3: pacotes v0 (`rhp init`, `rhp pkg add/install [--check]`,
  `pacotes/` + `rhp.lock`).
- [x] D4: `rhp build` (bundle autocontido) + limite de taxa no servidor.
- [ ] D5: release `0.1.0` (changelog consolidado + tag).

## Fase 1 — Fundacao (concluida em 2026-11-14)

- [x] Governanca, workspace, `hello.rhp`, lexer/parser/bytecode/VM/CLI.
- [x] Testes + `fmt`/`clippy`/`test` verdes; registro em `changes/`.

## Marco 2 — Nucleo imperativo (concluido em 2026-11-14)

- [x] Expressoes, variaveis, `if`/`else`, `while`, funcoes, `echo` geral.
- [x] ADR 0001; specs junto do codigo; compatibilidade Fase 1 preservada.

## Fase 3 — Stdlib nucleo (concluida em 2026-11-14)

- [x] Arrays/mapas (semantica por valor), indexacao e `a[i] = v` (1 nivel).
- [x] Builtins: `len push pop keys str int float type assert` (ADR 0002).

## Fase 4 — Modulos (concluida em 2026-11-14)

- [x] `import "x.rhp"` com DFS, dedup, ciclo/ausente como erro (ADR 0003).

## Fase 5 — Ferramentas (concluida em 2026-11-14)

- [x] `rhp-fmt` canonico idempotente + `rhp fmt [--check]`.
- [x] `tools/check-changes.sh` de CI (verificado em 4 cenarios reais).

## Fase 6 — Rede minima + fronteira (concluida em 2026-11-14)

- [x] `http_get` std-only verificado contra servidor local (ADR 0004).
- [x] Fronteira honesta: PostgreSQL/Redis, async/concorrencia,
      classes/generics, exports/privacidade, metodos — planejados, NAO
      implementados (exigem servidores de teste, runtime ou RFC de tipos).

Nada do item "planejado" deve ser assumido como existente.
