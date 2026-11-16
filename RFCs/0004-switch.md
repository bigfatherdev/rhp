# RFC 0004 — `switch` com fallthrough (comparação frouxa)

- ID: RFC-0004
- Data: 2026-11-16
- Estado: aceita (programa RHP-100; implementada na Fase 2)
- Contexto: programa "RHP 100% PHP" (fatia funcional); `switch` é assintótico
  do dia a dia PHP e faltava no núcleo imperativo (roadmap: pendente).

## 1. Problema

RHP só tem `if`/`else` para desvio múltiplo. Falta o `switch` estilo PHP:
discriminante avaliado uma vez, casos com fallthrough, `default` em qualquer
posição, `break` saindo do `switch`.

## 2. Objetivo e não-objetivos

Objetivo: `switch expr { case expr { ... } default { ... } }` com a
comparacao `==` do RHP (fatia): fallthrough, `default` unico em qualquer
posicao, `break` no `switch`, `continue` com semantica PHP (em `switch`
age como `break`; `continue N` conta niveis incluindo `switch`).

Não-objetivos: `break N` (RHP não tem `break` com argumento em lugar nenhum),
`endswitch` (RHP não tem sintaxe alternativa), `match` estrito (fase futura),
múltiplos `default` (erro), tipos restritos de discriminante (qualquer `==`).

## 3. Decisão de desenho

- Sintaxe (sem keyword nova além das 3): `switch EXPR { (case EXPR { STMT* } | default { STMT* })* }`.
  Braces nos cases (consistente com `if`/`while`/`fn`; nada de `:`).
- AST: `Stmt::Switch { discriminant: Expr, cases: Vec<SwitchCase> }` com
  `SwitchCase { cond: Option<Expr>, body: Vec<Stmt> }` (`None` = `default`;
  vetor único preserva a posição do `default`, inclusive no meio).
- Comparação: o `Eq` existente (`==` do RHP: promoção int/float, resto
  mesmo tipo). `switch(true)` com cases int NÃO casa (divergência deliberada
  do PHP, cujo `switch` é frouxo; seguir o `==` da linguagem evita uma
  segunda semântica de igualdade). Documentado em `PHP_DIFFERENCES.md`.
- Avaliação: discriminante 1x; `case`s em ordem até o primeiro match
  (preguiçoso — efeitos colaterais só até o match, como PHP).
- Fallthrough: sem `break`, o controle cai no próximo case/default.
- `break` liga ao `switch` mais interno (registrado como contexto
  "breakable"); `continue`/`continue 1` em `switch` equivalem a `break`
  dele (gotcha documentado do PHP); `continue N` conta N breakables para
  fora (laco ou `switch`; RFC 0008). `return`/`throw` seguem as regras
  vigentes (saem da ativacao / unwinding com `finally`).
- **Escopo por case** (`EnterScope`/`ExitScope`): `let` repetido em cases
  distintos não colide. Diferença deliberada vs PHP (escopo único) —
  coerente com blocos RHP; documentada em `PHP_DIFFERENCES.md`.
- Compilação: discriminante + por case `Dup; <case>; Eq; JumpIfFalse próximo`;
  `break` remendado para o fim; `Pop` final dispensa o discriminante.
  Nenhum op novo. `define`/`MakeFunc`/`Call` intocados.
- `diverges()`: corpo de case que sempre diverge dispensa fechamento de
  escopo local (mesma regra do `if`).

## 4. Alternativas consideradas

- Desugar para cadeia `if/else`: perde fallthrough e avalia `==` do mesmo
  jeito, mas o bytecode com `Dup` é mais fiel (discriminante 1x garantido
  sem temporário) e erros de `break` ficam precisos; rejeitado.
- Escopo único do `switch` (PHP puro): `let x` em dois cases colidiria
  ("já declarada") — armadilha gratuita; rejeitado em favor do escopo
  por case (documentado como divergência).
- `match` estrito junto: outra feature, outra RFC; rejeitado nesta leva.

## 5. Compatibilidade

Aditiva. `switch`/`case`/`default` viram keywords reservadas (3 novas);
programas que usavam esses identificadores como nomes quebram de forma
explícita no parse (erro claro, migração = renomear). Nenhum programa válido
sem esses identificadores muda de comportamento.

## 6. Segurança

Sem `unsafe`; sem ops novos; validador de pilha cobre os novos saltos
(alturas determinísticas: `Dup`/`Eq`/`JumpIfFalse` preservam). Profundidade
de parse inalterada (cases são blocos normais sob `MAX_PARSE_DEPTH`).

## 7. Performance

Um `Dup` + `Eq` por case até o match; custo O(cases) como qualquer cadeia.
Sem alocação nova além do discriminante na pilha (dispensado no fim).

## 8. Plano de implementação

1. Lexer: `Switch`/`Case`/`Default` + keywords + `describe` + testes.
2. Parser: `SwitchCase`, `Stmt::Switch`, `parse_switch` (default único,
   case/default fora = erro), dispatch, docs de gramática, testes.
3. Bytecode: `compile_switch`, `LoopCtx.is_loop` (break no mais interno,
   continue pula switches), testes de compilação.
4. VM: nada (ops existentes).
5. `rhp-fmt` + CLI (renamer com escopo por case; import-checks) + specs
   (`LANGUAGE_SPEC`, `BYTECODE_SPEC`, `PHP_DIFFERENCES`) + exemplo
   `examples/switch.rhp` + `changes/` + commit.

## 9. Plano de testes

- Parser: forma, fallthrough, `default` no meio/só, default duplo (erro),
  case fora de switch (erro), switch vazio, discriminante com efeito
  colateral (avaliação única conta via global — teste VM).
- VM e2e: matriz (match, fallthrough, break, sem-match sem/sem-default,
  aninhado, switch-em-laço com break/continue, laço-em-switch, `return`
  em case, `throw` com `try`/`finally` cruzando).
- Fmt idempotente + reparse; CLI rename (sombra por case) + import em case
  rejeitado; suite sem regressão.

## 10. Migração e rollout

Só para quem usava `switch`/`case`/`default` como identificadores:
renomear (erro de parse claro aponta o local). Demais programas: nenhuma.

## 11. Critérios de aceitação

- [ ] Testes da §9 passam; suite sem regressão.
- [ ] `fmt`/`clippy`/`test` + `check-changes.sh` verdes.
- [ ] Specs + exemplo executável; divergências em `PHP_DIFFERENCES.md`.
- [ ] Esta RFC marcada `aceita` antes do merge.
