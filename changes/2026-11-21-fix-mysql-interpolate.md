# Fixed — MySQL: `?` em literais/comentários não é placeholder

- ID: 2026-11-21-fix-mysql-interpolate
- Data: 2026-11-21
- Versão: 0.7.2
- Tipo: Fixed
- Compatibilidade: Compatível

## Resumo
`db_exec(h, "SELECT 'a?b'", [])` falhava ("mais `?` que parametros") e,
com literais + reais misturados, os binds desalinhavam (dado errado).

## Motivação
Auditoria 0.7.2: CONN-003 (P2).

## O que mudou
`interpolate` com estado: literais `'...'`/`"..."`/`` `...` `` (com
`\`-escape e `''` duplicada) e comentários `-- `/`#`/`/* */` não contam
`?`. Somente `?` fora deles consome parâmetro.

## Detalhes técnicos
`crates/rhp-vm/src/mysql.rs` (`interpolate`); escape de valores inalterado.

## Compatibilidade e migração
Compatível: SQL válido que antes falhava agora executa; SQL que antes
alinhava errado por `?` literal agora alinha certo. Quem contava com o
erro para SQL com `?` literal precisa remover o workaround. Sem migração
geral.

## Segurança e performance
Evita dado errado por desalinhamento (integridade). O(n) no SQL.

## Testes e validação
- `mysql_interpolate_ignores_question_in_literals_and_comments` (unit): ok.
- `cargo test -p rhp-vm --lib`: 103 passed.
- `cargo test --workspace`: 395 passed, 0 failed.

## Limitações e riscos conhecidos
`--` exige branco após (regra MySQL); `?` em identificador com crase é
literal (correto: crase = nome, não placeholder).

## Referências
CONN-003 (auditoria 0.7.2).
