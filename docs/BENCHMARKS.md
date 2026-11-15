# BENCHMARKS.md — Metodologia e baseline do RHP

> Medir antes de otimizar. Nenhuma otimizacao foi aplicada a partir destes
> numeros; eles sao o baseline para comparacoes futuras (antes/depois).

## Como rodar

```bash
cargo run --release -p rhp-cli --example bench_rhp
```

Harness 100% std (sem crates externas): gera fonte sintetica, mede com
`std::time::Instant` apos 1 iteracao de aquecimento e imprime media por
operacao. Separe **compilacao** (`lex`/`parse`/`compile` da mesma fonte) de
**execucao** (chunks compilados uma vez, executados N vezes). Rode em
`--release`; modo `debug` nao e comparavel.

## Baseline (2026-11-14)

Maquina: AMD EPYC (6 vCPU, ambiente compartilhado), Linux,
`rustc 1.100.0`, workspace no commit do Checkpoint 3.

| Carga | N | Total | Media/op |
|---|---|---|---|
| lex 85 KB (5000 `echo 1 + 2 * 3;`) | 20 | 17,458 ms | 0,873 ms |
| parse 5000 stmts | 20 | 184,145 ms | 9,207 ms |
| compile 5000 stmts | 20 | 204,940 ms | 10,247 ms |
| run fib(20) recursivo | 5 | 496,640 ms | 99,328 ms |
| run `while` 200k iteracoes | 3 | 1112,751 ms | 370,917 ms |
| run `push` 20k em loop | 3 | 31397,513 ms | 10465,838 ms |

> Nota CP3 (2026-11-14): apos dedup de constantes no `compile`, nova medicao
> do mesmo harness: `compile 5000 stmts` ~13,5 ms (antes ~10,2 ms) em CPU
> compartilhada com alta variancia entre runs (ex.: `lex` 1,0→4,3 ms).
> O pool do fixture caiu de 15003 para 5 entradas (deterministico).
> Decisao: manter o dedup (chunk menor e previsivel; custo de hash pequeno
> e constante) e nao otimizar alem disso sem novo protocolo — ver
> `reports/MEMORY_MODEL.md`.

## Leitura dos gargalos (analise, sem mudanca de codigo)

1. **`push` em loop e O(n²) por construcao**: cada `push` clona o array
   inteiro (semantica por valor, ADR 0002). 20k pushes ~ 200M de copias de
   `Value`. Nao e bug; mudar para referencia compartilhada seria mudanca
   semantica (breaking) e exige RFC + aprovacao.
2. **Parse ~10x o lex**: `parse_primary` clona `Token` (com `String`) por
   no e cada literal aloca; `compile_expr`/`compile` tambem clonam.
   Candidatos a micro-otimizacao (tokens por referencia, interning de
   constantes, `HashMap` no pool de nomes) — so com protocolo antes/depois
   nesta mesma harness e suite verde.
3. **`fib(20)`**: ~21891 chamadas em ~100 ms (~4,5 us/chamada com clone de
   chunk por ativacao). Clonagem de `Chunk`/`FuncDef` por chamada (ADR 0001,
   limitacao conhecida) e o custo dominante; arena/internacao e trabalho
   futuro com ADR propria.

## Regras para futuras otimizacoes

- Reproduzir o baseline nesta harness antes e depois, mesma maquina/modo.
- Tempo, memoria (observar RSS) e suite completa verde (`fmt`, `clippy`,
  120+ testes) como criterios de aceite.
- Nunca trocar semantica por velocidade (governanca §2); ganho sem prova
  de preservacao e regressao.
- Registrar metodo e numeros no `changes/` correspondente.
