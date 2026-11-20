# [Added] — Operadores lógicos `&&` e `||` (RFC 0031)

- ID: 2026-11-20-feat-logical-operators
- Data: 2026-11-20
- Versão: 0.7.0
- Tipo: Added
- Compatibilidade: Compatível (adição pura)

## Resumo

`&&` e `||` com curto-circuito, estritamente `Bool` (E3003 fora disso).
Precedência: ternario < `||` < `&&` < `??` < `==`. Opcode `AssertBool`
para o operando direito.

## Motivação

RHP-AUD-002: sem conjunção/disjunção, validações reais exigiam `if`
aninhado.

## O que mudou

- Lexer: `&&`/`||` (maior prefixo). Parser: `BinOp::{LogicalAnd,LogicalOr}`,
  `parse_logical_{or,and}`, precedência, `describe`, teto de cadeia.
- Bytecode: braços dedicados (`JumpIfFalse`/`JumpIfTrue` + `AssertBool` +
  `Constant(Bool)`), `eval_const_binary`, `AssertBool` no validador.
- VM: `Op::AssertBool` (espia o topo; E3003 `condicao deve ser bool`).
- Fmt: precedências 1/2 + símbolos; corrige parênteses de unário
  (bug encontrado pela suíte: `-(1+2)` perdia parênteses).
- Docs: REFERENCIA (precedência + semântica + divergência PHP),
  LANGUAGE_SPEC §3, BYTECODE_SPEC (`AssertBool`, tabela 64–82).

## Detalhes técnicos

Sem opcode de salto novo (reuso); `AssertBool` neutro na pilha.
Fuzz hostil cobre o opcode 53.

## Compatibilidade e migração

Programas válidos inalterados (`&&`/`||` eram erro de parse). Migração:
não necessária.

## Segurança e performance

Curto-circuito evita efeitos colaterais; teto 64 conta elos; bench sem
regressão atribuível.

## Testes e validação

- Tabela-verdade, curto-circuito observável, E3003 dir/esq, precedência
  (incl. `??` mais frouxo: `false ?? true || true` = `false`), `finally`/
  geradores/condições, cadeia longa = E1000.
- Suite verde; fmt idempotente; fuzz 300 casos com novas seeds.

## Limitações e riscos conhecidos

Sem `and`/`or` (rejeitado na RFC). `??` + `||` mistos seguem PHP/C#.

## Referências

- RFC 0031; `audit/0.6.1/FINDINGS.md` (RHP-AUD-002).
