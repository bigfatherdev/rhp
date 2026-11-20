# RFC 0031 (proposta) — Operadores logicos `&&` e `||` com curto-circuito

- ID: RFC 0031
- Status: aceita e implementada
- Data: 2026-11-20
- Versao: unreleased (implementada na 0.7.0)
- Depende de: nenhuma (usa `JumpIfFalse`/`JumpIfTrue` + `Constant` existentes)

## 1. Problema

RHP nao tem conjuncao/disjuncao logique (RHP-AUD-002): `&&`/`||`/`and`/`or`
nao existem; `&`/`|` rejeitam `Bool`. Compor condicoes exige `if` aninhado
ou ternario encadeado — ilegivel e propenso a erro em validacoes reais
(`if ok && x > 0 || emergencia` e impossivel hoje).

## 2. Objetivo e nao-objetivos

- Objetivo: `&&` (e) e `||` (ou) com curto-circuito, operandos estritamente
  `Bool` (coerente com `!` e ternario, que exigem `Bool`), sem truthiness.
- Nao-objetivos: `and`/`or`/`not` (aliases dispensaveis), truthiness
  (mudaria a filosofia estrita), XOR logico (use `!=` em bools).

## 3. Proposta

```rhp
if pronto && itens > 0 || emergencia {
  echo "vai";
}
let pago = recebido && conferido;   // Bool
```

-Tokens `&&`/`||` (maior prefixo em `&`/`|`); precedencia (menor→maior):
  ternario `?:` < `||` < `&&` < `??` < `==`/`!=` < `|` < ... (resto inalterado).
  `??` segue o mais frouxo dos tres (como PHP/C#); `&&` prende mais que `||`.
- Semantica: `a && b` = se `a` e falso, resultado falso SEM avaliar `b`;
  `a || b` = se `a` e verdadeiro, resultado verdadeiro SEM avaliar `b`.
  Resultado sempre `Bool`. Operando nao-`Bool` = erro E3003
  (`condicao deve ser bool`) — gratis via `JumpIfFalse`/`JumpIfTrue`.
- Compilacao: `a && b` = `<a>; JumpIfFalse F; <b>; AssertBool; Jump FIM;
  F: Constant(false); FIM:`. `a || b` analogo (`JumpIfTrue`/`true`).
  `AssertBool` e opcode novo (neutro na pilha; E3003 fora de `Bool`) pois o
  operando direito escapa aos jumps quando o esquerdo nao decide.
- AST: `BinOp::{LogicalAnd, LogicalOr}`; dobra constante em
  `eval_const_binary` (`true && x` etc. so com literais bool).

## 4. Alternativas

- Desacucar para `if`/ternario no parser: esconderia a estrutura e
  complicaria mensagens; jumps diretos sao o padrao do `??`/ternario.
- Truthiness: rejeitado (quebraria o contrato estrito documentado).
- Palavras `and`/`or`: rejeitado (duas sintaxes p/ mesma coisa; `&&`
  e universal).

## 5. Compatibilidade

Adicao pura: nenhum programa valido muda de parse (tokens novos `&&`/`||`
eram erro de parse antes). `&`/`|` inalterados. Precedencia nova so afeta
expressoes antes invalidas. Sem mudanca de bytecode existente.

## 6. Seguranca

Curto-circuito evita avaliar o lado direito (menos efeitos colaterais
inesperados). Teto de profundidade 64 conta elos `&&`/`||` (cadeia da
mesma familia). Sem panics novos (jumps existentes).

## 7. Performance

Custo zero no caminho quente (mesmos jumps do ternario); dobra constante
em literais. Bench `bench_rhp` deve ficar dentro do ruido.

## 8. Plano de implementacao

1. Lexer: `&&`/`||` (peek apos `&`/`|`). 2. `TokenKind::{LogicalAnd,LogicalOr}`
   + `BinOp::{LogicalAnd,LogicalOr}` + precedencias no parser/fmt.
3. `parse_logical_or/and` entre ternario e `??`. 4. Bracos dedicados em
   `compile_expr` (jumps + `Constant(Bool)`). 5. `eval_const_binary`.
6. `describe()`/fmt (strings, precedencia, parens).

## 9. Plano de testes

Tabela-verdade (4+4 casos); curto-circuito observavel (lado direito com
`throw`/efeito nao executa); nao-`Bool` = E3003; precedencia (`&&` sobre
`||`; `??` mais frouxo); misto com `??`/`==`/ternario; cadeia longa =
E1000; regressao na suite (nenhum programa valido muda).

## 10. Migracao e rollout

Nenhuma migração (adicao). Documentar precedencia com exemplos.

## 11. Criterios de aceitacao

Tabela-verdade verde; curto-circuito provado por teste; erro em nao-Bool;
suite 100% verde; docs (`REFERENCIA`, `LANGUAGE_SPEC`) com precedencia.
