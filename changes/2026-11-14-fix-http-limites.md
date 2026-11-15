# [Fixed] — AUD-104/105: teto de corpo no `http_get` + DNS documentado

- ID: 2026-11-14-fix-http-limites
- Data: 2026-11-14
- Versão: unreleased
- Tipo: Fixed (Security)
- Compatibilidade: Compatível (só falha o que antes estourava memória)

## Resumo

`http_get` limita o corpo a 1 MiB (`MAX_HTTP_BODY`, leitura em blocos de
8 KiB com interrupção antecipada) e documenta que o DNS usa o resolvedor do
sistema sem timeout próprio.

## Motivação

AUD-104 (P1): `read_to_end` integral permitia OOM com resposta gigante.
AUD-105 (P1): `getaddrinfo` fora dos 5s prometidos.

## O que mudou

- `rhp-vm`: `MAX_HTTP_BODY`, leitura em loop com teto; doc do DNS no código.
- `LANGUAGE_SPEC.md` (`http_get`: teto 1 MiB, DNS sem timeout próprio).

## Arquivos ou áreas afetadas

- `crates/rhp-vm/src/lib.rs` (+1 constante, +teste unitário de host inválido),
  `crates/rhp-cli/tests/integration.rs` (+teste com servidor de 2 MiB),
  `LANGUAGE_SPEC.md`

## Compatibilidade e migração

Compatível. Corpos ≤ 1 MiB inalterados. Migração: não necessária.

## Segurança e performance

Teto evita OOM; leitura interrompe no primeiro bloco além do limite.
Overhead: nenhum para corpos pequenos.

## Testes e validação

- Falha-antes: teste com servidor local de 2 MiB passou (leu tudo) sem a
  correção; após: erro `excedeu o limite de corpo (1048576 bytes)`.
- Teste novo de DNS `.invalid` falha rápido e limpo.
- `cargo test --workspace`: 103 testes, 0 falhas.
- `cargo fmt --check` ok; `cargo clippy ... -D warnings` ok.

## Limitações e riscos conhecidos

Sem limite de *taxa* (slow-loris consome os 5s de timeout e falha — aceitável);
sem HTTPS/redirect (fora do escopo, ver ADR 0004).

## Referências

- Auditoria: `reports/AUDIT_MASTER.md` (AUD-104, AUD-105). ADR: `ADRs/0005-limites-robustez.md`.
