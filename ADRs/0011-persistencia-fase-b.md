# ADR 0011 — Persistência da Fase B: SQLite (FFI mínimo) + PostgreSQL (protocolo)

- Data: 2026-11-15
- Estado: aceita (autorização do plano de sistemas; implementada em B2/B3)
- Contexto: auditoria exige drivers testados contra servidores/bibliotecas
  reais; Fase B do plano de sistemas (RHP para sites/sistemas).

## Decisão

1. **SQLite via FFI manual mínimo ao `libsqlite3` do sistema** (sem crate
   externa; `cargo tree` continua só workspace + std).
2. **PostgreSQL via cliente de protocolo próprio** (startup + extended
   query, auth `trust`), testado contra servidor real em Docker.
3. **Mesma API mínima para os dois** (`db_open/db_exec/db_query/db_close`
   com handles `Int`); SQL e placeholders passam direto (documentados por
   backend: `?` no SQLite, `$1` no PG).

## Alternativas consideradas (SQLite)

- (a) `rusqlite` com amalgamation embutida: portável (incl. Windows) e sem
  `unsafe` nosso, mas quebra o zero-dep (primeira crate externa), adiciona
  compilação C (~30 s) e auditoria de dependência. Rejeitada por ora;
  volta se portabilidade estática virar requisito (reabre esta ADR).
- (b) `rusqlite` linkando `libsqlite3` do sistema: mantém dependência de
  SO sem o benefício da portabilidade de (a). Pior dos dois mundos;
  rejeitada.
- (c, escolhida) FFI manual mínimo (`open/prepare/bind/step/column/
  finalize/close/changes/errmsg`): ~150 linhas em 1 módulo isolado,
  `unsafe` justificado item a item (§4), sem dependência nova, reversível
  em 1 commit. `libsqlite3` é onipresente (este ambiente, CI, SOs
  mainstream); ausente = erro claro de link em build.
- (d) Motor SQL próprio: meses de trabalho e risco alto; rejeitada.

## Alternativas consideradas (PostgreSQL)

- Driver via `libpq` (FFI ao sistema): API C estável, mas arrasta
  `libpq`+TLS como dependência de SO e `unsafe` equivalente; sem ganho
  sobre protocolo próprio para o subconjunto alvo. Rejeitada.
- Crate externa (`tokio-postgres`/`sqlx`): quebra zero-dep E puxa runtime
  async inteiro; rejeitada (conflito arquitetural, não só dependência).
- (Escolhida) Protocolo próprio sobre `TcpStream` std: mensagem de
  startup + `trust`, extended query (Parse/Bind/Describe/Execute/Sync,
  parâmetros em texto), parsing de `RowDescription`/`DataRow`/`ErrorResponse`.
  Sem COPY/LISTEN/preparadas cacheadas. Auth password/SCRAM: **fora do
  escopo** (exigiria cripto; documentado como limite; `trust` em rede
  local/testes).

## Consequências

- Novo `unsafe` isolado em `rhp-vm/src/sqlite.rs` (só chamadas à C ABI
  estável; sem aritmética de ponteiros; strings via `CString`/`CStr`;
  todo erro checado; testes contra lib real + `sqlite3` CLI como oráculo
  quando disponível). Revisão de `unsafe` exigida no review.
- Handles `Int` no espaço da VM (tabela por conexão; `db_close` libera;
  fim da VM fecha o resto). Sem variante nova em `Value` (compatível).
- `db_exec` = linhas afetadas (`Int`); `db_query` = array de mapas
  (colunas como chaves; `NULL` => `null`; inteiros => `Int`, resto => `Str`
  no PG-texto; tipos SQLite por afinidade).
- Transações via SQL (`BEGIN`/`COMMIT`), sem API dedicada.
- PG: `trust` apenas; produção com senha = pendente (erro claro se o
  servidor exigir outro método).
- Testes exigem: `libsqlite3` (build+runtime) e Docker com imagem
  `postgres` (B3). Sem eles, a suíte relevante é pulada? Não — **falha
  explícita não é aceitável em CI sem os serviços**; ver plano de testes.

## Plano de testes

- SQLite: CRUD, tipos, `NULL`, erros (SQL inválido, tabela ausente),
  params (`?`), `db_close` + reuso de handle fechado = erro, arquivo
  criado em disco (asserção de existência via `fs_exists`).
- PG: ciclo real contra container (`CREATE`, `INSERT` com `$1`, `SELECT`,
  tipos, erro de SQL, timeout de conexão recusada).
- Sem mock do protocolo (proibido pela missão: servidor real ou nada).

## Riscos

- `unsafe` no FFI (mitigado: módulo único, revisão, testes).
- Divergência de versão do `libsqlite3` do SO (API usada é estável há
  20 anos; erro de link é explícito).
- PG sem SCRAM = inadequado para produção com senha (declarado, não
  escondido).
