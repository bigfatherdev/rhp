# [Fixed] — AUD-101/102: formatador com roundtrip garantido

- ID: 2026-11-14-fix-fmt-roundtrip
- Data: 2026-11-14
- Versão: unreleased
- Tipo: Fixed
- Compatibilidade: Compatível (saída passa a reparsear; formato de programas
  válidos já canônicos inalterado)

## Resumo

`rhp fmt` não gera mais código inválido: floats sem notação científica
(`0.000001`, nunca `1e-6`) e chaves de mapa com aspas quando necessário
(`{"a b": 1}`).

## Motivação

AUD-101/AUD-102 (P1): o formatador quebrava o próprio contrato —
`echo 0.000001;` virava `echo 1e-6;` (erro de parse) e `{"a b": 1}` virava
`{a b: 1}` (erro de parse).

## O que mudou

- `rhp-fmt`: `Float` usa `Display` (nunca expoente) + sufixo `.0` quando sem
  ponto; `fmt_map_key()` emite nua só se identificador válido, senão `{:?}`.

## Arquivos ou áreas afetadas

- `crates/rhp-fmt/src/lib.rs` (+2 testes)

## Compatibilidade e migração

Compatível. Migração: não necessária.

## Segurança e performance

Sem impacto (formatação total já era O(n)).

## Testes e validação

- Falha-antes: 2 testes novos falharam com o diff exato (`1e-6`, `{a b: 1}`);
  após a correção passam, incluindo `check_idempotent` (reparse preserva AST).
- `cargo test --workspace`: 103 testes, 0 falhas.
- `cargo fmt --check` ok; `cargo clippy ... -D warnings` ok.

## Limitações e riscos conhecidos

Chaves com caracteres de controle dependem de AUD-103 (agora suportadas via
`\u{...}`).

## Referências

- Auditoria: `reports/AUDIT_MASTER.md` (AUD-101, AUD-102).
