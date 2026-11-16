# RFC 0006 — Parâmetros default e variádicos

- ID: RFC-0006
- Data: 2026-11-16
- Estado: aceita (programa RHP-100; implementada na Fase 2)
- Contexto: completar a história de funções (P1 closures + Fase 2) com a
  flexibilidade de chamada do dia a dia PHP (P3 do plano aprovado).

## 1. Problema

Toda função RHP exige aridade exata. Falta o par PHP `default` + `...rest`,
base de APIs tolerantes e callbacks com opções.

## 2. Objetivo e não-objetivos

Objetivo: `fn f(a, b = expr-const, ...rest)` — defaults constantes
avaliados em compilação (paridade PHP: só expressão constante), `...rest`
por último coletando excedentes em `Array` (vazio se nenhum).

Não-objetivos: defaults arbitrários avaliados por chamada (rejeitado:
exigiria thunks/reentrância; PHP também restringe a constantes);
`...` em chamada (spread, fase futura); named arguments; `...rest` no meio;
obrigatório após opcional.

## 3. Decisão de desenho

- Sintaxe: `param := Ident ("=" Expr)?`; `...Ident` só por último e uma vez.
  Obrigatório após opcional = erro de parse; duplicado (incl. `...rest` com
  nome repetido) = erro (regra vigente, estendida).
- `Param { name, default: Option<Expr> }` (parser); `Stmt::FnDef` e
  `ClosureData` ganham `variadic: Option<String>`.
- `FuncDef` carrega `defaults: Vec<Option<Value>>` (alinhado a `params`) +
  `variadic: Option<String>`; `eval_const_expr` no compilador dobra
  literais, `-`/`!`/`~`, aritmética/bitwise/comparação de números/strings,
  `+` de strings e compostos de constantes. `Var`/`Call`/closure/etc. =
  erro de compilação ("padrão deve ser expressão constante"); divisão por
  zero em constante = erro de compilação (determinístico).
- Chamada (runtime, `push_call_frame`): `n < obrigatórios` ou
  (`n > total` sem variádico) = erro de aridade (mensagens vigentes);
  posições ausentes preenchidas com o clone do `Value`; excedentes para
  `...rest` (`[]` se nenhum); `null` explícito é valor (não aciona default).
- `...rest` conta como declarado (sombreia captura com o mesmo nome).
- HOFs passam pelo mesmo caminho (callback com default funciona de graça).

## 4. Alternativas consideradas

- Defaults arbitrários por chamada: thunks + mini-protocolo de retomada;
  rejeitado (custo vs benefício; PHP restringe a constantes).
- Sintaxe `*rest`/`rest...`: rejeitada (PHP usa `...`, sem conflito léxico
  — RHP não tem `.` nem concat com ponto).
- `eval` parcial em runtime para `Var` global: rejeitado (fura "constante";
  complica semântica).

## 5. Compatibilidade

Aditiva. Funções existentes (sem `=`/`...`) compilam e chamam idêntico;
aridade exata continua valendo para elas. `...` (três pontos) antes era erro
léxico ("caractere inesperado") — nenhum programa válido quebra.

## 6. Segurança

Sem `unsafe`; dobra em compilação (sem I/O, sem loops — recursão limitada
pela profundidade de parse); sem `Send` novo.

## 7. Performance

Zero custo de runtime além do clone do `Value` por default aplicado e do
`Array` do `...rest` (mesma classe das cópias por valor).

## 8. Plano de implementação

1. Lexer: `Ellipsis` (`...`; `.` isolado = erro claro) + teste.
2. Parser: `Param`, `parse_params_list` → `(Vec<Param>, Option<String>)`,
   regras de posição/duplicata, `FnDef`/`ClosureData`, docs, testes.
3. Bytecode: `FuncDef.defaults/variadic`, `eval_const_expr`, aridade em
   `compile_func_body`, testes (ids? não há — só compilação).
4. VM: preenchimento em `push_call_frame` + testes (matriz + erros).
5. Fmt (defaults + `...`) + CLI (renamer: defaults renomeados, `rest`
   declarado; import-checks: discriminant? não há — defaults são exprs:
   cobrir via `check_expr_imports` onde params aparecem? `FnDef`/`Closure`
   no renamer + checkers) + specs + testes + `changes/` + commit.

## 9. Plano de testes

- Parse: defaults literais/expressão, `...rest`, erros (rest no meio,
  obrigatório após opcional, duplicata, `.`/`..` inválidos).
- Const: dobra aritmética, rejeita `Var`/`Call`, divisão por zero.
- VM: default aplicado, `null` explícito não aplica, `...` vazio/cheio,
  aridade mínima/máxima, closure com default via HOF, `...rest`
  sombreando captura.
- Fmt idempotente + reparse; CLI rename; suite sem regressão.

## 10. Migração e rollout

Nenhuma (aditivo; `...` antes era erro léxico).

## 11. Critérios de aceitação

- [ ] Testes da §9 passam; suite sem regressão.
- [ ] `fmt`/`clippy`/`test` + `check-changes.sh` verdes.
- [ ] Specs + divergências em `PHP_DIFFERENCES.md`.
- [ ] Esta RFC marcada `aceita` antes do merge.
