# [Fixed] — `return`/`yield` dentro de `try` capturam erro do operando

- ID: 2026-11-20-fix-return-try-operando
- Data: 2026-11-20
- Versão: 0.6.0
- Tipo: Fixed
- Compatibilidade: Compatível (mudança observável documentada abaixo)

## Resumo

`return alguma_funcao();` dentro de `try` agora tem o erro capturado pelo
`catch` ativo. Antes, apenas o workaround `let x = f(); return x;`
funcionava — o erro do operando escapava para o topo do programa.

## Motivação

Bug P0/P1 da missão 0.6.0 (§3): `return json_parse(s);` em `try` falhava
com `Error[E3000]` não capturado, enquanto `let x = json_parse(s);
return x;` era capturado. Mesma causa para nomes de função inexistentes.

## O que mudou

- `Stmt::Return` e `Stmt::Yield` no compilador: o operando é compilado
  ANTES de `emit_abrupt_exits` (que emite `TryEnd` + cópias de `finally`),
  não depois.
- Sem mudança na VM: com o handler ainda registrado no momento do erro,
  o unwinding existente entrega ao `catch` correto.

## Detalhes técnicos

Causa raiz (compilador, não VM): a ordem era `TryEnd → avalia operando →
Return`, então o erro nascia sem handler. Agora: `avalia operando →
TryEnd/finally → Return`. Efeito colateral correto: `finally` roda depois
do operando avaliado (semântica padrão PHP/JS/Java), e o valor já
empilhado sobrevive às cópias de `finally` (neutras na pilha; verificador
de pilha aprova).
Arquivos: `crates/rhp-bytecode/src/lib.rs` (Return/Yield),
`crates/rhp-vm/src/lib.rs` (teste), `LANGUAGE_SPEC.md`, `BYTECODE_SPEC.md`,
`docs/REFERENCIA.md`.

## Compatibilidade e migração

Programas válidos seguem válidos. Observável: `try { return e; }
finally { F }` com `e` dependente de estado mutado por `F` agora vê o
valor pré-`finally` (antes: pós). Nenhuma sintaxe removida; nenhum código
de erro alterado. Migração: não necessária.

## Segurança e performance

Sem novas superfícies; sem `unsafe`. Benchmark `bench_rhp` (release):
`fib(20)`/`while` inalterados dentro do ruído de CPU compartilhada.

## Testes e validação

- `cargo test -p rhp-vm --lib runs_return_inside_try_catches_operand_error`: falha antes, passa depois (8 casos da missão: builtin, nome inválido, normal, let, aninhada, try aninhado, if/while/recursão, propagação sem handler + ordem operando-antes-finally).
- `cargo test --workspace`: 354 passed, 0 failed (com Postgres de teste via Docker).
- `cargo fmt --check`, `cargo clippy --workspace --all-targets`: limpos.
- CLI manual: `return json_parse(s)` → `pegou`; `return unknown_fn()` → `capturado`.

## Limitações e riscos conhecidos

Nenhum novo. `exit()` dentro do operando continua não-capturável (por desenho).

## Referências

- Missão 0.6.0 §3; `LANGUAGE_SPEC.md` (§ try/return); commit na branch `feat/rhp-0.6.0`.
