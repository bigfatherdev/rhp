# CHANGELOG.md — Resumo consolidado por release

Formato inspirado em Keep a Changelog. Detalhes auditaveis ficam em `changes/`.

## [unreleased]

## [0.2.0] — 2026-11-18 (Etapas 3-5 + DevOps: conformidade, heap, fuzz, fmt, DevX, tipos, OO, geradores, async, conectores, Docker)

### Added

- Distribuição oficial via Docker Hub (`bigfatherdev/rhp`, Etapa DevOps):
  `Dockerfile` multi-stage multi-arch, workflow `docker-publish`
  (gates + build amd64/arm64 + push condicional + cache + tags semver
  reais), `docs/DOCKER.md`, ADR 0015. Registro:
  `changes/2026-11-17-tooling-docker-publish.md`.
- `fmt` preserva comentários e linhas em branco (Etapa 4, DevX):
  trivia `//` no lexer, `StmtNode` com decoracao, reemissao idempotente.
  Registro: `changes/2026-11-16-feat-fmt-comentarios.md`.
- Stack traces em erros de runtime (Etapa 4): `VmError.trace` + `  em f`
  na CLI (message intacta p/ `try`). Registro:
  `changes/2026-11-16-feat-stack-traces.md`.
- REPL com estado persistente (Etapa 4): `rhp repl` + `Vm::run_continue`.
  Registro: `changes/2026-11-16-feat-repl.md`.
- Trace de execucao + LSP minimo (Etapa 4): `rhp run --trace`,
  `rhp lsp` (JSON-RPC stdio, diagnostics). Registro:
  `changes/2026-11-16-feat-trace-lsp.md`.
- Conectores TCP/Redis/MySQL (RFC 0020, Etapa 5): `tcp_*`, `redis_*`
  (RESP2), `db_open("mysql:...")` reaproveitando `db_*`. Testes com
  servidores in-process. Registro:
  `changes/2026-11-16-feat-conectores-tcp-redis-mysql.md`.
- Tarefas assíncronas (RFC 0019, Etapa 5): `async_spawn("f")` (thread
  isolada, recompila o programa) + `async_join(h)` (bloqueia, devolve
  o retorno desserializado). Registro:
  `changes/2026-11-16-feat-async-spawn-join.md`.
- Geradores lazy (RFC 0018, Etapa 5): `yield`, `next`/`exhausted`,
  `for x in gen` (duas formas), `type()` = gerador, `==` por identidade.
  Registro: `changes/2026-11-16-feat-geradores.md`.
- Classes mínimas (RFC 0017, Etapa 5): `class`, `new` (construtor `novo`),
  `obj.metodo()` com `this` e write-back, `type()` classe/instância.
  Por valor, sem herança. Registro:
  `changes/2026-11-16-feat-classes-minimas.md`.
- Anotações de tipo com checagem em runtime (RFC 0016, Etapa 5):
  `let x: int`, `fn f(a: int): int`, 8 tipos, erro `tipos incompativeis`
  (E3001). Opt-in, sem coercao. Registro:
  `changes/2026-11-16-feat-tipos-anotacoes.md`.
- Teto de heap opt-in (RFC 0015, Etapa 3): `VmLimits.max_heap_bytes` +
  `rhp run --max-heap-bytes N`, estimador do modelo com enforcement O(1)
  amortizado, erro `heap excedeu o limite` (E3012). Padrão ilimitado.
  Registro: `changes/2026-11-16-feat-heap-limit.md`.
- Fuzz determinístico std-only em CI (Etapa 3): lexer (5000 entradas),
  parser (5000 sequências), VM/validador (8000 chunks hostis + 600
  mutações de programas válidos). Sem novas dependências. Registro:
  `changes/2026-11-16-fix-fuzz-vm-panic.md`.
- Suite de conformidade `tests/conformance/` (Etapa 3, Tooling): 18
  programas válidos com stdout pinada + 6 inválidos com stderr pinada,
  via `rhp run` (`crates/rhp-cli/tests/conformance.rs`); property tests
  std-only ampliados (`-`, `<`/`==`, `len()` de concatenação; 1800
  programas gerados). Registro:
  `changes/2026-11-16-test-conformidade-etapa3.md`.
- Closures de primeira classe (RFC 0003, PHP-100 P1): `fn (params) { }`
  como expressão com captura por valor, `MakeFunc`, chamadas de callee
  arbitrário (`CallValue`), `import` em closure rejeitado no loader.
  Registro: `changes/2026-11-16-feat-closures-primeira-classe.md`.
- `switch` com fallthrough (RFC 0004, Fase 2): `case`/`default` (único,
  posição livre), `break`/`continue`, escopo por ramo, comparação `==`.
  `switch`/`case`/`default` viram keywords.
  Registro: `changes/2026-11-16-feat-switch.md`.
- HOFs com callback (RFC 0005, Fase 2): `map`/`filter`/`usort` via
  protocolo de chamada pendente na VM (`usort` estável).
  Registro: `changes/2026-11-16-feat-hofs-callback.md`.
- Parâmetros default e variádicos (RFC 0006, Fase 2): `b = expr-const`
  dobrado em compilação, `...rest` coletando excedentes.
  Registro: `changes/2026-11-16-feat-args-default-variadicos.md`.
- Destructuring raso (RFC 0007, Fase 2): `let [a, b]` / `let {x}` /
  `let {k: v}` (+ `const`), erros herdados, sem `null` silencioso.
  Registro: `changes/2026-11-16-feat-destructuring.md`.
- Lote nucleo-2 (RFC 0008, Fase 3): `match` estrito, Elvis `?:`, arrow
  `=>`, `for(;;)`, `break`/`continue N` (`match` vira keyword).
  Registro: `changes/2026-11-16-feat-lote-nucleo-2.md`.
- Chamadas mistas (RFC 0009, Fase 4): nomeados + spread via `CallMixed`.
  Registro: `changes/2026-11-16-feat-chamadas-mistas.md`.
- `static` em funcao (RFC 0010, Fase 4): estado por definicao.
  Registro: `changes/2026-11-16-feat-static-funcao.md`.
- Re-export seletivo (RFC 0011, Fase 4): transitivo, sem `export *`.
  Registro: `changes/2026-11-16-feat-reexport.md`.
- Builtins `rand`/`rand_range`/`fs_list` (RFC 0012, Fase 4).
  Registro: `changes/2026-11-16-feat-rand-fslist.md`.
- Hashing de senhas (RFC 0013, Fase 5): `hash_password` (Argon2id) e
  `verify_password` (`$argon2id$` + bcrypt `$2*$`, migração de bancos PHP).
  Registro: `changes/2026-11-16-feat-password-hashing.md`.
- Regex, hashing e HMAC (RFC 0014, Fase 6): `regex_match`/`regex_replace`/
  `regex_split` (sem lookaround/backrefs), `sha256` + `hmac_sha256`/`hmac_verify`
  (RustCrypto puro, teto anti-DoS).
  Registro: `changes/2026-11-16-feat-regex-hmac.md`.

### Removed

- `RHP_GOVERNANCE.md` (duplicata byte-idêntica de `GOVERNANCE.md`).
  Registro: `changes/2026-11-16-docs-remover-governanca-duplicada.md`.

### Fixed

- `async_spawn` sem programa-fonte no REPL, no `rhp test` e nos handlers
  `http_serve` (so `rhp run` definia; worker precisa recompilar). Achado
  na verificação pós-merge (auditoria de todos os caminhos + servidor
  real). Registro: `changes/2026-11-17-fix-async-program-context.md`.
- Panic com `max_stack_depth` + `return` (Etapa 3, achado do fuzz):
  `Return` remove o frame e a checagem final de profundidade usava
  índice obsoleto; agora tolera frame removido (topo corrente medido
  no passo seguinte). Regressão pinada. Registro:
  `changes/2026-11-16-fix-fuzz-vm-panic.md`.

## [0.1.0] — 2026-11-15 (primeira release: linguagem + runtime web + persistencia)

### Added

- Primeira fatia vertical do RHP: `hello.rhp`, lexer, parser/AST, geracao de
  bytecode, VM em pilha e CLI `rhp run hello.rhp` (saida `Hello from RHP!`).
  Registro: `changes/2026-11-14-feat-fase-1-fatia-vertical.md`.
- Marco 2 (nucleo imperativo, compativel): expressoes (int/float/bool/null,
  aritmetica checada, comparacoes), `let`/atribuicao, blocos/escopos,
  `if`/`else`, `while`, `fn`/`return`/chamadas com recursao limitada (512),
  `echo` de qualquer valor, comentarios `//` e exemplo `examples/fib.rhp`.
  Registro: `changes/2026-11-14-feat-marco-2-nucleo-imperativo.md`.
- Fase 3 (stdlib nucleo): arrays/mapas por valor, indexacao e `a[i] = v`,
  builtins `len push pop keys str int float type assert` (ADR 0002).
  Registro: `changes/2026-11-14-feat-fase-3-stdlib-nucleo.md`.
- Fase 4 (modulos): `import "x.rhp"` com DFS/dedup/ciclo como erro (ADR 0003).
  Registro: `changes/2026-11-14-feat-fase-4-modulos.md`.
- Fase 5 (ferramentas): crate `rhp-fmt` + `rhp fmt [--check]` idempotente e
  `tools/check-changes.sh` de CI (ADR 0003).
  Registro: `changes/2026-11-14-feat-fase-5-ferramentas.md`.
- Escape `\u{HEX}` em strings.
  Registro: `changes/2026-11-14-feat-escape-unicode.md`.
- Limites de execução opt-in (`VmLimits`, `rhp run --max-steps N`).
  Registro: `changes/2026-11-14-feat-limites-execucao.md`.
- Robustez de testes: corpus malformado, property test determinístico,
  verificação de exemplos, `check-changes.sh` com renames.
  Registro: `changes/2026-11-14-test-robustez.md`.
- Benchmarks std-only + baseline documentado (sem otimizações).
  Registro: `changes/2026-11-14-bench-baseline.md`.
- Documentação: README reestruturado, `docs/`, `CONTRIBUTING.md`, `SECURITY.md`.
  Registro: `changes/2026-11-14-docs-checkpoint2.md`.
- Stdlib lote 2: 19 builtins (strings, matemática, mapas; ids 10–29).
  Registro: `changes/2026-11-15-feat-stdlib-lote2.md`.
- I/O e ambiente: `fs_read/fs_write/fs_append/fs_exists`, `argv()`, `env()`;
  `rhp run` repassa args extras.
  Registro: `changes/2026-11-15-feat-io-env.md`.
- `rhp test` (descoberta `*_test.rhp`) e `rhp doctor` (versão + autoteste).
  Registro: `changes/2026-11-15-feat-cli-test-doctor.md`.
- Sintaxe lote 2: bitwise, ternário, `??`, `break`/`continue`, `const`.
  Registro: `changes/2026-11-15-feat-sintaxe-lote2.md`.
- Laco `for x in ...` (array = elementos; mapa = chaves ordenadas),
  builtins `sort(array)` (copia ordenada estavel) e `exit(codigo)`
  (0-255; vira exit code no `rhp run`).
  Registro: `changes/2026-11-15-feat-laco-for-sort-exit.md`.
- Builtins `json_parse(texto)` / `json_stringify(valor)` (parser e
  serializador proprios, sem dependencias; teto de 128 niveis; erros
  com posicao).
  Registro: `changes/2026-11-15-feat-json.md`.
- Exceções: `try`/`catch`/`finally` + `throw` (RFC 0001), com propagação
  entre funções, `finally` em todos os caminhos (`exit()` não aciona) e
  erro como valor (`{tipo, mensagem}`).
  Registros: `RFCs/0001-excecoes.md`, `changes/2026-11-15-feat-excecoes.md`.
- Módulos v2 (ADR 0009): `export fn/let/const` opt-in e
  `import { a, b as c } from` seletivo (privacidade real, alias, colisão
  só entre visíveis; bare preserva o legado).
  Registro: `changes/2026-11-15-feat-modulos-v2.md`.
- Datas/horas std-only: `now()` (epoch), `sleep(s)` (teto 1h) e
  `date_str(ts)` (ISO UTC).
  Registro: `changes/2026-11-15-feat-datas-horas.md`.
- Rede v2: `http_fetch(url, opts)` (redirects, headers, timeout;
  `http_get` preservado). Fase A concluída.
  Registro: `changes/2026-11-15-feat-rede-v2.md`.
- SQLite via FFI mínimo ao `libsqlite3` (sem crate externa):
  `db_open/exec/query/close` (ADR 0011).
  Registro: `changes/2026-11-15-feat-sqlite.md`.
- PostgreSQL via protocolo próprio (mesmos `db_*`, prefixo `pg:`),
  testado contra servidor real. Fase B concluída.
  Registro: `changes/2026-11-15-feat-postgres.md`.
- Servidor HTTP: `http_serve(porta, handler, opts)` (RFC 0002; thread
  por conexão, 500 sem vazar, `exit` termina o servidor).
  Registro: `changes/2026-11-15-feat-http-serve.md`.
- Sessões e templates: cookies em `req`, `template()` com escape HTML,
  padrão sessão-via-banco documentado.
  Registro: `changes/2026-11-15-feat-sessao-template.md`.
- HTTPS: `http_fetch` com TLS verificado (rustls + raízes embutidas +
  `RHP_CA_BUNDLE`; ADR 0012). Fase D iniciada.
  Registro: `changes/2026-11-15-feat-https.md`.
- Pacotes v0: `rhp init`, `rhp pkg add/install [--check]`, `pacotes/` +
  `rhp.lock` com hash por arquivo.
  Registro: `changes/2026-11-15-feat-pacotes.md`.
- `rhp build` (bundle autocontido para deploy) + `limite_taxa` no
  servidor (429 com `Retry-After`).
  Registro: `changes/2026-11-15-feat-build-ratelimit.md`.
- Fase 6 (rede minima): builtin `http_get` std-only (so `http://`, 5s, 2xx)
  verificado contra servidor local; fronteira honesta para PostgreSQL/Redis,
  async, classes/generics (ADR 0004, planejados e NAO implementados).
  Registro: `changes/2026-11-14-feat-fase-6-rede-minima.md`.
- Diagnosticos estruturados: crate `rhp-diagnostic` (codigos E0xxx–E3xxx,
  `render` com snippet e `help`) + CLI com `Error[CODIGO]` (ADR 0007).
  Registro: `changes/2026-11-14-feat-diagnosticos.md`.
- Verificador de fluxo de pilha no bytecode + `VmLimits` estendido
  (`max_stack_depth`, `max_collection_len`) + runtime separado em
  `rhp-vm/src/builtins.rs` (ADR 0008).
  Registro: `changes/2026-11-14-feat-verificador-pilha.md`.
- Dedup de constantes e interning O(1) de nomes (sem mudanca semantica;
  tradeoff documentado) + `reports/MEMORY_MODEL.md`.
  Registro: `changes/2026-11-14-feat-memoria-cp3.md`.
- `rhp check [--json]` (lex+imports+compile sem executar), gramatica VSCode
  versionada com teste de consistencia, desenho de debugger
  (`docs/DEBUGGER_DESIGN.md`) — sem debugger improvisado (ADR 0010).
  Registro: `changes/2026-11-14-feat-devtools-check.md`.
- Modulos v2: apenas semantica proposta em ADR (sem codigo).
  Registro: `changes/2026-11-14-docs-modulos-v2.md`.
- Auditoria de maturidade Next Generation.
  Registro: `changes/2026-11-14-docs-nextgen-audit.md`.

### Fixed

- Teto de profundidade sintatica (128 niveis) e de constantes: entradas
  profundamente aninhadas geram erro em vez de abortar o processo.
  Registro: `changes/2026-11-14-fix-limite-profundidade.md`.
- `rhp fmt` com roundtrip garantido: floats sem notacao cientifica e chaves
  de mapa citadas quando necessario.
  Registro: `changes/2026-11-14-fix-fmt-roundtrip.md`.
- `http_get` com teto de corpo de 1 MiB e DNS documentado.
  Registro: `changes/2026-11-14-fix-http-limites.md`.
- Parametros duplicados rejeitados; `int()` estrito no limite de `i64`;
  `+x` como identidade; `import` confinado a arvore, teto de 8 MiB e
  apenas no nivel superior; `VmLimits`/`--max-steps` opt-in.
  Registros: `changes/2026-11-14-fix-p2-nucleo.md`,
  `changes/2026-11-14-feat-limites-execucao.md`.
- Escopo de `while`/`for` nao vaza em `break`/`continue` (`let` do corpo
  invisivel fora do laco).
  Registro: `changes/2026-11-15-fix-laco-escopo-break.md`.
