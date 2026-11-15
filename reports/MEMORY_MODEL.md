# MEMORY_MODEL.md — Investigacao, decisao e alternativas (Checkpoint 3)

- Data: 2026-11-14 · Ramo: `feat/next-generation` · Missao §4
- Regra: nao alterar a semantica de arrays/mapas por valor sem ADR + testes
  de compatibilidade + spec (governanca §3–§4, §10). Nada aqui muda semantica.

## 1. Modelo atual (verificado no codigo)

- `Value::{Null, Bool, Int, Float, Str, Array(Vec), Map(HashMap), Func(Rc)}`;
  tudo `Clone`; **semantica por valor**: atribuicao, chamada e `push`
  copiam integralmente (ADR 0002). Sem aliasing observavel, sem coleta.
- Alocacao: pool de constantes por chunk + pilha de operandos + escopos
  (`HashMap` por escopo) + globais. Frames clonam `Chunk`/`FuncDef` por
  ativacao (limitacao conhecida, ADR 0001).
- Pinos de compatibilidade (testes que travam a semantica): igualdade por
  conteudo, `push` retorna novo array, primeira chave duplicada vence,
  `Display` deterministico.

## 2. Medicoes (antes de decidir)

Ambiente: AMD EPYC (6 vCPU, compartilhado), `rustc 1.100.0`, release.

| Experimento | Resultado |
|---|---|
| `push` 20k em loop | ~10 s release (~98 s debug); pico RSS **8 MB** |
| Array literal 100k itens (300 KB fonte) | 0,6 s, ok |
| Cadeia de 200 imports | 14 ms totais |
| Pool de constantes, fonte 85 KB repetitiva (5000 stmts) | **15003 entradas antes** |
| `int(2^63)`, `nan==nan`, `{a:1,a:2}` | comportamento pinado (ver specs) |

Conclusao da medicao: o custo e **tempo de copia** (O(n²) no `push` em
loop), nao retencao — o pico RSS mantem-se baixo porque cada iteracao
libera o array anterior. Nao ha vazamento observavel nesse padrao.

## 3. Alternativas comparadas

| Abordagem | Efeito | Custo/risco | Veredito |
|---|---|---|---|
| Dedup de constantes escalares no `compile` | Pool 15003 -> 5 no fixture; `validate` e clones mais baratos | Hash por literal (~30% a mais no `compile` repetitivo, medido 10,2 -> ~13,5 ms com variancia de CPU compartilhada) | **Adotar** (semantica identica; prova: suite + teste dedicado) |
| `intern_name` via `HashMap` (era O(n)) | Lookup O(1) | Troca localizada, sem mudanca observavel | **Adotar** |
| Compartilhamento interno (`Rc`) / CoW | Eliminaria copias O(n²) | Muda custo observavel de `push`/atribuicao? Nao muda resultado, mas muda **modelo de custo** prometido e exige re-auditoria de aliases; sem RFC | **Descartar por agora** (requer RFC + testes de equivalencia) |
| Reference counting / GC | Resolve retencao em ciclos (que nao existem: sem referencias) | Complexidade sem problema correspondente; `Rc` ja usado p/ `Func` | **Descartar** (YAGNI; revisitar se surgirem referencias) |
| Arena/internacao de chunks | Reduz clone por ativacao | Ganho real, mas toca execucao quente; exige antes/depois dedicado | **Futuro** (registrado, nao implementado) |

## 4. Decisao

1. **Manter semantica por valor** (contrato da linguagem; ADR 0002 intacto).
2. **Adotar apenas**: dedup de constantes escalares + interning O(1) de
   nomes — ambos sem efeito observavel, provados pela suite (inclui
   asserts exatos de bytecode) + teste novo de pool.
3. **Nao adotar**: CoW/RC/GC/arena nesta missao (registrar como trabalho
   futuro com protocolo de medicao da secao 2).

## 5. O que mudou neste checkpoint

- `rhp-bytecode`: `ConstKey` + mapas no `Compiler`; `intern_name` O(1).
- Teste: pool de 2000 literais iguais => 1 entrada.
- `docs/BENCHMARKS.md`: baseline mantido + nota sobre variancia e tradeoff.
- Root `MEMORY_MODEL.md`: atualizado para o modelo real (era o stub da Fase 1).
