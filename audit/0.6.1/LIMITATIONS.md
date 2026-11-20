# LIMITATIONS.md — Limitações (revalidadas + novas)

## Herdadas e confirmadas (sem mudança)

Sem `&&`/`||`; sem pertinência em array; MySQL `caching_sha2_password`;
teto 64; keep-alive/HTTPS/corpo-binário/trailers/closure-handler/recompile;
bool sqlite/mysql→int; O(n²); FS sem sandbox.

## Novas, encontradas nesta rodada

- **`db_exec(SELECT)` inconsistente entre drivers** (IND-006, P3): MySQL =
  erro "use db_query"; PG = tag de contagem. Documentado; sem mudança de
  comportamento (compatibilidade).
- **Ciclo de vida de arquivos** (IND-004, P2): ELIMINADA — `fs_rename`,
  `fs_remove`, `fs_mkdir` (83 builtins). Sem `fs_chmod`/`fs_stat` (P4).
- **USAGE do `build`** (IND-005, P3): ELIMINADA (texto corrigido + teste).

## Recomendação 0.7.0 (inalterada + 1 item)

RFCs `&&`/`||`, pertinência, `caching_sha2`; versionar fuzz de fonte;
builder de strings; `fs_chmod`/`fs_stat`; unificar `db_exec(SELECT)`.
