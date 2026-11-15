# [Added] — Stdlib: strings, matemática e mapas (+19 builtins)

- ID: 2026-11-15-feat-stdlib-lote1
- Data: 2026-11-15
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (só adiciona builtins; nomes novos seguem a
  regra de precedência documentada)

## Resumo

20 builtins novos (ids 10–29): strings (`contains starts_with ends_with
split join trim upper lower replace`), matemática (`abs min max sqrt pow
floor ceil`) e mapas (`values entries merge has`).

## Motivação

Missão Next Generation §11 (biblioteca padrão): núcleo usável sem mudar
sintaxe ou VM além de `CallBuiltin` (genérico, validado por aridade).

## O que mudou

- `rhp-bytecode`: variantes, `builtin_by_name`, aridades, ids 10–29.
- `rhp-vm/src/builtins.rs`: implementações com erros tipados em português.
- `LANGUAGE_SPEC.md`: tabela estendida (inclui regras: `join` exige array
  de `Str`; `merge` com `b` vencendo; `split` vazio divide em caracteres).
- Testes: 3 suítes novas (strings/matemática/mapas) + teste anti-deriva de
  ids (planejado; ver abaixo).

## Compatibilidade e migração

Compatível. `merge` documenta "b vence" (distinto da regra first-wins de
literais duplicados, documentada na mesma seção).

## Segurança e performance

Sem `unsafe`; sem I/O; `sqrt`/`pow` IEEE (sem `unwrap`).

## Testes e validação

- Falha-antes: 3 suítes falharam como "variavel nao definida" antes da
  implementação.
- `cargo test --workspace`: suite verde (mapeamento de ids coberto por
  `rejects_unknown_builtin`; roundtrip id<->builtin a adicionar no CP final
  desta frente).
- `cargo fmt --check` ok; `cargo clippy --workspace --all-targets -- -D warnings` ok.

## Limitações e riscos conhecidos

IDs até 29 usados; próximos builtins continuam a sequência. `merge` não
é profundo (só nível 1).

## Referências

Missão Next Generation §11. ADR: não necessária (extensão dentro do desenho
existente dos builtins).
