# [Added] — Teto de heap em bytes (RFC 0015, Etapa 3)

- ID: 2026-11-16-feat-heap-limit
- Data: 2026-11-16
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (opt-in; padrão ilimitado, sem limite = zero
  mudança de comportamento)

## Resumo

`VmLimits.max_heap_bytes: Option<u64>` + `rhp run --max-heap-bytes N`
(inteiro > 0; `0` = erro de uso, exit 2). Estouro = erro
`heap excedeu o limite ({max} bytes)` com código E3012. Exige RFC
prévia por `MEMORY_MODEL.md:37` — RFC 0015 redigida e aprovada antes
de qualquer código.

## Motivação

Dívida honesta de `SECURITY.md` ("heap sem teto medido em bytes"):
`max_collection_len` limita cada valor isolado, mas N valores pequenos
somam heap arbitrário. Para código não confiável (ex. `http_serve`),
faltava um orçamento global configurável.

## O que mudou

- Estimador `heap_size_of` (bytes do modelo: escalar 0, `Str` = len,
  `Array` = 8/elemento + recursão, `Map`/`Func.env` = chave + 8 por
  entrada + recursão; além de `MAX_VALUE_DEPTH`, só o nível imediato).
- Enforcement amortizado O(1) em `push_checked`: soma por push; ao
  cruzar `max(64, max/16)`, varre pilhas + escopos + globais + statics
  e reancora o contador (pops liberam orçamento).
- `set_limits` zera o contador. `run_file_limited` passa a receber
  `VmLimits` (3 chamadores atualizados). `USAGE` documenta a flag.

## Detalhes técnicos

- Arquivos: `crates/rhp-vm/src/lib.rs` (limite, estimador, varredura,
  1 teste), `crates/rhp-cli/src/main.rs` (flag + refator + 1 teste),
  `crates/rhp-diagnostic/src/lib.rs` (E3012 + assert).
- Sem `unsafe`; sem `unwrap` em caminho quente; `saturating_add` em
  toda soma do estimador.

## Compatibilidade e migração

Opt-in; nenhuma migração. `MEMORY_MODEL.md` perde o item "limites de
heap/stack medidos" do "A definir" (stack segue via `max_stack_depth`).

## Segurança e performance

Segurança: fecha DoS por acumulação para quem opta. Direção honesta:
o estimador mede o modelo, não o alocador (subestima; overhead de
`Vec`/`HashMap` fora) — teto rígido de RSS continua sendo
cgroup/ulimit (documentado em `MEMORY_MODEL.md` e no RFC).

Performance (protocolo antes/depois, `bench_rhp`, release, mesma
máquina — CPU compartilhada, alta variância):

| métrica | antes | depois (3 corridas) |
|---|---|---|
| lex 85KB | 4,410 ms/op | 4,145 (não tocado; ruído) |
| parse | 9,734 | 9,907 (não tocado; ruído) |
| compile | 14,527 | 14,299 (não tocado; ruído) |
| fib(20) | 82,254 | 99,151 / 84,100 / 81,603 |
| while 200k | 429,367 | 429,919 / 432,008 / 340,188 |
| push 20k | 18141 | 17858 / 17407 / 17503 |

Leitura: a variância entre corridas idênticas (±20% em `while`)
domina qualquer efeito; ANTES cabe dentro do intervalo DEPOIS. Delta
no caminho quente sem limite = 1 branch previsível (`None`).
Sem otimização de semântica; sem regressão evidenciada.

## Testes e validação

- `cargo test -p rhp-vm --lib respects_heap_byte_limit`: passou
  (rejeita ~1 KB com teto 512; teto 1 MiB passa; sem teto passa;
  aloca-e-libera 50x cabe em 2048).
- `cargo test -p rhp-cli --bin rhp` (inclui
  `rejects_heap_over_budget_via_loader`): 39 passed.
- `cargo test -p rhp-diagnostic`: 5 passed (inclui E3012).
- Gates finais: `cargo fmt --check` limpo,
  `cargo clippy --workspace --all-targets -- -D warnings` 0 warnings,
  `cargo test --workspace`: **290 passed, 0 failed** em 15 suites
  (288 baseline + `respects_heap_byte_limit` +
  `rejects_heap_over_budget_via_loader`).

## Limitações e riscos conhecidos

- Estimador ≠ alocador (ver Segurança). Loops que mantêm lixo vivo
  transitório entre varreduras podem estourar antes do pico real —
  direção fail-safe, documentada.
- Flags `--max-steps` e `--max-heap-bytes` são exclusivas por invocação
  (limitação herdada do parser da CLI, não introduzida aqui).
- Sem casos de heap na suite de conformidade (teto é opt-in; padrão
  ilimitado não altera nenhuma saída pinada).

## Referências

- RFC 0015 (`RFCs/0015-limite-heap-bytes.md`, aceita e implementada);
- `MEMORY_MODEL.md`, `VM_SPEC.md`, `SECURITY.md`;
- `reports/PHP_PARITY_AUDIT.md` (lacuna de limites).
