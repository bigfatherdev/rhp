# [Added] — Lote 2: bitwise, ternário, `??`, `break`/`continue`, `const`

- ID: 2026-11-15-feat-sintaxe-lote2
- Data: 2026-11-15
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (sintaxe e ops novos; todo programa anterior
  compila e executa igual — suíte de regressão intacta)

## Resumo

Novos operadores (`& | ^ ~ << >>`), ternário `a ? b : c`, coalescência
nula `a ?? b` (avalia uma vez via `Dup`/`JumpIfNotNull`), `break`/`continue`
com desenrolamento correto de escopos, e `const` imutável com verificação
em compilação.

## Motivação

Missão Next Generation §18 (lote imediato): controle de fluxo e expressões
que faltavam, sem arquitetura nova (só lexer/parser/bytecode/VM/fmt).

## O que mudou

- Lexer: tokens `& | ^ ~ << >> ? ??`, keywords `const break continue`.
- Parser: níveis de precedência (ternário < `??` < `==` < `|` < `^` < `&` <
  comparação < `<<` `>>` < `+ -` < `*` < unária), `Expr::Ternary`,
  `Stmt::{Const,Break,Continue}`, `const` exige `=`, `break`/`continue`
  fora de laço = erro.
- Bytecode: ops `BitNot BitAnd BitOr BitXor Shl Shr Dup JumpIfNotNull`;
  análise de fluxo estendida; compilador rastreia escopos (`const` e
  desenrolamento de `break`/`continue`); `diverges()` evita `ExitScope`
  em ramo inalcançável (bug real encontrado e corrigido no caminho).
- VM: implementação dos 8 ops (`checked_shl/shr`; `~`/bitwise só `Int`).
- `rhp-fmt`: precedência nova + nós novos + `const`/`break`/`continue`.
- Specs: `LANGUAGE_SPEC` (gramática, precedência, semântica) e
  `BYTECODE_SPEC` (ops, compilação, ids de builtin atualizados).

## Arquivos ou áreas afetadas

- `crates/rhp-{lexer,parser,bytecode,vm,fmt}/src/lib.rs`,
  `crates/rhp-vm/src/builtins.rs` (nada), `crates/rhp-cli/tests/integration.rs`,
  `LANGUAGE_SPEC.md`, `BYTECODE_SPEC.md`.

## Compatibilidade e migração

Compatível. Precedência escolhida = PHP/C (`+` liga mais que `<<`;
`&` liga mais que `==`). `??` é de curto-circuito com avaliação única.
Migração: não necessária.

## Segurança e performance

Sem `unsafe`; shift com `checked_*` (deslocamento inválido = erro, sem
panic em debug). Sem impacto mensurável no caminho existente.

## Testes e validação

- Falha-antes: lexer/parser/VM/integração novos falharam antes da
  implementação; escopo de `break` em `if` aninhado exigiu correção real.
- `cargo test --workspace`: 150 testes, 0 falhas (inclui teste fim a fim
  com `const`/bitwise/ternário/`??`/`break`/`continue`).
- `cargo fmt --check` ok; `cargo clippy --workspace --all-targets -- -D warnings` ok.

## Limitações e riscos conhecidos

`for`/`switch`/destructuring continuam ausentes (planejados). `??`
aninhado à direita formata com parênteses apenas quando necessário.

## Referências

Missão Next Generation §18 (lote imediato). Sem ADR (extensão dentro da
arquitetura vigente).
