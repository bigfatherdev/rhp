# [Fixed] — AUD-001: teto de profundidade sintatica e de constantes

- ID: 2026-11-14-fix-limite-profundidade
- Data: 2026-11-14
- Versão: unreleased
- Tipo: Fixed (Security)
- Compatibilidade: Compatível (só falha o que antes abortava o processo)

## Resumo

Entradas com aninhamento profundo não derrubam mais o processo: parser recusa
acima de 128 níveis com erro limpo; `validate()` virou iterativo e rejeita
constantes artesanais além de 128 níveis.

## Motivação

AUD-001 (P0): `echo (((...)))` com 50k parênteses causava `stack overflow` +
SIGABRT (`exit=134`), inclusive matando o harness de testes.

## O que mudou

- `rhp-parser`: `Cursor::depth` + `nest()` em `parse_expr`/`parse_stmt`;
  `MAX_PARSE_DEPTH = 128` (cadeias planas não acumulam).
- `rhp-bytecode`: `validate()` iterativo (pilha explícita de chunks);
  `MAX_VALUE_DEPTH = 128` com checagem iterativa de constantes.
- `LANGUAGE_SPEC.md`: documenta o limite de 128 níveis.

## Arquivos ou áreas afetadas

- `crates/rhp-parser/src/lib.rs`, `crates/rhp-bytecode/src/lib.rs`,
  `LANGUAGE_SPEC.md`

## Compatibilidade e migração

Compatível. Programas reais (aninhamento de 1 dígito) inalterados; só
entradas patológicas mudam de "abort" para "erro". Migração: não necessária.

## Segurança e performance

Elimina DoS por stack overflow via fonte; validação resiste a chunks
artesanais. Sem impacto mensurável em programas normais (1 contador por nó).

## Testes e validação

- Falha-antes: teste novo abortou o harness sem a correção (SIGABRT);
  após: erro limpo.
- `cargo test --workspace`: 103 testes, 0 falhas (3 testes novos de parser
  + 2 de bytecode).
- `rhp run deep.rhp` (50k parênteses): antes `exit=134`, agora erro
  `profundidade maxima excedida em expressao (limite 128)`, `exit=1`.
- `cargo fmt --check` ok; `cargo clippy ... -D warnings` ok.

## Limitações e riscos conhecidos

Limite de 128 é conservador (calibrado p/ pilha de 2 MiB); embedding com
pilha menor pode precisar de limite menor (futuro: configurável).

## Referências

- Auditoria: `reports/AUDIT_MASTER.md` (AUD-001). ADR: `ADRs/0005-limites-robustez.md`.
