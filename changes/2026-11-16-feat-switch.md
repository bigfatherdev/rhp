# [Added] — `switch` com fallthrough (RFC 0004, Fase 2)

- ID: 2026-11-16-feat-switch
- Data: 2026-11-16
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (ressalva: `switch`/`case`/`default` viram keywords)

## Resumo
`switch expr { case expr { ... } default { ... } }` com a comparação `==`
do RHP, fallthrough, `default` único em qualquer posição, `break` no
`switch` e `continue` mirando o laço externo. Escopo próprio por ramo.

## Motivação
Primeira fase da leva aprovada (Fase 2 do programa RHP-100): completar o
núcleo imperativo antes dos HOFs com callback, args e destructuring.
Branch: `feat/php-100-fase2`.

## O que mudou
- Lexer: `TokenKind::{Switch, Case, Default}` + keywords + teste.
- Parser: `SwitchCase { cond: Option<Expr>, body }`, `Stmt::Switch`,
  `parse_switch` (default duplo, `case`/`default` fora = erro), gramática.
- Bytecode: `compile_switch` (discriminante 1x; testes `Dup+Eq+JumpIfFalse`;
  corpos em sequência com escopo; `break` com `Pop` próprio; `continue`
  com `Pop` por `switch` cruzado), `LoopCtx.is_loop` (`break` = topo,
  `continue` pula switches; `emit_abrupt_exits` com `>=` + profundidade do
  alvo — idêntico ao `==` anterior quando sem `switch` no caminho).
- VM: nenhum op novo (reuso de `Jump`/`Eq`/`Dup`/`Pop`).
- Fmt: `switch/case/default` + testes (golden, idempotência, reparse).
- CLI: renamer com escopo por case; `import` em case/disc rejeitado.
- Specs: `LANGUAGE_SPEC.md` (sintaxe/semântica/gramática), `BYTECODE_SPEC.md`
  (esquema), `PHP_DIFFERENCES.md` (`==` do RHP em vez de frouxo PHP, escopo
  por ramo, sem `break N`/`endswitch`), `ROADMAP.md`, exemplo
  `examples/switch.rhp` (roda em CI).

## Detalhes técnicos
- Dois bugs reais achados por smoke test e corrigidos: (1) fallthrough
  reavaliava o próximo `case` (layout testes→corpos resolve); (2) caminho
  sem-match caía no primeiro corpo (`JumpIfFalse` final remendado para o
  `Pop` pós-corpos); (3) `break`/`continue` cruzando o `switch` com o
  discriminante na pilha (validador acusou altura inconsistente; `Pop`
  por fronteira cruzada); (4) `break` + código morto no case truncava o
  rastreamento de escopos (salva/restaura por case, como os laços).
- Arquivos: `crates/rhp-lexer/src/lib.rs`, `crates/rhp-parser/src/lib.rs`,
  `crates/rhp-bytecode/src/lib.rs`, `crates/rhp-fmt/src/lib.rs`,
  `crates/rhp-cli/src/main.rs`, `RFCs/0004-switch.md` (+ specs, exemplo).

## Compatibilidade e migração
Aditivo, com uma ressalva: `switch`, `case` e `default` passam a ser
palavras reservadas. Código que usava esses identificadores falha no parse
com erro claro (migração = renomear). Demais programas: nenhuma mudança.

## Segurança e performance
- Sem `unsafe`; sem ops novos; validador cobre os novos saltos.
- Custo O(cases) até o match; sem alocação além do discriminante na pilha.

## Testes e validação
- `cargo fmt --check`: limpo. `cargo clippy --workspace --all-targets`:
  0 warnings/errors.
- `cargo test --workspace`: 232 passed, 0 failed (224 + 8 novos).
- Novos: lexer keywords; parser forma + `default` no meio + duplo/case
  solto; bytecode (`Dup`/`Eq`/`Pop`, break/continue soltos, continue só
  em switch); VM matriz e2e (match, fallthrough, default meio/só,
  sem-match, vazio, promoção int/float, `Bool` vs `Int`, avaliação única,
  escopo por case, aninhado, switch-em-laço, laço-em-case, return,
  + `try`/`finally` cruzando); fmt golden + idempotência; CLI e2e,
  import-em-case, rename com escopo por case.
- `examples/switch.rhp` executa com exit 0 (CI `rhp_run_all_examples_succeed`).

## Limitações e riscos conhecidos
- `==` do RHP não é o `switch` frouxo do PHP (`switch(true)` com cases int
  não casa) — divergência deliberada documentada.
- Sem `break N`, sem `endswitch`, sem `match` estrito (fases futuras).

## Referências
- `RFCs/0004-switch.md` (aceita)
- Branch `feat/php-100-fase2`
