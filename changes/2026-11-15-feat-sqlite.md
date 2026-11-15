# [Added] — SQLite std-only via FFI mínimo (`db_*`)

- ID: 2026-11-15-feat-sqlite
- Data: 2026-11-15
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (só adiciona builtins; ids 44–47)

## Resumo

`db_open(caminho)` => handle `Int`; `db_exec(h, sql, params)` =>
linhas afetadas; `db_query(h, sql, params)` => array de mapas;
`db_close(h)`. SQLite via FFI manual mínimo ao `libsqlite3` do sistema
(sem crate externa; `cargo tree` intacto), conforme ADR 0011.

## Motivação

Fase B2 do plano de sistemas: persistência local sem dependências novas
e sem servidor — base para sistemas single-node antes do PostgreSQL.

## O que mudou

- `rhp-bytecode`: variantes `DbOpen/DbExec/DbQuery/DbClose`, nomes,
  aridades (1/3/3/1), ids 44–47, anti-deriva estendido.
- `rhp-vm/src/sqlite.rs` (novo): FFI isolado (`open/prepare/bind/step/
  column/finalize/close/changes/errmsg`), `SQLITE_TRANSIENT`, guarda de
  `finalize` em todos os caminhos, `Connection` `!Send`, sem BLOB (erro
  claro), uma instrução por chamada, aridade de `?` checada.
- `rhp-vm`: tabela de handles por VM (`DbHandles`; `db_close` libera,
  fim da VM fecha o resto); `Value` inalterado (sem variante nova).
- Gramática VSCode + teste (4 nomes). Specs: `LANGUAGE_SPEC.md` §6,
  `BYTECODE_SPEC.md` §2/§4.

## Detalhes técnicos

Params RHP => SQL: `Int`/`Float`/`Str`/`Bool`(0/1)/`Null`; resto = erro
(nunca interpolação — sem injection por desenho). Colunas: INTEGER =>
`Int`, FLOAT => `Float`, TEXT => `Str` (UTF-8 validado), NULL => `null`.
`db_exec` descarta linhas de SELECT (documentado). Transações via SQL.

## Compatibilidade e migração

Compatível. Migração: não necessária. Requer `libsqlite3` do SO em
build+runtime (erro explícito se ausente; Linux verificado, demais
plataformas declaradas como não testadas).

## Segurança e performance

`unsafe` restrito a 1 módulo com invariantes documentadas por chamada
(ADR 0011 §4; sem aritmética de ponteiros; `CString`/`CStr`; nulos
checados). Revisão de `unsafe` exigida no review desta entrega. Sem
I/O além do arquivo do banco (permissões do usuário, como `fs_*`).

## Testes e validação

- Falha-antes: builtins inexistiam (auditoria marcava AUSENTE).
- Banco real em temp: CRUD, tipos, `NULL`, SQL inválido, aridade de `?`,
  handle fechado, arquivo criado (`fs` como oráculo) + e2e via `rhp run`.
- `cargo test --workspace`: 197 passed, 0 failed (195 antes + 2 novos:
  `runs_sqlite_crud` + e2e via `rhp run`).
- `cargo fmt --check` ok; `cargo clippy --workspace --all-targets
  -D warnings` ok; `tools/check-changes.sh origin/main` ok.
- `sqlite3` CLI disponível como oráculo manual (não automatizado).

## Limitações e riscos conhecidos

Sem BLOB (RHP não tem bytes). Uma instrução por chamada. Sem `VACUUM`/
`backup` dedicados (via SQL quando suportado). Windows/macOS: compila
se houver `libsqlite3`, mas declarado não testado. Divergência futura
de `libsqlite3` = erro de link explícito.

## Referências

ADR 0011 (implementada para SQLite; PG em B3). Sem ADR nova.
