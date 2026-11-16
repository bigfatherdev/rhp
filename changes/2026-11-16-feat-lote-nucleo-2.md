# [Added] — Lote núcleo-2: `match`, Elvis, arrow, `for(;;)`, `break N` (RFC 0008, Fase 3)

- ID: 2026-11-16-feat-lote-nucleo-2
- Data: 2026-11-16
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (ressalvas: `match` vira keyword; `=>` antes era erro)

## Resumo
`match` como expressão (igualdade estrita, sem fallthrough), Elvis `?:`
(açúcar para `??`), arrow `fn (x) => expr`, `for init; cond; passo` e
`break N;`/`continue N;` (N literal >= 1, `switch` conta como nível e
`continue` nele age como `break` — gotcha PHP).

## Motivação
Fechar o núcleo imperativo (P do plano; RHP-100 Fase 3). Branch:
`feat/php-100-fase3-nucleo2`.

## O que mudou
- Lexer: `Match`, `FatArrow` (`=>`; `=`+`>`), testes.
- Parser: `Expr::Match` + `MatchArm`, `Stmt::ForC`, `BreakN`/`ContinueN`
  (variantes dedicadas — zero churn nos testes existentes), `parse_match`,
  Elvis (desugar em `parse_ternary`), arrow (corpo `[return]`), `parse_for_c`
  com lookahead de 2 tokens, `parse_break_level`, `peek_at`, gramática, testes.
- Bytecode: `Op::StrictEq` (+ validador), `compile_match` (testes→corpos,
  sem-match = `Pop`+`Throw` capturável), `compile_for_c` (espelha `for`,
  `continue` ao passo), `compile_break(n)`/`compile_continue(n)` +
  `compile_break_at` (N-ésimo contexto; `continue` em `switch` = `break`
  dele), `diverges`, testes.
- VM: `StrictEq` + `strict_equal` (mesmo tipo; `Int`/`Float` cruzam).
- Fmt: `match` multilinha, Elvis→`??`, arrow→bloco, for-C, `break N` +
  goldens/idempotência.
- CLI: renamer (ForC com escopo, Match, BreakN/ContinueN), checkers
  (ForC/Match/defaults), testes (e2e, rename, import-em-braço).
- Specs: `LANGUAGE_SPEC.md` (gramática + semântica), `BYTECODE_SPEC.md`
  (`StrictEq`, esquemas), `PHP_DIFFERENCES.md` (match estrito, Elvis sem
  truthiness, arrow, for-C, `break N`), `ROADMAP.md`, exemplo
  `examples/nucleo2.rhp` (CI).

## Detalhes técnicos
- Dois diagnósticos reais guiaram o desenho: (1) `for x [...]` (typo de
  for-in) mantém o erro útil "esperado `in`" (dica de for-C no texto);
  (2) `continue` em `switch` segue o PHP (equivale a `break`; teste antigo
  que codificava o comportamento não-PHP foi corrigido e documentado).
- Frames do caminho quente preservados (`parse_fn_body`/`parse_elvis_rest`
  com `#[inline(never)]`; `Expr`/`Stmt` seguem 48/120 bytes).
- Arquivos: lexer/parser/bytecode/vm/fmt/cli + `RFCs/0008-lote-nucleo-2.md`.

## Compatibilidade e migração
Aditivo, com ressalvas: `match` vira keyword (renomear identificadores);
`=>` antes era erro léxico/sintático. Demais programas: nenhuma mudança.

## Segurança e performance
- Sem `unsafe`; `N` limitado pelo aninhamento (sem salto selvagem);
  validador cobre testes/corpos/`Pop` por switch cruzado.
- `match`: um `Dup`+`StrictEq` por braço até o match (como `switch`).

## Testes e validação
- `cargo fmt --check`: limpo. `cargo clippy --workspace --all-targets`:
  0 warnings/errors.
- `cargo test --workspace`: 263 passed, 0 failed (251 + 12 novos).
- Novos: parser (match/Elvis/arrow/for-C/níveis + erros); VM (matriz match,
  Elvis 1x, arrow+captura, for-C, `break 2`/`continue 2`, além-aninhamento);
  bytecode (StrictEq/Throw, for-C, níveis, `continue`-em-switch válido);
  fmt (goldens, idempotência); CLI (e2e, import-em-braço, rename for-C).
- Comportamento `continue`-em-`switch` alinhado ao PHP e documentado
  (RFC 0004 + specs atualizadas).

## Limitações e riscos conhecidos
- Sem `match` com guardas/padrões profundos; sem spread/named; sem `goto`.
- Fmt achata closures aninhadas/match aninhado (estável, documentado).

## Referências
- `RFCs/0008-lote-nucleo-2.md` (aceita; atualiza comportamento de `continue`
  da RFC 0004)
- Branch `feat/php-100-fase3-nucleo2`
