# [Added] — `fs_chmod` + `fs_stat` (85 builtins)

- ID: 2026-11-20-feat-fs-stat-chmod
- Data: 2026-11-20
- Versão: 0.7.1
- Tipo: Added
- Compatibilidade: Compatível (adição pura)

## Resumo

Ciclo de vida de arquivos completo: `fs_chmod(caminho, modo)` (Unix,
`Int` octal 0–0o7777) e `fs_stat(caminho)` (mapa `{tamanho,
e_diretorio, e_arquivo, somente_leitura, modificado}` com epoch `Int`).

## Motivação

Auditoria apontou lacuna real: sem eles, RHP não inspeciona nem ajusta
arquivos (P2 resolvido; `fs_rename/remove/mkdir` vieram na 0.6.2).

## O que mudou

- `Builtin::{FsChmod,FsStat}` (ids 83–84), aridades 2/1, impl
  (`std::fs` + `PermissionsExt`; fora Unix = erro claro).
- Docs: linha REFERENCIA; contagem 83→85; padrão `join` p/ strings grandes.

## Detalhes técnicos

Sem `unsafe`, sem dependências. Segue o molde dos builtins `fs_*`.

## Compatibilidade e migração

Adição pura. Migração: não necessária.

## Segurança e performance

Mesmos privilégios do processo (sem sandbox nova). O(1) + syscall.

## Testes e validação

- `runs_fs_lifecycle_builtins` estendido (stat/chmod/roundtrip de
  permissão); suite 375/375; fmt/clippy limpos.

## Limitações e riscos conhecidos

Sem `fs_chown`/ACL (fora do escopo); `modificado` pode ser `null` se o
SO não informar.

## Referências

- `audit/0.6.1/FINDINGS.md` (lacuna de ciclo de vida).
