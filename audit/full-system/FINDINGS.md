# FINDINGS.md — Matriz de descobertas (IDs estáveis RHP-AUD-*)

Legenda prioridade: P0 crítico (crash/corrupção/dados) · P1 alto (recurso
essencial quebrado) · P2 médio (limitação com workaround) · P3 baixo
(ergonomia/docs) · P4 evolução futura.

| ID | Cat. | Problema | Evidência | Impacto | Prior. | Correção |
|---|---|---|---|---|---|---|
| RHP-AUD-001 | Parser | Stack overflow nativo (SIGABRT 134) com cadeias planas longas (`1+1+...`, `!!!`, `?:`, `??`, `.x`, `[0]`, `()`) — sem teto antes | `/tmp/audit/t03*.rhp`, `t10–t13`; `fmt`/`check`/`run` abortavam | Crash com input não confiável (P0 segurança) | P0 | **Corrigido**: `check_chain` + `nest` nas recursões; teto uniforme 64; regressão `rejects_excessive_flat_chains` + 2 testes CLI |
| RHP-AUD-002 | Linguagem | Sem `&&`/`\|\|`/`and`/`or` (só `&`/`\|` bitwise, que rejeitam `Bool`) | `p02`, `p11`, gramática `BitOr/Xor/And` sem `LogicalAnd/Or` | Composição booleana exige `if` aninhado | P2 | Planejado (exige RFC/gramática) — não implementado |
| RHP-AUD-003 | Diagnóstico | 22 builtins 2-arg reportavam só o tipo do 1º arg (`split("a,b",1)` dizia "encontrado string") | `s01`, `s05`; padrão `(other, _)` | Diagnóstico enganoso | P3 | **Corrigido**: ambos os tipos (`(string, int)`); regressão CLI |
| RHP-AUD-004 | Stdlib | Sem pertinência em array (`has` é só mapa; sem `in`) | `s02`, `p25` | Loop manual p/ membership | P2 | Planejado (RFC) — não implementado |
| RHP-AUD-005 | Stdlib/docs | Bool gravado volta `1` (`Int`) no SQLite/MySQL (sem bool nativo); docs listavam `Bool` genérico | `s08`, `d10` vs PG (`true` ok) | Surpresa de tipos | P3 | **Docs**: nota em REFERENCIA `db_query` |
| RHP-AUD-006 | Docs | `min`/`max` binários; `sort` sem comparador; `assert` 1-arg; `env` 1-arg — probes assumiram errado, docs corretas | s03/s06/s11 | Nenhum (docs certas) | — | Nenhuma (verificado) |
| RHP-AUD-007 | Parser/docs | Sem `/* */` (só `//`); `and`/`or` inexistentes; `in` inexistente | `p08`, gramática | Nenhum (documentado) | — | Nenhuma (verificado) |
| RHP-AUD-008 | MySQL | `db_open("mysql:")` conectava mas TODO comando falhava (1156): `seq` não zerava por comando | `d10` MariaDB 11 + MySQL 8.0 (native) | Conector 100% inoperante | P1 | **Corrigido**: `self.seq = 0` em `command()`; mock agora asserta seq 0 (falha sem o fix); validado live nos 2 servidores |
| RHP-AUD-009 | MySQL | `caching_sha2_password` (default MySQL 8): handshake falha ("resposta inesperada") — cliente só fala `mysql_native_password` (documentado) | `d10_mysql80` (1º round) | Requer usuário native ou flag de servidor | P2 | Planejado (troca de auth completa) — documentado |
| RHP-AUD-010 | Segurança | FS sem sandbox (`/etc/passwd` legível): por desenho (`SECURITY.md:28`) | `e03` | Nenhum (rodar código confiável) | Info | Nenhuma (confirmado documentado) |
| RHP-AUD-011 | Performance | Concat `+=` e `push` O(n²) por semântica de valor (string 9M estoura timeout 120 s) | `e04`, bench `push` | Cliff conhecido/documentado | P3 | Nenhuma (semântica; builder seria RFC) |
| RHP-AUD-012 | Compat | Teto sintático 128→64 (RHP-AUD-001 exigiu margem real p/ threads 2 MiB debug) | medição `stack_probe`: n=120 ok, n=128 aborta → 64 = ~2× margem | Cadeias 65–128 agora rejeitadas (patológicas) | P2 | **Aplicado** + spec/docs/teste 100→50 (justificado, não silencioso) |

Descobertas 0.6.0 já entregues (return/try, bind, HTTP) não repetidas aqui.
Fuzz: 2500 casos mutacionais, 0 crash/hang/panic (pós-correção 001).
