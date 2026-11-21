# Fixed — Stdlib: `http_serve` no topo, sqlite, mensagens, `tcp_recv`, `argv`

- ID: 2026-11-21-fix-stdlib-builtins
- Data: 2026-11-21
- Versão: 0.7.2
- Tipo: Fixed
- Compatibilidade: Compatível

## Resumo
`echo http_serve(...)` (ou em `if`) no topo dava 500 em todo request;
`exec(SELECT)` devolvia contagem obsoleta; SQL vazio dizia "not an error";
`tcp_recv(h, 0)` fingia EOF; `argv()` incluía o `--`; mensagens de
`regex_replace`/`hmac_verify`/`sqrt(NaN)`/`fs_chmod` enganosas.

## Motivação
Auditoria 0.7.2: LIB-001 (P2), CONN-004 (P3), CONN-005 (P4), LIB-012 (P4),
LIB-004 (P4), LIB-002/LIB-003/LIB-007 (P3/P4).

## O que mudou
- `is_serve_stmt`: filtro por "executaria `http_serve` no topo" — cobre
  `Echo`, `Assign`, `Destructure`, `Static`, `if`/`while`/`for`/`switch`/
  `try`/blocos; definições (`fn`, classes, closures) mantidas.
- `sqlite exec`: após bind, `column_count > 0` devolve 0 (bind continua
  checando aridade/tipo antes).
- `prepare`: SQL vazio = "`db`: SQL vazio".
- `tcp_recv`: `max <= 0` = erro (antes, 0 fingia EOF).
- `strip_separator`: `--` inicial fora de `argv()` (4 ramos do `run`).
- Mensagens: `regex_replace`/`hmac_verify` listam os 3 tipos;
  `sqrt(NaN)` = "indefinido"; `fs_chmod` mostra `0o...`.

## Detalhes técnicos
`crates/rhp-vm/src/http_server.rs` (filtro + `block_calls_serve`),
`sqlite.rs` (`exec`, `prepare`), `builtins.rs` (`TcpRecv`, mensagens),
`crates/rhp-cli/src/main.rs` (`strip_separator`, `run_repl` NO — ver tooling).

## Compatibilidade e migração
Compatível. `tcp_recv(h, 0)` e `argv` com `--` mudam de comportamento,
mas ambos eram armadilhas documentadas como corrigidas. Sem migração.

## Segurança e performance
Sem impacto de performance (checagens O(1) no caminho de setup).

## Testes e validação
- `synthetic_program_filters_echo_and_compound_serve` (unit, sem rede): ok.
- `rhp_run_argv_skips_dashdash_separator`: ok; probes manuais sqlite
  (`exec(SELECT)`=0, vazio=erro): ok.
- `cargo test --workspace`: 395 passed, 0 failed.

## Limitações e riscos conhecidos
Filtro é conservador (bloco com `http_serve` em ramo morto também é
filtrado do worker — estado equivalente, sem execução real).

## Referências
LIB-001/002/003/004/007/012, CONN-004/005 (auditoria 0.7.2).
