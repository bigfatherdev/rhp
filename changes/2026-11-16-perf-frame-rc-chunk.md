# [Performance] — Frames compartilham chunk por `Rc` (Etapa 3)

- ID: 2026-11-16-perf-frame-rc-chunk
- Data: 2026-11-16
- Versão: unreleased
- Tipo: Performance (refatoração interna, semântica preservada)
- Compatibilidade: Compatível (tipos `pub` `FuncDef.body`/`Chunk.funcs`
  mudam de representação interna; linguagem, bytecode textual e
  comportamento observável inalterados)

## Resumo

`Frame` guardava `code`/`constants`/`names`/`funcs` clonados por
ativação; agora guarda `chunk: Rc<Chunk>` (bump de refcount por `Call`).
`FuncDef.body: Chunk` → `Rc<Chunk>`, `Chunk.funcs: Vec<FuncDef>` →
`Vec<Rc<FuncDef>>`. Prova de semântica: 294 testes verdes (inclui
conformidade, property, fuzz, integração).

## Motivação

Último item da Etapa 3 (otimizações com protocolo antes/depois,
GOVERNANCE §11). Alvo medido: `fib(20)` pagava clone de chunk por
chamada (~22k chamadas).

## O que mudou

- `crates/rhp-bytecode`: tipos + 1 ponto de construção (`Rc::new`).
  `validate` e testes de índice funcionam sem alteração (auto-deref).
- `crates/rhp-vm`: `Frame` com 1 campo `Rc` no lugar de 4 `Vec`s;
  `reset` (1 clone por `run`), `activate` (1 bump por chamada);
  5 pontos de leitura (`code`/`constants`/`names`/`funcs` →
  `chunk.*`).
- Sem `unsafe`; sem mudança em valores owned da pilha/escopos.

## Compatibilidade e migração

Tipos `pub` mudam; sem usuários externos (workspace fechado, 0.x).
`Display`/comparação de `Chunk` preservados (`Rc` delega).
`MEMORY_MODEL.md`: semântica por valor intacta (só metadados
imutáveis compartilhados).

## Segurança e performance

Protocolo antes/depois (`bench_rhp`, release, mesma máquina).
Descoberta honesta: a 1ª corrida do ANTES (fib 134,9) era outlier
(CPU compartilhada); com 3+3 corridas:

| métrica | antes (3) | depois (3) | leitura |
|---|---|---|---|
| fib(20) | 134,9 / 81,5 / 83,0 | 78,2 / 80,5 / 78,9 | **~-4%** (sem overlap) |
| push 20k | 19825 / 16064 / 15838 | 16573 / 16233 / 16227 | ~0% (igual) |
| while 200k | 437 / 336 / 364 | 374 / 342 / 365 | ~0% (sem chamadas) |
| lex/parse/compile | oscila ±60% | oscila ±60% | ruído (nada tocado lá) |

Ganho pequeno e consistente no alvo (`Call`); resto inalterado.
Parse sem alvo com prova → não otimizado (§11: sem prova, sem
otimização). Critério RFC (>2% no alvo): atendido no `fib`.

## Testes e validação

- `cargo test --workspace`: **294 passed, 0 failed** (mesmo total do
  `main`; só testes, nenhum comportamento alterado).
- Suite de conformidade (`tests/conformance/`, 24 casos pinados):
  verde — prova direta de semântica preservada.
- `cargo fmt --check` limpo; `clippy -D warnings` 0 warnings.

## Limitações e riscos conhecidos

- Retenção: chunk vive enquanto algum frame o referencia (antes era
  clonado e liberado no `return`); teto = profundidade de chamada,
  desprezível. Teto de heap (`max_heap_bytes`) não conta metadados
  compartilhados (só valores vivos) — documentado como estimador.
- Tipos `pub` alterados: notar no `CHANGELOG` (feito).

## Referências

- GOVERNANCE §11; `docs/BENCHMARKS.md` (metodologia);
- `reports/MEMORY_MODEL.md` (decisão CoW/Rc adiada — este passo usa
  `Rc` só em metadados imutáveis, sem mudar a semântica por valor).
