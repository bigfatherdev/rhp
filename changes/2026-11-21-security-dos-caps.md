# Security — Teto em pré-alocação de rede + `RHP_CA_BUNDLE=""` + userinfo

- ID: 2026-11-21-security-dos-caps
- Data: 2026-11-21
- Versão: 0.7.2
- Tipo: Security
- Compatibilidade: Compatível

## Resumo
Resposta Redis/MySQL maliciosa abortava o processo (`with_capacity`
gigantesco, sem `catch` possível); `RHP_CA_BUNDLE=""` quebrava todo
`https`; `usuario:senha@host` vazava o segredo em erros/logs.

## Motivação
Auditoria 0.7.2: CONN-001/CONN-002 (P0), SEC-002 (P3), SEC-003 (P3).

## O que mudou
- `redis.rs`: array RESP > 1M elementos = erro; reserva via `try_reserve`.
- `mysql.rs`: contagem de colunas > 100k = erro ("colunas demais").
- `tls.rs`: `RHP_CA_BUNDLE=""` = só embutidas (doc já prometia).
- `builtins.rs` (`do_request`): autoridade com `@` = erro claro, sem ecoar.

## Detalhes técnicos
`crates/rhp-vm/src/redis.rs` (`MAX_ARRAY_LEN`), `mysql.rs`
(`MAX_COLUMNS`), `tls.rs` (`client_config`), `builtins.rs` (userinfo).
Leitores de pacote já capavam em 16 MiB (`MAX_PACKET`) — inalterados.

## Compatibilidade e migração
Compatível. Arrays Redis >1M e resultados >100k colunas agora erram em
vez de abortar/OOM (nenhum programa válido dependia disso). URLs com
userinfo nunca foram suportadas — quem embutia deve usar `Authorization`.

## Segurança e performance
Elimina 2 aborts remotos (DoS sem recuperação) + 1 vazamento de segredo.
Sem custo no caminho comum (1 comparação).

## Testes e validação
- `redis_open_rejects_giant_array_without_abort` (fake `*999999999`): ok.
- `mysql_query_rejects_absurd_column_count_without_abort` (fake u64 5e9): ok.
- `rhp_run_http_userinfo_rejected_without_leak`,
  `rhp_run_https_with_empty_ca_bundle_reaches_network`: ok.
- `cargo test --workspace`: 395 passed; fuzz 1000 limpo.

## Limitações e riscos conhecidos
Resultados grandes (legítimos) seguem sem paginação (buffer integral —
limite de desenho documentado). Redis sem AUTH (v1, documentado).

## Referências
CONN-001, CONN-002, SEC-002, SEC-003 (auditoria 0.7.2); RFC 0020.
