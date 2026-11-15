# [Added] — Sintaxe lote 2: bitwise, ternário, `??`, `break`/`continue`, `const`

- ID: 2026-11-15-feat-sintaxe-lote2
- Data: 2026-11-15
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (sintaxe e ops novos; todo programa anterior
  compila e executa igual — suíte de regressão intacta)

## Resumo

Operadores bitwise (`& | ^ ~ << >>`), ternário `a ? b : c`, coalescência
nula `a ?? b` (avaliação única via `Dup`+`JumpIfNotNull`), `break`/`continue`
com desenrolamento correto de escopos, e `const` imutável verificado em
compilação.

## Motivação

Missão Next Generation §18 (lote imediato): controle de fluxo e expressões
que faltavam, sem arquitetura nova (só lexer/parser/bytecode/VM/fmt).

## O que mudou

- Lexer: tokens `& | ^ ~ << >> ? ??`, keywords `break continue const`.
- Parser: precedência completa (ternário < `??` < `==` < `|` < `^` < `&` <
  comparação < `<<` `>>` < `+` `-` < `*` < unaria); `Expr::Ternary`;
  `Stmt::{Const,Break,Continue}`.
- Bytecode: ops `BitNot BitAnd BitOr BitXor Shl Shr Dup JumpIfNotNull`;
  análise de fluxo estendida; compilador rastreia escopos e laços
  (`break`/`continue` desenrolam; `const` rejeita atribuição; ramo que
  sempre diverge dispensa fechamento de escopo — bug real encontrado e
  corrigido nesse ponto).
- VM: implementação dos 8 novos ops (deslocamento com `checked_*`).
- `rhp-fmt`: precedência, `?:`, `??` (associativo à direita), `const`,
  `break`/`continue` (idempotente, com testes).
- Specs: `LANGUAGE_SPEC` (sintaxe, precedência, semântica) e
  `BYTECODE_SPEC` (ops, compilação, ids de builtin atualizados).

## Arquivos ou áreas afetadas

- `crates/rhp-{lexer,parser,bytecode,vm,fmt}/src/lib.rs`,
  `crates/rhp-vm/src/builtins.rs` (intocado),
  `crates/rhp-cli/tests/integration.rs`,
  `LANGUAGE_SPEC.md`, `BYTECODE_SPEC.md`.

## Compatibilidade e migração

Compatível. Precedência segue PHP/C (`+` liga mais que `<<`; `&` liga mais
que `==`). `??` tem avaliação única (sem dupla avaliação de efeitos).
Migração: não necessária.

## Segurança e performance

`checked_shl/checked_shr` (deslocamento inválido = erro, sem panic em
debug). Sem `unsafe`. Sem impacto mensurável no caminho existente.

## Testes e validação

- Falha-antes: lexer/parser/VM/integração novos falharam antes das
  implementações; `break` em `if` aninhado exigiu correção real no
  compilador (desenrolamento duplo de escopo).
- `cargo test --workspace`: suite verde, 0 falhas (inclui teste fim a fim
  com `const`/bitwise/ternário/`??`/`break`/`continue`).
- `cargo fmt --check` ok; `cargo clippy --workspace --all-targets -- -D warnings` ok.

## Limitações e riscos conhecidos

`for`/`switch`/destructuring continuam ausentes (planejados).
`??` aninhado à direita formata sem parênteses (correto por associatividade).

## Referências

Missão Next Generation §18 (lote imediato). Sem ADR (extensão dentro da
arquitetura vigente).
