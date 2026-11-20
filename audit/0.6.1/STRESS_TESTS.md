# STRESS_TESTS.md — Carga controlada (com limites)

| Teste | Limite | Resultado |
|---|---|---|
| Parse/run repetido (`json_parse` simples) | 300× | OK; RSS 8928→8952 KiB (estável) |
| Servidor HTTP sequencial | 200 reqs | OK; encerrou limpo em `max_requisicoes` |
| String 9M via `+=` | timeout 120 s | Timeout (O(n²) documentado; sem crash/hang permanente) |
| Async 50 tarefas | — | OK em 0,1 s |
| Loop 200k / array 5k / JSON 128 | — | OK |
| Fuzz mutacional (2 rodadas, seeds distintas) | 3500 casos, timeout 5 s/caso | 0 crash/hang/panic |

Sem OOM intencional, sem fork bomb, sem alvos externos (só example.com via
`http_fetch`, 1 request). Containers de teste descartáveis removidos ao fim
(mysql×2, redis); pg mantido (dependência da suíte).
