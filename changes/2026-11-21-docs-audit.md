# Docs — Correções da auditoria 0.7.2 (deps, `unsafe`, limites, APIs)

- ID: 2026-11-21-docs-audit
- Data: 2026-11-21
- Versão: 0.7.2
- Tipo: Documentation
- Compatibilidade: Compatível

## Resumo
Docs afirmavam "zero dependências", "sem `unsafe`", "sem `expect` em
produção", "limite 128", "testes 216", `max_conexoes` 64, `int()` trunca,
`fs_exists` = legível, `timeout` 0–60s — tudo divergente do código.

## Motivação
Auditoria 0.7.2: DOC-001–DOC-009, LIB-005/006/008/009, HTTP-005,
COMP-004 (adiado com workaround), COMP-008.

## O que mudou
- `ARCHITECTURE.md`: deps reais listadas; `unsafe` isolado no FFI SQLite;
  `expect` só em invariantes internas.
- `README.md`/`SECURITY.md`: sem "zero deps/sem unsafe"; profundidade 64;
  exibição 128; testes 395.
- `LANGUAGE_SPEC.md`: `max_conexoes` 32; `for ;;` sem parênteses;
  `yield`-em-`try` (0.7.2); `i64::MIN` inexpresível + workaround;
  http (RST, sem prazo/handler, exit→200, `+`, absoluta, NUL/obs-fold).
- `docs/REFERENCIA.md`: `int()` (sem truncar), `fs_exists` (existência),
  `fs_stat` (`null` pré-1970), `timeout` (0,60], `argv` (`--`),
  `tcp_recv` (>0), HOFs (só RHP), `db_exec` (literais/`?`, vazio, SELECT=0
  sqlite), `http_serve` (32, RST, 200-no-exit, filtro do topo), teto de exibição.
- `docs/CLI_REFERENCE.md`: REPL (sobrevive + multilinha), `test` recursivo,
  versão 0.7.2.
- `docs/WEB.md`: 200-no-exit, 404/405 com selo, `Allow` único, 413 real.
- `docs/ESTADO-RHP.md`: delta 0.7.2.

## Detalhes técnicos
Só documentação; nenhuma afirmação sem evidência de código/probe.

## Compatibilidade e migração
Sem mudança de comportamento. Sem migração.

## Segurança e performance
Docs de segurança agora honestas (sem fronteira fictícia).

## Testes e validação
- Exemplos `examples/*.rhp` (20 arquivos): todos OK.
- `framework/exemplo_api.rhp`: "rotas ok".
- Âncoras/links revisados.

## Limitações e riscos conhecidos
Adiados p/ 0.8.0 (documentados): TLS MySQL (RFC 0034), `fs_chown`/ACL,
builder de strings, `db_exec(SELECT)` unificado, prazo por handler,
`i64::MIN` literal, fuzz-gate de CI. Rejeitados (com motivo): `and`/`or`,
`in`, truthiness, keep-alive.

## Referências
DOC-001–009, LIB-005/006/008/009, HTTP-005, COMP-004/008 (auditoria 0.7.2).
