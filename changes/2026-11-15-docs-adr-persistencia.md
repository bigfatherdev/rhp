# [Added] — ADR 0011: persistência da Fase B

- ID: 2026-11-15-docs-adr-persistencia
- Data: 2026-11-15
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (só documento)

## Resumo

ADR 0011 decide a persistência: SQLite via FFI manual mínimo ao
`libsqlite3` do sistema (sem crate externa) + PostgreSQL via protocolo
próprio contra servidor real em Docker, mesma API mínima
(`db_open/db_exec/db_query/db_close`).

## Motivação

Fase B1 do plano de sistemas: decidir antes de codificar (alternativas
com impacto em zero-dep, `unsafe` e CI).

## O que mudou

- `ADRs/0011-persistencia-fase-b.md` (novo): contexto, decisão,
  4+3 alternativas com motivo de rejeição, consequências (`unsafe`
  isolado, handles, transações via SQL, `trust` apenas no PG), plano de
  testes com servidores/bibliotecas reais, riscos declarados.

## Compatibilidade e migração

Nenhuma (documento). Implementação em B2/B3.

## Segurança e performance

`unsafe` futuro restrito a 1 módulo com revisão exigida; sem I/O novo
nesta etapa.

## Testes e validação

Etapa de documento (B2/B3 trazem a matriz). `check-changes.sh` ok.

## Referências

Plano Fase B. Reabre-se se portabilidade estática virar requisito.
