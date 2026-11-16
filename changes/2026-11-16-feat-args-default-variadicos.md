# [Added] — Parâmetros default e variádicos (RFC 0006, Fase 2)

- ID: 2026-11-16-feat-args-default-variadicos
- Data: 2026-11-16
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (`...` antes era erro léxico)

## Resumo
`fn f(a, b = expr-const, ...rest)`: defaults constantes dobrados em
compilação para `Value`, `...rest` por último coletando excedentes em
`Array`. Aridade flexível com mensagens de mínimo/máximo.

## Motivação
Completar a história de funções (P1 closures + Fase 2): P3 do plano
aprovado (args default/variádicos), paridade com o PHP do dia a dia.
Branch: `feat/php-100-fase2`.

## O que mudou
- Lexer: `TokenKind::Ellipsis` (`...`; `.`/`..` = erro claro) + teste.
- Parser: `Param { name, default }`, `parse_params_list` → params +
  `variadic`; regras (rest por último e único, obrigatório após opcional
  = erro); `FnDef`/`ClosureData` com `variadic`; `describe`; docs; testes.
- Bytecode: `FuncDef.{defaults, variadic}` (alinhado a `params`),
  `eval_const_expr` (literais, unários, aritmética/bitwise/comparação de
  números/strings, `+` de strings, compostos; resto = erro "constante";
  divisão por zero/estouro = erro de compilação), `compile_func_body`
  com duplicatas (incl. `...rest`) + dobra, testes.
- VM: `push_call_frame` com mínimo/máximo/fill (clone do `Value`),
  `...rest` (`[]` se nenhum), `null` explícito não aciona default; matriz
  de testes + erros.
- Fmt: `fmt_params` (`a, b = expr, ...rest`) + golden/idempotência.
- CLI: renamer (defaults no escopo externo, params + `rest` declarados),
  `import` em default via `check_expr_imports` + testes (e2e, rename,
  import-em-default).
- Specs: `LANGUAGE_SPEC.md` (gramática + aridade), `BYTECODE_SPEC.md`
  (`FuncDef`), `PHP_DIFFERENCES.md` (constantes, sem spread/named),
  `ROADMAP.md`, exemplo `examples/args.rhp` (CI).

## Detalhes técnicos
- Defaults arbitrários por chamada rejeitados (thunks/reentrância; PHP
  também restringe a constantes) — decisão da RFC 0006.
- HOFs passam pelo mesmo caminho (callback com default funciona de graça;
  coberto em teste de exemplo).
- Correção incidental: `push_call_frame` perdeu param `idx` morto (clippy).
- Arquivos: `crates/rhp-lexer/src/lib.rs`, `crates/rhp-parser/src/lib.rs`,
  `crates/rhp-bytecode/src/lib.rs`, `crates/rhp-vm/src/lib.rs`,
  `crates/rhp-fmt/src/lib.rs`, `crates/rhp-cli/src/main.rs`,
  `RFCs/0006-args-default-variadicos.md` (+ specs, exemplo).

## Compatibilidade e migração
Aditivo. Funções sem `=`/`...` compilam e chamam idêntico (aridade exata
e mensagens preservadas — suíte confirma). `...` antes era erro léxico.
Nenhuma migração.

## Segurança e performance
- Sem `unsafe`; dobra sem I/O/loops (limitada pela profundidade de parse).
- Custo de runtime: um clone por default aplicado + `Array` do `...rest`.

## Testes e validação
- `cargo fmt --check`: limpo. `cargo clippy --workspace --all-targets`:
  0 warnings/errors.
- `cargo test --workspace`: 244 passed, 0 failed (236 + 8 novos).
- Novos: lexer (`...`, erro `.`); parser (forma, erros de posição/duplicata);
  bytecode (dobra, não-const, div-zero, duplicatas); VM (matriz + erros +
  segundo `...`); fmt (golden, idempotência); CLI (e2e, import-em-default).
- `examples/args.rhp` com exit 0 (CI).

## Limitações e riscos conhecidos
- Sem spread em chamada, sem named arguments, sem default arbitrário.
- `...rest` só por último e um só (erro claro nos demais casos).

## Referências
- `RFCs/0006-args-default-variadicos.md` (aceita)
- Branch `feat/php-100-fase2`
