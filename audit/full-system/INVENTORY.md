# INVENTORY.md — Inventário do RHP (auditoria full-system, pós-0.6.0)

Data: 2026-11-20. Branch: `audit/full-system` (sobre `main` = 0.6.0 lançado).
Baseline: `cargo test --workspace` 364 passed, 0 failed.

## Arquitetura (crates)

| Crate | Linhas (src) | Papel |
|---|---|---|
| `rhp-lexer` | ~1000 | Tokens UTF-8, `//` apenas, sem `&&`/`\|\|`/`and`/`or` |
| `rhp-parser` | ~4400 | AST + `Cursor{depth}` + teto `MAX_PARSE_DEPTH` (64, RHP-AUD-001) |
| `rhp-bytecode` | ~4400 | `Builtin` (80 ids), compilador, verificador de pilha, `MAX_VALUE_DEPTH=128` |
| `rhp-vm` | ~5500 (+json 423, sqlite 326, pg 420, mysql 748, redis 142, tcp 107, tls 88, time 104, http_server ~1300, extensions 193) | VM em pilha, builtins, conectores, `http_serve` |
| `rhp-fmt` | — | Formatador canônico idempotente |
| `rhp-diagnostic` | — | Códigos E0xxx–E3xxx, mapeamento `runtime_code()` |
| `rhp-cli` | — | `run/check/fmt/test/doctor/init/pkg/build/repl/lsp` |

## Biblioteca-padrão (80 builtins)

Len Push Pop Keys Str Int Float Type Assert HttpGet Contains StartsWith
EndsWith Split Join Trim Upper Lower Replace Abs Min Max Sqrt Pow Floor
Ceil Values Entries Merge Has FsRead FsWrite FsAppend FsExists Argv Env
Sort Exit JsonParse JsonStringify Now Sleep DateStr HttpFetch DbOpen
DbExec DbQuery DbClose HttpServe Template Map Filter Usort HashPassword
VerifyPassword RegexMatch RegexReplace RegexSplit Sha256 HmacSha256
HmacVerify Rand RandRange FsList Next Exhausted AsyncSpawn AsyncJoin
AwaitTimeout AsyncCancel AsyncCancelled AwaitAll AwaitRace TcpConnect
TcpSend TcpRecv TcpClose RedisOpen RedisExec RedisClose (+await:
`await` é palavra-chave, não builtin)

Contagem confere com `docs/REFERENCIA.md` (80).

## Testes existentes (baseline)

| Arquivo | Testes |
|---|---|
| `rhp-vm/src/lib.rs` | 86 unitários (+fuzz LCG de bytecode hostil) |
| `rhp-parser/src/lib.rs` | 52 (+fuzz de tokens) |
| `rhp-bytecode/src/lib.rs` | 41 |
| `rhp-lexer/src/lib.rs` | 17 (+fuzz de bytes) |
| `rhp-cli/tests/integration.rs` | 79 (HTTP, DB pg, exemplos, erros) |
| `rhp-cli/tests/conformance.rs` | 2 |
| demais (cli main, diagnostic, fmt) | 43+5+30 |

## Matriz de cobertura (pós-auditoria)

| Subsistema | Inventariado | Testado (novo) | Limitações | Status |
|---|---|---|---|---|
| Lexer | sim | fuzz 2500 casos-fonte | nenhuma nova | confiável |
| Parser | sim | cadeias RHP-AUD-001 + limites | teto 64 (intencional) | corrigido |
| AST | sim | — | sem `&&`/`\|\|` (RHP-AUD-002) | limitação P2 |
| Compiler | sim | regressão return/try (0.6.0) | — | confiável |
| Bytecode | sim | fuzz hostil existente | — | confiável |
| VM | sim | valores 300-deep em 2 MiB | — | confiável |
| Runtime/erros | sim | 22 mensagens 2-arg (RHP-AUD-003) | — | corrigido |
| Functions | sim | aridade/closures/recursão-limite | sem `in` membership (RHP-AUD-004) | limitação P2 |
| Exceptions | sim | matriz 19+20 + 8 casos return | — | confiável |
| Stdlib (80) | sim, todas | válidos+inválidos+aridade p/ todas | bool sqlite/mysql (doc) | 1 fix + docs |
| HTTP | sim | 15 testes + Docker 5/5 (0.6.0) | keep-alive/HTTPS/binário | conhecido |
| Database | sim, live pg/mysql/mariadb/sqlite/redis | RHP-AUD-008 (mysql seq) | caching_sha2 | corrigido |
| Filesystem | sim | leitura/escrita/erros/travessia | sem sandbox (documentado) | por desenho |
| Modules | sim | ciclo/ausente/cadeia 12 | — | confiável |
| CLI | sim | check/fmt/exit/argv/doctor | — | confiável |

Probes da auditoria: `/tmp/audit/` (p*, s*, a*, t*, d*, e*, h*, v*, w*) — não
versionadas (artefato de sessão); regressões permanentes adicionadas à suíte.
