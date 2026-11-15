# [Added] — PostgreSQL via protocolo próprio (`db_*` com `pg:`)

- ID: 2026-11-15-feat-postgres
- Data: 2026-11-15
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (mesmos 4 builtins; `db_open("pg:...")`
  escolhe o backend; SQLite intacto)

## Resumo

`db_open("pg:host=.. port=.. user=.. dbname=..")` conecta via protocolo
v3 sobre TCP std (startup + `trust`, extended query com params em texto,
tipos por OID, `Terminate` ao fechar). Mesma API do SQLite (`?` vira
`$1` no SQL — passado direto, documentado por backend).

## Motivação

Fase B3 do plano de sistemas: persistência cliente-servidor testada
contra servidor real (Docker `postgres:16-alpine`), sem mock e sem
dependência externa — conforme exigência da auditoria.

## O que mudou

- `rhp-vm/src/pg.rs` (novo, ~330 linhas, zero `unsafe`): framing
  big-endian, extended query (Parse/Bind/Describe/Execute/Sync),
  `RowDescription`/`DataRow`/`ErrorResponse`/`CommandComplete`, tipos por
  OID (int2/4/8 => `Int`, float4/8 => `Float`, bool, numeric, resto =>
  `Str`), `trust` apenas (MD5/SASL = erro claro).
- `rhp-vm`: `DbConn::Postgres`, dispatch por prefixo em `db_open`,
  conversão de params/valores, `db_close` com `Terminate`.
- Sem mudança de bytecode/gramática (mesmos builtins/ids 44–47).
- Spec: `LANGUAGE_SPEC.md` §6 (forma `pg:`, `$n`, tipos).

## Compatibilidade e migração

Compatível. Migração: prefixar `pg:` + trocar `?` por `$n`; auth só
`trust` (senha/SCRAM = pendente declarado).

## Segurança e performance

Sem `unsafe`, sem deps, sem TLS (só TCP local/rede confiável nesta
fase). Timeouts: conexão 5 s, I/O 30 s. Sem prepared cache (cada chamada
faz Parse; documentado). Erros do servidor com código (`[42601]`).

## Testes e validação

- Falha-antes: `db_open("pg:...")` inexistente (só SQLite).
- Servidor real (`rhp-pg-test`, `postgres:16-alpine`, trust, porta
  dedicada 55432): CRUD e2e via `rhp run` (tipos bool/int/float/text,
  `$n` com tipos, `[42601]` em SQL inválido) + 3 erros de conexão sem
  servidor na VM. Sem mock do protocolo em lugar nenhum.
- `cargo test --workspace`: 199 passed, 0 failed (197 antes + 2 novos:
  `rejects_bad_pg_conninfo` + e2e via `rhp run` contra container real).
- `cargo fmt --check` ok; `cargo clippy --workspace --all-targets
  -D warnings` ok; `tools/check-changes.sh origin/main` ok.

## Limitações e riscos conhecidos

Só `trust` (sem senha/SCRAM — inadequado p/ produção exposta; declarar,
não esconder). Sem COPY/LISTEN/transações dedicadas (SQL resolve). Sem
pool de conexões (1 handle = 1 socket; documentado). Tipos exóticos
caem para `Str`. Container de teste não versionado no repo (comando
documentado no teste).

## Referências

ADR 0011 (implementada por completo: SQLite + PG). Fase B concluída.
