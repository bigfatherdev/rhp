# ADR 0005 — Limites de robustez (Checkpoint 3)

- Data: 2026-11-14
- Estado: aceita (missão de engenharia, CHECKPOINT 3; sem breaking change)
- Contexto: auditoria AUD-001/AUD-104/AUD-105

## Contexto

Entradas patologicas (aninhamento profundo, corpo HTTP gigante) derrubavam o
processo (abort) ou permitiam exaustao de memoria. A linguagem precisa de
tetos deterministicos sem mudar programas validos existentes.

## Decisão

1. **Profundidade sintatica maxima: 128 niveis** (`MAX_PARSE_DEPTH`, parser).
   Cadeias planas nao acumulam. Acima: `ParseError` limpo.
   Constantes de bytecode artesanais: `MAX_VALUE_DEPTH = 128`, checado de
   forma iterativa em `validate()` (a propia validacao nao recursa).
2. **Corpo HTTP maximo: 1 MiB** (`MAX_HTTP_BODY`, leitura em blocos de 8 KiB
   com interrupcao antecipada). Acima: `VmError` limpo.
3. **DNS sem timeout proprio**: documentado como limitacao (resolvedor do
   sistema); sem thread dedicada por enquanto (custo/complexidade).

## Alternativas consideradas

- Recursao com pilha segmentada/`RUST_MIN_STACK`: rejeitado — mascara o
  problema e depende de ambiente.
- Aumentar pilha da thread principal: rejeitado — nao protege threads de
  teste nem embedding futuro.
- Recusar `http_get` por completo: rejeitado — o teto preserva o recurso.

## Consequências

- Programas com >128 niveis ( so patologicos/gerados) passam a falhar com
  erro em vez de abortar — mudanca de comportamento apenas para entradas que
  antes derrubavam o processo.
- Specs atualizadas (`LANGUAGE_SPEC.md` limite e escapes; teto HTTP).
