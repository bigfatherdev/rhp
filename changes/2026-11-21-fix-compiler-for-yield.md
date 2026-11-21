# Fixed — Compilador: `for-in` linear + `yield` em `try` + diagnósticos

- ID: 2026-11-21-fix-compiler-for-yield
- Data: 2026-11-21
- Versão: 0.7.2
- Tipo: Fixed
- Compatibilidade: Compatível

## Resumo
`for x in` aninhados travavam o CLI (custo 2^N; 20 níveis = hang);
`yield` dentro de `try` quebrava o gerador no 2o `next`; EOF após
palavra-chave apontava coluna errada; float gigante virava `inf` mudo.

## Motivação
Auditoria 0.7.2: COMP-001 (P0), VM-002 (P0), COMP-006 (P2), COMP-007 (P3).

## O que mudou
- `compile_for` compila o corpo UMA vez (antes: lazy + eager); `continue`
  despacha em runtime por marca da forma. Complexidade linear.
- `Stmt::Yield` não emite mais `emit_abrupt_exits`: suspende com
  handlers/escopos intactos; `finally` roda uma vez, no término.
- `token_width`: palavras-chave com largura real (EOF `1:5` após `echo`).
- Literal float não-finito = `E0004` (nunca `inf` silencioso).

## Detalhes técnicos
`crates/rhp-bytecode/src/lib.rs` (`compile_for`, `Stmt::Yield`,
`token_width`); `crates/rhp-lexer/src/lib.rs` (checagem `is_finite`).
Semântica preservada: mesmos pontos de `continue`/`break`, mesmos
escopos por iteração; só o loop-back mudou de lugar.

## Compatibilidade e migração
Compatível. Programas válidos geram o mesmo comportamento; só o que
antes travava/abortava/enganava agora funciona ou erra limpo.
Não necessária migração.

## Segurança e performance
COMP-001 era DoS por arquivo-fonte (hang de CI). Pós-fix: `check` de
20 níveis aninhados = instantâneo (antes >20 s timeout). Sem regressão
de bench (fib/loops inalterados).

## Testes e validação
- `cargo test -p rhp-bytecode compiles_nested_for_linear_not_exponential`: ok (crescimento <3x de 4→8 níveis).
- `cargo test -p rhp-cli --test integration rhp_run_nested_for_both_forms rhp_run_yield_inside_try`: ok.
- `cargo test -p rhp-lexer rejects_non_finite_float_literal`, `-p rhp-parser eof_after_keyword`: ok.
- `cargo test --workspace`: 395 passed, 0 failed.
- `tools/fuzz-smoke.sh 1000`: 0 suspeitos.

## Limitações e riscos conhecidos
`i64::MIN` literal segue inexpresível (documentado; workaround
`0-9223372036854775807-1`). Gerador abandonado não executa `finally`.

## Referências
COMP-001, VM-002, COMP-006, COMP-007 (auditoria 0.7.2); RFC 0018.
