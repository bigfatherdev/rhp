# [Added] — TCP, Redis e MySQL (RFC 0020, Etapa 5)

- ID: 2026-11-16-feat-conectores-tcp-redis-mysql
- Data: 2026-11-16
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (7 builtins novos ids 68–74 + `DbConn::Mysql`;
  sem renumerar; `db_*` existentes inalterados)

## Resumo

Sockets TCP (`tcp_connect/send/recv/close`), cliente Redis RESP2
(`redis_open/exec/close`) e cliente MySQL via `db_open("mysql:...")`
(protocolo 4.1 + `mysql_native_password`, `db_exec`/`db_query`/`db_close`
reaproveitados). Testes contra servidores in-process (echo TCP, Redis
scriptado, MySQL com handshake+auth verificados).

## Motivação

Etapa 5, item "sockets/Redis/MySQL" (RFC 0020, aceita sob autorização
prévia): rede programável além de HTTP/SQLite/PG.

## O que mudou

- `tcp.rs`: `TcpConn` (timeout 5s, teto de leitura 8 MiB) + `TcpHandles`.
- `redis.rs`: RESP2 encode/decode (teto 128 níveis), `RedisConn`,
  `RedisHandles`; `redis_exec(h, [cmd, args])`.
- `mysql.rs`: handshake 4.1, auth `mysql_native_password` (SHA-1
  próprio, testado contra vetores NIST multibloco + `native_password`
  determinístico), `COM_QUERY` texto com `?` interpolado (escape
  MySQL), `COM_QUIT`, mapeamento de tipos (INT/FLOAT/Str/null),
  `read_packet`/`write_packet` com teto 16 MiB.
- Builtins 66→74 no total do lote (async 66-67 já): tcp 68-71, redis
  72-74 + `DbConn::Mysql`; `call_builtin` ganha `net`/`redis`
  (`set_program` inalterado).
- `Vm`: `tcp` + `redis` handles (paralelos a `db`; `reset` não limpa,
  como `db`).
- `LANGUAGE_SPEC.md` (§6: 7 linhas + `db_open("mysql:...")` + tipos),
  `CHANGELOG.md`.

## Compatibilidade e migração

Aditivo; sem migração. Sem exemplos `.rhp` executáveis (exigiriam
servidores; CI e coberto por testes com mocks).

## Segurança e performance

Timeouts 5s em tudo; teto de leitura; sem TLS (declarado); senha
MySQL nunca em erro/log; `password` so em memoria. Sem `unsafe` novo
(SQLite FFI preexistente intocado). Performance: bloqueante com
timeout; paralelismo via `async_*` (documentado); sem regressao no
nucleo (suite verde).

## Testes e validação

- `conectores.rs` (TCP echo + Redis scriptado: SET/GET/INCR/DEL/erro/close).
- `mysql.rs #[cfg(test)]`: SHA-1 NIST + 6 vetores multibloco, handshake
  com auth VERIFICADA, query com tipos, INSERT, erro 1064, close/reuso.
- `cargo test --workspace`: 321 passed, 0 failed; `fmt --check` limpo;
  `clippy -D warnings` 0 warnings.

## Limitações e riscos conhecidos

- Sem TLS nos conectores (stunnel/proxy; follow-up com ADR 0012).
- Redis sem AUTH (servidor com senha = erro claro); sem cluster.
- MySQL: sem prepared statements binarios; interpolacao client-side
  com escape (documentado); tipos data/hora => `Str`.
- Testes contra mocks (sem servidor real no CI; Redis local exige
  senha desconhecida; sem mysqld no ambiente) — interop real =
  follow-up documentado.
- `for` sobre gerador infinito sem `break` trava (como `while true`).

## Referências

- RFC 0020 (`RFCs/0020-conectores-tcp-redis-mysql.md`, aceita e implementada).
