# [Fixed] — P2 do núcleo: params, `int()`, `+` unário + pins de spec

- ID: 2026-11-14-fix-p2-nucleo
- Data: 2026-11-14
- Versão: unreleased
- Tipo: Fixed
- Compatibilidade: Compatível (só falha o que era bug silencioso ou indefinição)

## Resumo

Parâmetros duplicados viram erro de compilação; `int()` rejeita faixa
acima de `i64::MAX`; `+x` é identidade; mapa duplicado, NaN, shadowing e
precedência de builtins passam a especificados + pinados por testes.

## Motivação

Auditoria AUD-202/203/204/201/208/209/303: silenciosos que mascaram bugs
(`f(a,a)` retornando o 2º), saturação em vez de erro, `+5` rejeitado.

## O que mudou

- `rhp-bytecode`: erro `parametro duplicado: a` em `FnDef`.
- `rhp-vm`: `int()` usa `>= 2^63` (antes `> (i64::MAX as f64)`, que
  arredonda para cima e deixava `2^63` saturar).
- `rhp-parser`: `+x` desugarado para `x` (sem nó novo na AST).
- `LANGUAGE_SPEC.md`: chaves duplicadas (vence a primeira), NaN/inf,
  precedência de builtins em chamada, `let` não sombreia parâmetro.
- Testes pino para cada comportamento.

## Arquivos ou áreas afetadas

- `crates/rhp-{parser,bytecode,vm}/src/lib.rs`, `LANGUAGE_SPEC.md`

## Compatibilidade e migração

Compatível. Programas afetados: só os que dependiam dos silenciosos
(prováveis bugs). Migração: renomear parâmetro; ajustar `int()` de borda.

## Segurança e performance

Sem impacto além do pretendido. Conversões estritas reduzem surpresa.

## Testes e validação

- Falha-antes: `rejects_duplicate_params` falhou (compilou); `+5` era erro
  de parse; `int(2^63)` saturava.
- `cargo test --workspace`: 120 testes, 0 falhas.
- `cargo fmt --check` ok; `cargo clippy ... -D warnings` ok.

## Limitações e riscos conhecidos

Nenhum novo. `==` com `NaN` segue IEEE (`false`).

## Referências

- Auditoria: `reports/AUDIT_MASTER.md` (AUD-201–204, 208, 209, 303).
