# [Fixed] — MySQL: `seq` reiniciado por comando (RHP-AUD-008)

- ID: 2026-11-20-fix-mysql-seq-reset
- Data: 2026-11-20
 - Versão: 0.6.1
- Tipo: Fixed
- Compatibilidade: Compatível

## Resumo

`db_open("mysql:...")` conectava, mas todo comando falhava com erro 1156
(`Got packets out of order`): o cliente nunca zerava o número de sequência
(o protocolo exige 0 a cada comando). Conector estava 100% inoperante.

## Motivação

Auditoria full-system com servidores reais (MariaDB 11 + MySQL 8.0):
primeiro `db_exec` falhava em ambos. Nunca testado ao vivo (mock não
checava `seq`).

## O que mudou

- `crates/rhp-vm/src/mysql.rs`: `self.seq = 0` no início de `command()`.
- Mock de teste agora asserta `seq == 0` em todo comando (falha sem o fix).

## Detalhes técnicos

Handshake (seq 1) intacto. Sem mudança de API. `COM_QUIT` também reinicia.

## Compatibilidade e migração

Nenhum programa funcionava antes (sempre erro); agora funciona. Migração:
não necessária. (`caching_sha2_password` segue sem suporte — RHP-AUD-009,
documentado.)

## Segurança e performance

Sem impacto (1 atribuição por comando).

## Testes e validação

- `mysql_query_and_exec_against_mock`: passa com fix; falha sem (verificado).
- Live MariaDB 11 + MySQL 8.0 (`mysql_native_password`): CRUD, tipos,
  6 classes de erro limpas (tabela, bind, fechado, conexão, formato).
- `cargo test --workspace`: 367 passed, 0 failed; fmt/clippy limpos.

## Limitações e riscos conhecidos

Auth `caching_sha2_password` (default MySQL 8) ainda falha no handshake
(P2 planejado).

## Referências

- `audit/full-system/FINDINGS.md` (RHP-AUD-008/009), `FIXES.md`.
