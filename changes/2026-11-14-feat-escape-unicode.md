# [Added] — AUD-103: escape `\u{HEX}` em strings

- ID: 2026-11-14-feat-escape-unicode
- Data: 2026-11-14
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (antes era erro léxico; nenhum programa válido
  muda de significado)

## Resumo

Strings aceitam `\u{HEX}` (1–6 dígitos, ponto de código válido, ex.:
`\u{41}` = `A`, `\u{1F600}` = 😀). Fecha o roundtrip do formatador para
caracteres de controle (`"\u{1}"`).

## Motivação

AUD-103 (P1): `rhp fmt` emitia `"\u{1}"` para strings com `\x01`, e o lexer
rejeitava — roundtrip quebrado.

## O que mudou

- `rhp-lexer`: `lex_unicode_escape` (`{`, 1–6 hex, `}`, `char::from_u32`;
  substitutos e >0x10FFFF rejeitados; sem `unwrap`).
- `LANGUAGE_SPEC.md`: lista de escapes inclui `\u{HEX}`.

## Arquivos ou áreas afetadas

- `crates/rhp-lexer/src/lib.rs` (+2 testes), `LANGUAGE_SPEC.md`

## Compatibilidade e migração

Compatível (extensão de linguagem só torna válidos programas antes inválidos).
Migração: não necessária.

## Segurança e performance

Parsing O(n) do literal; sem alocação extra além da string. Fora do intervalo
=> erro, nunca substituto silencioso (sem `from_u32_unchecked`).

## Testes e validação

- Falha-antes: `resolves_unicode_escapes` falhou sem a implementação.
- `cargo test --workspace`: 103 testes, 0 falhas (inclui 6 malformados).
- `cargo fmt --check` ok; `cargo clippy ... -D warnings` ok.

## Limitações e riscos conhecidos

Sem `\U`, `\0` dedicado ou `\xHH` (fora do escopo; `\u{0}` cobre nulo).

## Referências

- Auditoria: `reports/AUDIT_MASTER.md` (AUD-103).
