# ROADMAP.md — Planejamento e estado (missao Next Generation: CP1–CP7)

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

- [ ] Etapa 3 (suporte, em andamento): suite de conformidade
  (`tests/conformance/`), property tests std-only ampliados; pendentes:
  teto de heap em bytes, fuzz contínuo, otimizações com antes/depois.
- [ ] Etapa 4 (DevX): `rhp fmt` preservando comentários, stack traces,
  REPL, debugger/LSP.
- [ ] Etapa 5 (grandes, cada item com RFC/ADR + aprovação): tipos, OO/
  classes, geradores, async, Redis/MySQL/sockets.

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
