# RFC 0020 — Conectores TCP, Redis e MySQL (Etapa 5)

- ID: RFC-0020
- Data: 2026-11-16
- Estado: aceita (autorização prévia do programa Etapas 3–5) e implementada
- Contexto: Etapa 5, item "sockets/Redis/MySQL". Rede programável além
  de HTTP/servidor (Fases A–C) e SQLite/PG (Fase B).

## 1. Problema

RHP fala HTTP (cliente/servidor) e SQL (SQLite/PG), mas não TCP cru,
Redis ou MySQL — três peças padrão de sistemas reais (cache, filas,
banco legado).

## 2. Objetivo e não-objetivos

Objetivo: `tcp_connect/send/recv/close` (sockets com timeout);
`redis_open/redis_exec/redis_close` (RESP2); `db_open("mysql:...")`
+ `db_exec/db_query/db_close` reaproveitados (protocolo MySQL 4.1 com
`mysql_native_password`), com mock server em-process nos testes.

Não-objetivos: TLS nos conectores (use stunnel/`rhp build`+proxy;
futuro com ADR 0012 reaproveitada); Redis Cluster/Sentinel;
prepared statements binários MySQL (só texto); async nao-bloqueante
(documentado: bloqueante com timeout; paralelismo via `async_*`).

## 3. Proposta

- Sockets (`crates/rhp-vm/src/tcp.rs`): `TcpHandles` (handles `Int`,
  como `DbHandles`); `tcp_connect(host, port)` (timeout 5s, DNS do
  sistema); `tcp_send(h, str)` => bytes; `tcp_recv(h, max)` => `Str`
  (ate `max`; `Null` em EOF limpo); `tcp_close(h)` (idempotente? Nao:
  handle invalido = erro, como `db_close`).
- Redis (RESP2 sobre TCP): `redis_open(host, port)` (sem AUTH v1;
  servidor com senha = erro claro na saudacao `-NOAUTH`;
  documentado); `redis_exec(h, [cmd, args...])` => `Str`/`Int`/`Array`/
  `null`; erro RESP (`-...`) vira `VmError`; `redis_close`.
- MySQL via `db_*` existente: `db_open("mysql:host=.. port=.. user=..
  [password=..] [dbname=..]")`; handshake 4.1 + `mysql_native_password`
  (SHA1 duplo com seed); `COM_QUERY` texto; tipos comuns mapeados
  (`LONG/LONGLONG`=>`Int`, resto textual=>`Str`, `NULL`=>`null`,
  resto=>`Str` com nota); `COM_QUIT` no close.
- Testes: echo server TCP in-process; mock Redis scriptado; mock
  MySQL (greeting+auth+query+quit). Sem dependencias externas
  (servidor Redis local exige auth desconhecida; sem mysqld — mocks
  deterministicos, documentado como lacuna de interop real).

## 4. Alternativas

- Reusar `http_fetch` para tudo: Redis/MySQL nao sao HTTP — rejeitada.
- `mysql` via crate externa: dependencia nova + sem controle do
  protocolo (politica std-first) — rejeitada.
- RESP3/hello: complexidade sem necessidade (RESP2 universal) —
  rejeitada (v1).

## 5. Compatibilidade

Aditivo (builtins + `db_open("mysql:...")`). `db_open("pg:...")` e
SQLite inalterados.

## 6. Segurança

Timeouts 5s (connect/read); sem TLS (declarado); senha MySQL nunca em
log (erros nao incluem credenciais); `password` so em memoria.
`tcp_connect` nao executa nada alem de conectar (sem protocolos
automaticos). Limites: `max` de `tcp_recv` teto 8 MiB (DoS de leitura).

## 7. Performance

Sincrono e bloqueante (documentado); paralelismo via `async_*`.
Sem bench dedicado (I/O local; sem regressao possivel no nucleo).

## 8. Plano de implementacao

1. Sockets: modulo + handles + 4 builtins + teste echo.
2. Redis: RESP encode/decode + 3 builtins + mock.
3. MySQL: handshake/auth/query/quit + mapeamento + mock.
4. Spec (`db_open` + tabela), exemplos, `changes/`.

## 9. Plano de testes

- Echo TCP (envia/recebe/fecha, EOF, handle invalido).
- Redis: SET/GET/INCR/DEL/erro (`-`), tipos (int/bulk/nil/array).
- MySQL: connect+auth, SELECT com tipos, INSERT (linhas afetadas),
  erro SQL, close + reuso invalido.
- Suite verde; sem flake (timeouts locais, sem rede externa).

## 10. Migracao e rollout

Nenhuma migracao (aditivo). Rollout normal.

## 11. Criterios de aceitacao

- [x] RFC aceita antes do codigo (autorizacao previa 2026-11-16).
- [ ] 3 conectores com testes contra servidores in-process.
- [ ] Suite verde; spec + exemplos + `changes/`.
