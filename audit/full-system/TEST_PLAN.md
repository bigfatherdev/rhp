# TEST_PLAN.md — Plano executado (auditoria full-system)

Método: probes `.rhp` rotuladas + runner com timeout + asserções permanentes
na suíte. Nada destrutivo; FS isolado em `/tmp/audit/fs`; DBs em containers
descartáveis (`rhp-pg-test`, `rhp-mysql80`, `rhp-mysql-test`, `rhp-redis-test`).

## Executado

1. **Núcleo** (p01–p27): identificadores, operadores, strings/unicode/escapes,
   comentários, 4 classes de erro de parse, precedência, if/while/for/switch/
   match, aridade, recursão 600 (limite 512 limpo), closures/HOFs, try/catch
   matriz completa, rethrow, finally, throws tipados, não-capturado.
2. **Stdlib 80/80** (s01–s11): cada builtin com válido + tipo inválido +
   aridade (aridade = E2002 de compilação, por desenho). JSON (malformado,
   unicode, func, limites), FS, env/argv/time/template/assert/sort/usort,
   crypto, rand, regex, sqlite/pg/mysql/mariadb/redis lives, http/tcp client,
   async.
3. **Limites/stress** (t*, v*, w*, d*): ninhos 200–2000, cadeias 500–10000,
   string 100 KiB, mapa 5k, loop 200k, json 128–300, valores 120–300 em
   thread 2 MiB, 50 tasks, string 9M (O(n²) documentado, timeout controlado),
   arquivo 9 MiB (rejeitado limpo), módulos cadeia 12.
4. **Fuzz mutacional** (`fuzz.py`, 2 seeds): 2500 programas, 0 crash/hang/panic.
5. **Segurança**: overflow int, float/0, traversal (sem sandbox, documentado em
   `SECURITY.md`), secrets sweep (limpo), HTTPS real, env, exit codes.
6. **Performance**: `bench_rhp` release 2 runs (ver PERFORMANCE.md).
7. **HTTP**: métodos, HEAD, corpo 1 MiB+1, retorno inválido, 4 requests
   (complemento aos 15 testes de integração + Docker 5/5 da 0.6.0).

## Critérios de achado

Bug = comportamento contrário à spec/docs ou crash/hang. Limitação =
comportamento consistente porém restritivo (classificar P2–P4). Melhoria só
com benefício comprovado e escopo seguro (§20 da missão).
