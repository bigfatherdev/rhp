# [Fixed] — Dreno de erro em pg/mysql (RHP-AUD-IND-002/003)

- ID: 2026-11-20-fix-db-error-drain
- Data: 2026-11-20
- Versão: 0.6.2
- Tipo: Fixed
- Compatibilidade: Compatível (erros passam a sair no comando certo)

## Resumo

Erro no meio de um result-set (pg) ou `db_exec(SELECT)` (mysql) deixava
pacotes não lidos: o próximo comando lia resposta alheia (sucesso falso /
erro atrasado no pg; `lenenc truncado` no mysql).

## Motivação

Auditoria independente com servidores reais (pg 16, MySQL 8, MariaDB 11):
pg mostrava o erro de um comando no comando seguinte; mysql dessincronizava
após `exec(SELECT)`.

## O que mudou

- `pg.rs::extended`: flag `failure` + dreno limitado (1M) até `ReadyForQuery`
  antes de devolver o erro; I/O continua falhando alto.
- `mysql.rs::exec`: `drain_result_set` (colunas→EOF→linhas→EOF/ERR, teto 1M)
  antes do erro "use db_query".
- Teste live pg estendido (erro-then-sucesso); mock mysql asserta dreno
  (falha com dreno desativado).

## Detalhes técnicos

Sem mudança de protocolo; só ordem de leitura. `db_close`/sync intactos.

## Compatibilidade e migração

Comandos que falhavam silenciosamente/adiado agora falham no ponto certo
(mesmo código de erro). Migração: não necessária.

## Segurança e performance

Dreno limitado (sem loop infinito); caminho feliz inalterado.

## Testes e validação

- pg live: v3-erro-na-v3 + v4-erro-na-v4 (antes: v3-ok + v4-com-erro-de-v3).
- mysql live + mock; suíte 370/370; fmt/clippy limpos.

## Limitações e riscos conhecidos

Nenhum novo.

## Referências

- `audit/0.6.1/FINDINGS.md` (IND-002/003), `FIXES.md`.
