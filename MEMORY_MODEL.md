# MEMORY_MODEL.md — Modelo de memoria do RHP

Estado real (revisado na missao Next Generation; antes era o stub da Fase 1).
Decisao arquitetural e alternativas em `reports/MEMORY_MODEL.md` (+ ADR 0002).

## Modelo vigente: valores por copia, sem compartilhamento

- `Value::{Null, Bool, Int, Float, Str, Array(Vec<Value>),
  Map(HashMap<String, Value>), Func(FuncValue)}` com
  `FuncValue { def: Rc<FuncDef>, env: HashMap<String, Value> }`; tudo `Clone`.
- **Semantica por valor**: atribuicao, passagem de argumentos, retorno e
  `push` copiam integralmente o valor. Nao ha aliasing observavel:
  mutar via uma variavel nunca afeta outra (inclui `a[i] = v`, atomico).
- **Sem coleta**: nao ha GC nem contagem de referencias sobre valores
  (o unico `Rc` interno e para definicoes de funcao, imutaveis e
  compartilhadas por leitura). Sem ciclos possiveis, sem finalizadores.
- **Onde vivem os valores**: pool de constantes do chunk (deduplicado para
  escalares no `compile`), pilha de operandos por frame, escopos lexicos
  (`HashMap` por bloco) e mapa de globais. Frames sao descartados no
  `return`/`Halt`, liberando seus valores (sem `Drop` customizado).
- **Custos assumidos**: copias O(n) por `push`/atribuicao de compostos
  (documentado; mudar isso e mudanca semantica e exige RFC). `VmLimits`
  (`max_steps`) e tetos de HTTP/fonte/profundidade limitam crescimento
  patologico; sem teto geral de heap (ver `SECURITY.md`).

## Fora do modelo (nao prometido)

Ownership estatico, borrow checker da linguagem, threads/async, limites de
memoria medidos em bytes, compactacao, weak refs. Ver "A definir" abaixo
para o que ainda exige RFC.

## A definir (requer RFC antes de implementar)

- Compartilhamento interno (CoW/`Rc`) ou arena para ativacoes.
- Internacao de strings em runtime.
- Concorrencia e excecoes/unwinding.
- Limites de heap/stack medidos.
- Compatibilidade de bytecode persistido (hoje nao serializado).
