# [Added] — Stdlib lote 4: strings, matemática e mapas (+19 builtins)

- ID: 2026-11-15-feat-stdlib-lote2
- Data: 2026-11-15
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (só adiciona builtins; ids 10–28)

## Resumo

19 builtins: `contains starts_with ends_with split join trim upper lower
replace` (strings), `abs min max sqrt pow floor ceil` (matemática),
`values entries merge has` (mapas). Aridades checadas em compilação;
ids estáveis 10–28 com teste anti-deriva estendido.

## Motivação

Missão Next Generation §18 (lote imediato): stdlib usável sem mudar
sintaxe nem VM além do genérico.

## O que mudou

- `rhp-bytecode`: variantes, `builtin_by_name`, aridades, ids, teste
  anti-deriva (lista completa).
- `rhp-vm/src/builtins.rs`: implementações tipadas (Unicode onde aplica;
  `int()`/`float()` estritos herdados; `join` exige array de `Str`).
- `LANGUAGE_SPEC.md`: tabela estendida (inclui regras: separador vazio
  divide em caracteres; `merge` com `b` vencendo; `values`/`entries`
  ordenados por chave).

## Arquivos ou áreas afetadas

- `crates/rhp-bytecode/src/lib.rs`, `crates/rhp-vm/src/builtins.rs`,
  `LANGUAGE_SPEC.md` (+3 testes de VM).

## Compatibilidade e migração

Compatível. Migração: não necessária.

## Segurança e performance

Sem `unsafe`; sem I/O; sem alocação além do resultado. `split`/`join`
O(n) no tamanho.

## Testes e validação

- Falha-antes: suíte nova falhou como "variavel nao definida".
- `cargo test --workspace`: suite verde, 0 falhas.
- `cargo fmt --check` ok; `cargo clippy --workspace --all-targets -- -D warnings` ok.

## Limitações e riscos conhecidos

Sem regex (requer decisão: motor próprio vs crate externa).
`sort`/`filter`/`map` com callback exigem funções de primeira classe
(planejado, não implementado).

## Referências

Missão Next Generation §11 (lote imediato). Complementa
`changes/2026-11-15-feat-stdlib-lote1.md` (ids 0–9).
