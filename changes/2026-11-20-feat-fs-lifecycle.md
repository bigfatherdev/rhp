# [Added] — `fs_rename`/`fs_remove`/`fs_mkdir` + correções P3 (RHP-AUD-IND-004/005/006)

- ID: 2026-11-20-feat-fs-lifecycle
- Data: 2026-11-20
- Versão: 0.6.2
- Tipo: Added (compatível)
- Compatibilidade: Compatível (80→83 builtins; nada removido/alterado)

## Resumo

Ciclo de vida de arquivos completo: `fs_rename(de, para)`, `fs_remove`
(arquivo ou diretorio vazio), `fs_mkdir` (com pais). Bônus: USAGE do
`rhp build` corrigido (`[saida]`, era `[--check]` que criava arquivo
literal) + nota `db_exec(SELECT)` (MySQL erra, PG retorna tag).

## Motivação

Auditoria independente: sem os três, RHP não gerencia arquivos (P2);
USAGE mentia e gerava artefato `--check` (P3); inconsistência drivers
sem documento (P3).

## O que mudou

- `Builtin::{FsRename,FsRemove,FsMkdir}` (ids 80–82), aridades, impl
  (`std::fs`, erros OS limpos), REFERENCIA (83 builtins + 3 linhas).
- `USAGE`: `rhp build <arquivo.rhp> [saida]` + teste que trava o texto.
- REFERENCIA `db_exec`: nota MySQL-vs-PG em SELECT.

## Detalhes técnicos

Sem `unsafe`, sem dependências. `fs_remove` usa `symlink_metadata`
(arquivo vs diretorio).

## Compatibilidade e migração

Adição pura. Migração: não necessária.

## Segurança e performance

Mesmo modelo de privilégios do processo (documentado); sem sandbox nova.
O(1) além do I/O.

## Testes e validação

- `runs_fs_lifecycle_builtins` (ciclo + 3 erros); `usage_documents_...`.
- suíte 370/370; fmt/clippy limpos.

## Limitações e riscos conhecidos

Sem `fs_chmod`/`stat` (P4).

## Referências

- `audit/0.6.1/FINDINGS.md` (IND-004/005/006), `LIMITATIONS.md`.
