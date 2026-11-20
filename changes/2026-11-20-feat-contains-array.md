# [Added] — `contains` aceita array (RFC 0032)

- ID: 2026-11-20-feat-contains-array
- Data: 2026-11-20
- Versão: 0.7.0
- Tipo: Added
- Compatibilidade: Compatível (erro vira valor)

## Resumo

`contains(arr, valor)` => `Bool` por igualdade de valor (`==`); `[]` = `false`.

## Motivação

RHP-AUD-004: sem teste de pertinência, só loop manual.

## O que mudou

- Braço `(Value::Array, v)` em `Builtin::Contains` com `values_equal`
  (early-exit); mensagem passa a citar `(string, string) ou (array, valor)`.
- Docs: linha do `contains` na REFERENCIA.

## Detalhes técnicos

Sem mudança de ABI (mesmo builtin). Complexidade O(n).

## Compatibilidade e migração

`contains(array, _)` era erro; agora funciona. Migração: não necessária.

## Segurança e performance

Sem alocação; early-exit. Sem impacto mensurável.

## Testes e validação

- Vazios, tipos mistos, aninhados, promoção numérica, `NaN`, aridade,
  mensagem 2-arg; suite verde.

## Limitações e riscos conhecidos

Sem `in` (rejeitado na RFC); sem comparador custom (usar `filter`).

## Referências

- RFC 0032; `audit/0.6.1/FINDINGS.md` (RHP-AUD-004).
