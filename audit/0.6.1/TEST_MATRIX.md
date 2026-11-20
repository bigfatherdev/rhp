# TEST_MATRIX.md — Matriz de testes executados (independente)

Probes em `/tmp/v061/` + `/tmp/audit/` (artefatos de sessão) e regressões
permanentes na suíte (+3 testes: `runs_hof_throw_does_not_leak_stale_pending`,
`rhp_run_reports_both_arg_types` já existia da rodada anterior... ver lista).

| Área | Casos | Resultado |
|---|---|---|
| try/catch×return (8 variantes + estado pós-erro) | 20+ | OK; 1 bug P0 achado (IND-001) |
| try/catch×loop/recursão/HOF/gerador | 12 | OK após fix |
| HTTP métodos/headers/corpo/conexão | 25 | OK (1 probe-bug meu, não do runtime) |
| DB pg/mysql/mariadb/sqlite/redis | 30+ | OK; 2 bugs P1 achados (IND-002/003) |
| FS (unicode, erros, ciclo de vida) | 12 | OK; 3 builtins adicionados |
| async (join/timeout/cancel/race/all) | 8 | OK (documentado: cancel cooperativo) |
| Parser adversarial (ninhos, cadeias, incompletos) | 30+ | OK (teto 64 íntegro) |
| Stdlib 2ª rodada (tipos 3º arg, vazios, limites) | 40+ | OK; USAGE fix |
| CLI (check/fmt/test/build/repl/doctor/exit/argv) | 12 | OK; USAGE fix |
| Combinações (import+fn+try, json+fn, http+db) | 8 | OK |
| Stress (RSS, 200 reqs, 300 runs, string 9M c/ timeout) | 6 | OK (sem vazamento) |
| Fuzz mutacional (2 rodadas, seeds distintos) | 3500 | 0 crash/hang/panic |

Regressões permanentes adicionadas nesta rodada: `runs_hof_throw_does_not_leak_stale_pending`
(4 asserts), mock mysql seq-assert (0.6.1) + drain-assert, pg sync-assert,
`runs_fs_lifecycle_builtins`, `usage_documents_build_output_arg`.
(Herdadas da rodada anterior e revalidadas: cadeias profundas, mensagens 2-arg,
mysql seq, teto 64.)
