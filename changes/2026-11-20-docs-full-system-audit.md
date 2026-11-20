# [Docs] — Auditoria full-system: docs + nota bool drivers (RHP-AUD-005)

- ID: 2026-11-20-docs-full-system-audit
- Data: 2026-11-20
 - Versão: 0.6.1
- Tipo: Changed (documentação)
- Compatibilidade: Compatível

## Resumo

Documentação da auditoria full-system em `audit/full-system/` (9 arquivos)
+ nota de assimetria bool SQLite/MySQL em `docs/REFERENCIA.md`.

## Motivação

RHP-AUD-005: `true` gravado volta `1` (`Int`) no SQLite/MySQL (sem bool
nativo); só PG round-tripa `Bool`. Mapear 0/1→bool corromperia inteiros —
decisão: documentar, não mudar código.

## O que mudou

- `audit/full-system/`: INVENTORY, TEST_PLAN, FINDINGS (matriz RHP-AUD-*),
  LIMITATIONS, SECURITY, PERFORMANCE, FIXES, REGRESSION (+ este FINAL
  em FINAL_REPORT.md).
- `docs/REFERENCIA.md` (`db_query`): nota bool SQLite/MySQL vs PG.

## Detalhes técnicos

Só documentação. Probes de sessão em `/tmp/audit/` (não versionadas).

## Compatibilidade e migração

Não se aplica.

## Segurança e performance

Não se aplica (medições em PERFORMANCE.md, sem alegações de ganho).

## Testes e validação

Docs descrevem comportamento validado ao vivo (s08, d10, d12).

## Limitações e riscos conhecidos

Pendências P2/P4 em LIMITATIONS.md (planejamento 0.7.0+).

## Referências

- `audit/full-system/` (todos); `audit/full-system/FINDINGS.md` (RHP-AUD-005).
