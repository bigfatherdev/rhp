# PERFORMANCE.md — Benchmarks (sem números inventados)

Harness: `cargo run --release -p rhp-cli --example bench_rhp` (2 runs,
máquina compartilhada — mesma ressalva do baseline 0.6.0).

| Carga | 0.6.0 (baseline) | pós-auditoria (parser+cadeias) | Delta atribuível |
|---|---|---|---|
| lex 85 KB | 0,818 ms/op | 0,829 ms/op | nenhuma (lexer intocado) |
| parse 5000 stmts | 9,4–10,2 ms/op | 10,335 ms/op | ruído (custo novo: 1 contador+comparação por operador) |
| compile 5000 stmts | 15,0–16,1 ms/op | 15,477 ms/op | ruído |
| fib(20) | 78–96 ms/op | 81,4 ms/op | ruído |
| while 200k | 372–400 ms/op | 400,6 ms/op | ruído |
| push 20k | ~18–20 s/op | ~20,2 s/op | ruído (O(n²) documentado) |

Conclusão: **sem regressão atribuível** às mudanças (custo teórico do
`check_chain`: O(1) por operador contra o parse total; sem alocação).
Sem otimizações aplicadas (sem necessidade provada — regra do projeto).

Gargalos já documentados em `docs/BENCHMARKS.md` permanecem (`push` O(n²),
`fib` por clonagem de chunk, parse ~10× lex).
