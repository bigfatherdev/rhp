# PERFORMANCE.md — Comparativo (sem números inventados)

Harness `bench_rhp`, release, mesma máquina (compartilhada).

| Carga | 0.6.1 (baseline) | pós-mudanças desta rodada | Delta atribuível |
|---|---|---|---|
| lex 85 KB | 0,83 ms/op | 0,83 ms/op (lexer intocado) | nenhum |
| parse 5000 | 9,4–10,3 ms/op | ~10,3 ms/op | ruído (`check_chain`: O(1)/operador) |
| compile 5000 | 15,0–16,1 ms/op | ~15,5 ms/op | ruído (só +1 campo em `Frame`) |
| fib(20) | 78–96 ms/op | ~81 ms/op | ruído |
| while 200k | 372–400 ms/op | ~400 ms/op | ruído |
| push 20k | ~18–20 s/op | ~20 s/op | ruído (O(n²) documentado) |

`pending_cb` (+8 B/frame) e `check_chain` (+1 int/elo) sem impacto mensurável.
Sem otimizações (sem necessidade provada). Sem regressão.
