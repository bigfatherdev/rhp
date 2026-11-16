# RFC 0009 — Argumentos nomeados e spread em chamada

- ID: RFC-0009
- Data: 2026-11-16
- Estado: aceita (programa RHP-100; implementada na Fase 4)
- Contexto: completar a historia de chamada (defaults+variadicos existem
  desde a RFC 0006, mas so posicionais). Dia a dia PHP: `f(x: 1)`, `f(...$a)`.

## 1. Problema

Chamadas RHP sao estritamente posicionais. Falta nomear (`f(x: 1)`,
auto-documentado, imune a reordenacao) e espalhar (`f(...arr)`, aridade
dinamica).

## 2. Objetivo e não-objetivos

Objetivo: `f(x: 1, y: 2)` (qualquer ordem), `f(...arr)` (posicional,
splice em ordem, multiplos e misturados ok), combinados com defaults e
`...rest`. Nao-objetivos: spread em literais de array (`[...a]` = futuro,
exige maquinaria propria); named em builtins (erro de compilacao — aridade
fixa); `...` como parametro formal fora do `...rest` (ja existe).

## 3. Decisao de desenho

- Sintaxe: `NomeArgs := Ident ":" Expr` (posicao de argumento); `...Expr`
  (spread). `:` apos `Var` sem `?` anterior = nomeado (ternarios consomem
  o proprio `:`; sem ambiguidade). `f(1 : 2)` (nao-Ident) = erro de parse.
- Regras estaticas (compilacao): posicional apos nomeado = erro
  (`f(x: 1, 2)`); nome duplicado = erro. (Melhor que o PHP, que erra em
  runtime — visivel no fonte.)
- Runtime (`Op::CallMixed(desc)` + descritor nas constantes como
  `Value::Array` de `Int(0)`=posicional, `Int(1)`=spread, `Str`=nomeado):
  valores na pilha em ordem do fonte; percorre o descritor, expandindo
  spreads (nao-`Array` = erro) e montando mapa de nomeados (duplicata
  dinamica impossivel — spreads so geram posicionais).
- Ligacao: posicionais preenchem em ordem; nomeados por nome; ausentes usam
  `defaults`; excedentes vao para `...rest` (sem ele = erro); nome
  desconhecido = erro; named que repete posicao ja preenchida = erro
  (como no PHP); `null` explicito e valor.
- `CallMixed` so alimenta o caminho `Call` (LoadName ou callee arbitrario);
  builtin com nomeado/spread = erro de compilacao (aridade fixa).
- Validador: `desc` dentro das constantes e `Array` (falha rapida);
  efeito de pilha `-(1+n)+1` (n = slots do fonte).
- Sem `Call` novo para o caso comum: chamadas so-posicionais continuam
  `Call` byte a byte (zero churn no existente).

## 4. Alternativas consideradas

- Dois arrays (posicional + mapa) com ops existentes: spread no meio
  (`f(a, ...r, b)`) exigiria append — sem op de append (copia O(n));
  rejeitado.
- Erro em runtime (PHP) para posicional-apos-nomeado/duplicado: rejeitado
  (visivel no fonte; compilador ja sabe).
- Named em builtins: rejeitado nesta fase (aridades fixas checadas).

## 5. Compatibilidade

Aditivo. `f(x: 1)` antes era erro de parse (esperava `,`/`)` apos `x`);
`...` em chamada idem. Nenhum programa valido muda.

## 6. Seguranca

Sem `unsafe`; descritor validado (bounds + tipo); erros de ligacao sao
`VmError` (capturaveis); sem `Send` novo.

## 7. Performance

Um `Array` descritor por call-site misto (sem dedup — mesma classe das
constantes compostas); bind O(params + args).

## 8. Plano de implementacao

1. Parser: `CallArg::{Pos(Expr), Named(String, Expr), Spread(Expr)}`?
   OU `Expr` + wrapper... Decisao: `enum CallArg` dedicado; `parse_call_args`
   retorna `Vec<CallArg>`; regras estaticas; `Expr::Call/CallValue.args`
   viram `Vec<CallArg>` (ripple controlado em compiler/fmt/CLI/testes).
2. Bytecode: descritor const + `Op::CallMixed`, `call_mixed_slots`,
   validador, ligacao (`push_call_frame_mixed`), testes.
3. VM: braco `CallMixed` + erros; `Call` intacto.
4. Fmt (preserva forma) + CLI (renamer/checkers nos args) + specs
   (`LANGUAGE_SPEC` gramatica + regras, `BYTECODE_SPEC` op, `PHP_DIFFERENCES`:
   builtin sem named, erro estatico vs PHP, `[...a]` futuro) + testes + `changes/`.

## 9. Plano de testes

- Parse: nomeado, ordem trocada, spread, misto, erros (pos-apos-nome,
  duplicado, `1 : 2`, builtin com nomeado).
- VM: ligacao por nome, ordem irrelevante, spread splice/multiplo,
  `f(a, ...r, b)`, defaults+variadico combinados, desconhecido, conflito
  posicional+nomeado, `null` explicito, `...` nao-array, aridades.
- Fmt idempotente + reparse; CLI rename; suite sem regressao.

## 10. Migracao e rollout

Nenhuma (aditivo; formas novas eram erro).

## 11. Criterios de aceitacao

- [ ] Testes da §9 passam; suite sem regressao.
- [ ] `fmt`/`clippy`/`test` + `check-changes.sh` verdes.
- [ ] Specs + divergencias em `PHP_DIFFERENCES.md`.
- [ ] Esta RFC marcada `aceita` antes do merge.
