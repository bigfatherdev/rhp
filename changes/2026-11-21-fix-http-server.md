# Fixed — HTTP: `+` em caminho, forma absoluta, obs-fold/NUL, resposta no `exit`

- ID: 2026-11-21-fix-http-server
- Data: 2026-11-21
- Versão: 0.7.2
- Tipo: Fixed
- Compatibilidade: Compatível

## Resumo
`/a+b` virava `/a b` (colidia com `/a%2Bb`); `GET http://h/...` virava
caminho literal; linha de continuação (obs-fold) era ignorada em
silêncio; NUL passava até o handler; `exit()` no handler derrubava a
conexão sem resposta.

## Motivação
Auditoria 0.7.2: HTTP-001 (P2), HTTP-008 (P4), HTTP-007 (P3),
HTTP-009 (P3/P4), HTTP-006 (P3).

## O que mudou
- `percent_decode(text, plus_as_space)`: `+`→espaço só em query.
- `strip_absolute_form`: forma absoluta normaliza para origem.
- Linha de cabeçalho sem `:` = 400 (antes, ignorada).
- NUL em alvo (bruto ou `%00`), nome ou valor de cabeçalho = 400.
- `HandlerFail::Exit` responde 200 vazio + `Connection: close` antes.

## Detalhes técnicos
`crates/rhp-vm/src/http_server.rs` (`percent_decode`, `read_request`,
`strip_absolute_form`, `handle_one`). Validados live (curl + socket cru).

## Compatibilidade e migração
Compatível. Rotas que dependiam de `/a+b`→`/a b` (colisão) precisam de
`%20`; proxies que mandavam forma absoluta passam a casar (melhoria).
Sem migração geral.

## Segurança e performance
Fecha vetor de smuggling (obs-fold) e confusão de roteador/fs (NUL,
colisão `+`). Sem custo (checagens no parse já alocado).

## Testes e validação
- `target_normalization_and_rejections`,
  `read_request_rejects_obs_fold_and_nul` (loopback real): ok.
- Probes live: `curl` + socket cru (400s, `/a+b`, absoluta, exit): ok.
- `cargo test --workspace`: 395 passed, 0 failed.

## Limitações e riscos conhecidos
503/429 sob carga podem encerrar com RST (documentado; retentar).
Sem prazo por handler (documentado; 0.8.0 com RFC).

## Referências
HTTP-001/006/007/008/009 (auditoria 0.7.2); RFC 9112 §6.3/§7.1.
