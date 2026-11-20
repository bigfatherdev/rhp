# FIXES.md — Correções (causa raiz + prova antes/depois)

## IND-001 — HOF `pending` obsoleto (P0)

- **Causa**: `pending` vive no frame chamador (sobrevive ao unwind); o
  callback que lança é truncado, mas nada limpa o `pending`. O próximo
  `Return` para aquele frame o consome como "retorno do callback":
  erro fantasma (`comparador ... encontrado string/null`) ou avanço
  espúrio do HOF (resultado errado silencioso).
- **Fix**: `Frame.pending_cb` (índice do callback); `push_hof_call`
  centraliza armação; `unwind` limpa pendings cujo callback foi truncado
  (`cb > depth`), preservando os vivos; `Return` descarta a amarra junto.
- **Prova**: `m08`/`m11`/`m12` falhavam com erro fantasma, passam com
  `n1:fundo`/`42` corretos; regressão `runs_hof_throw_...` falha com o
  `unwind`-clear desativado e passa com ele; suíte 370/370.

## IND-002 — Dreno pg até ReadyForQuery (P1)

- **Causa**: `return Err` no `b'E'` sem drenar o `Z`; próximo comando lia
  `Z` alheio (sucesso falso) e o erro surgia um comando atrasado.
- **Fix**: flag `failure` + dreno limitado (1M) até `Z`; só então devolve.
- **Prova**: `d14` (v3-erro na v3, v4-erro na v4); teste live estendido
  passa; suíte verde.

## IND-003 — Dreno mysql em `exec(SELECT)` (P1)

- **Causa**: `command()` lê 1 pacote; SELECT deixava result-set no socket.
- **Fix**: `drain_result_set` (colunas→EOF→linhas→EOF/ERR, teto 1M) antes
  do erro "use db_query".
- **Prova**: `d15` (query seguinte `[{um: 1}]`); mock-teste falha com
  dreno desativado e passa com ele.

## IND-004 — `fs_rename`/`fs_remove`/`fs_mkdir` (P2→feito)

- Adição compatível: ids 80–82, aridades, docs (83 builtins), teste
  `runs_fs_lifecycle_builtins`. Comportamento validado (incl. erros OS).

## IND-005 — USAGE `build` (P3)

- Texto `[--check]` → `[saida]` (docs+código já diziam saída) + teste.
