# RFC 0015 — Teto de heap medido em bytes (`VmLimits.max_heap_bytes`)

- ID: RFC-0015
- Data: 2026-11-16
- Estado: aceita (2026-11-16) e implementada
- Contexto: Etapa 3 (suporte). Fecha a dívida honesta de `SECURITY.md`:
  "heap sem teto medido em bytes". `max_collection_len` limita cada
  valor isolado; falta um orçamento global (soma de pilha + escopos +
  globais + statics).

## 1. Problema

Programa RHP pode acumular valores sem teto global: `max_steps` limita
tempo, `max_collection_len` limita cada valor, mas N valores pequenos
somam heap arbitrário. Para hospedar código não confiável (ex.:
`http_serve`), falta um orçamento de memória configurável.

## 2. Objetivo e não-objetivos

Objetivo: `VmLimits.max_heap_bytes: Option<u64>`, opt-in, padrão
`None` (= ilimitado, comportamento atual bit a bit). Erro claro ao
estourar; flag CLI `--max-heap-bytes` (segue `--max-steps`).

Não-objetivos: contabilidade do alocador real (jemalloc/sistema);
cotas por frame; GC ou mudança de representação (sem RFC própria);
mudar o padrão (sempre ilimitado sem opt-in); derrubar `push` O(n²)
(essa semântica continua; o teto só limita o dano).

## 3. Proposta

- Estimador `heap_bytes(&self) -> u64`: soma sobre frames (pilha +
  escopos), globais e statics. `size_of_val`: escalar = 0 (constante
  documentada); `Str` = `len`; `Array` = 8·len + recursão; `Map` =
  Σ(key.len + 8 + recursão). Sem ciclos possíveis (valores por cópia),
  logo recursão é segura; usa o `MAX_VALUE_DEPTH` existente como trava.
  Documentado como **estimativa conservadora do modelo**, não do
  alocador (overhead de `HashMap`/`Vec` não medido — direção fail-safe
  declarada no erro e na spec).
- Enforcement amortizado O(1): contador `alloc_since_check` soma a
  estimativa de cada valor em `push_checked`; ao cruzar
  `max(1 MiB, limite/16)`, faz a varredura completa e zera o contador
  com o valor real (pops liberam orçamento — loops legítimos passam).
  Erro: `heap excedeu o limite ({max} bytes)` + entrada no mapeamento
  do `rhp-diagnostic` (novo E3xxx na faixa de limites).
- CLI: `rhp run --max-heap-bytes N` → `VmLimits`; rejeita N=0 com erro
  de uso.

## 4. Alternativas

- Varredura O(heap) a cada instrução: correta, mas taxa fixa sobre todo
  programa limitado — rejeitada (GOVERNANCE §11: sem taxa sem prova).
- Contador monotônico sem reset (só soma): O(1) e simples, mas quebra
  loops legítimos que alocam e liberam — rejeitada (falso-positivo por
  desenho).
- Contar só `Vec`/`String` alocados (sem estimador): exigiria `unsafe`
  ou tracking invasivo — rejeitado (sem `unsafe`, §9).

## 5. Compatibilidade

Aditivo e opt-in: sem limite configurado, zero instruções novas no
caminho quente (`if let Some` em `push_checked`, já existente para
`max_collection_len`). Bytecode, linguagem e stdlib inalterados.
`MEMORY_MODEL.md` §"A definir" perde o item "limites de heap/stack
medidos" (stack segue via `max_stack_depth`; heap passa a medido).

## 6. Segurança

Fecha DoS por acumulação para quem opta pelo teto. Direção fail-safe:
estimador subestima o alocador real (overhead não contado) — o teto
dispara *antes* do consumo real equivalente? Não: subestimar significa
permitir mais que o nominal. Declarar honestamente: o teto limita o
**modelo**, com margem; quem precisa de teto rígido de RSS usa
cgroup/ulimit (documentar). Sem `unsafe`, sem `unwrap` em caminho
quente (erros via `VmError`).

## 7. Performance

Caminho sem limite: +1 `Option::is_some` por push (ruído). Com limite:
O(1) amortizado; varredura completa só a cada ≥1 MiB estimado.
Protocolo antes/depois no bench (`bench_rhp` + suite): critério =
sem regressão >2% no `run` sem limite; com limite, custo da varredura
diluído (medir e registrar em `docs/BENCHMARKS.md`).

## 8. Plano de implementação

1. `VmLimits.max_heap_bytes` + `heap_bytes`/`size_of_val` + contador
   amortizado em `push_checked` (+ teste que falha primeiro).
2. Flag `--max-heap-bytes` na CLI + mapeamento diagnóstico.
3. Docs: `MEMORY_MODEL.md`, `VM_SPEC.md` (limites), `SECURITY.md`
   (troca "sem teto" por opt-in), `CHANGELOG.md`, `ROADMAP.md`,
   `changes/` + exemplo `examples/limites.rhp` (opcional, se couber
   sem rede/fs).
4. Gates: fmt, clippy `-D warnings`, workspace verde, bench antes/depois.

## 9. Plano de testes

- Limite pequeno rejeita `[range grande]`/string gigante com a mensagem.
- Padrão (`None`) roda a suite inteira inalterada (conformidade passa).
- Loop aloca-libera com limite folgado passa (valida o reset).
- Limite folgado >> uso passa; `N=0` na CLI = erro de uso.
- Fuzz artesanal existente de bytecode inválido continua verde.

## 10. Migração e rollout

Nenhuma migração: opt-in, padrão ilimitado. Rollout = merge normal;
uso recomendado documentado para `http_serve` não confiável.

## 11. Critérios de aceitação

- [ ] RFC aprovada pelo responsável (sem implementação antes).
- [ ] Estimador documentado como aproximação do modelo (não do alocador).
- [ ] 289+ testes verdes, sem regressão de bench >2% sem limite.
- [ ] Docs e `changes/` completos; `check-changes.sh` ok.
