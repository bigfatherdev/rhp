# [Added] — `http_serve` com `host` configurável (bind de rede)

- ID: 2026-11-20-feat-http-host-bind
- Data: 2026-11-20
- Versão: 0.6.0
- Tipo: Added (opt compatível)
- Compatibilidade: Compatível (default preserva `127.0.0.1`)

## Resumo

`http_serve(8081, "app", {"host": "0.0.0.0"})` agora escuta em todas as
interfaces IPv4. Também aceita IP literal específico (v4/v6). O default
segue `127.0.0.1` (decisão consciente e documentada).

## Motivação

Bug P0/P1 da missão 0.6.0 (§4): bind fixo em `127.0.0.1` impedia acesso
entre containers, reverse proxy separado e port forwarding — a arquitetura
exigia workaround de network namespace compartilhado.

## O que mudou

- `ServeOpts.host` (default `"127.0.0.1"`); validado em `parse_serve_opts`
  (`IpAddr::from_str`; outros nomes rejeitados com erro claro.
  `"localhost"` virou apelido de `127.0.0.1` em seguida — ver
  `changes/2026-11-20-feat-http-chunked-localhost.md`).
- `serve()` faz bind em `(host, porta)`; erro de bind cita `host:porta` e
  causa provável (porta em uso, permissão, interface inexistente).
- `rhp: ouvindo em {host}:{porta}` mostra onde escutou.
- Erro novo mapeia para E3000 genérico (sem ajuda enganosa).

## Detalhes técnicos

Arquivo: `crates/rhp-vm/src/http_server.rs` (`ServeOpts`, `parse_serve_opts`,
`serve`). Nenhuma dependência nova; `std::net` apenas. Docs:
`docs/WEB.md` (seção + aviso de segurança), `docs/DOCKER.md` (bind em
container + topologia), `docs/REFERENCIA.md` (linha do `http_serve`).

## Compatibilidade e migração

Chamadas existentes sem `host` comportam-se exatamente como antes.
Migração: adicionar `"host": "0.0.0.0"` onde exposição externa é desejada;
restringir via firewall/security groups (ver `docs/WEB.md`).

## Segurança e performance

Default local preservado; `0.0.0.0` é opt-in documentado. Sem impacto em
throughput (bind ocorre uma vez).

## Testes e validação

- Unitário `rejects_bad_serve_opts`: literais ok / inválidos rejeitados
  (`localhost` aceito como apelido — ver registro chunked/localhost).
- Integração: `host_bind` (anúncio `0.0.0.0` + GET via 127.0.0.1; default anuncia 127.0.0.1), `rejeita_host_invalido` (4 casos), `porta_ocupada`.
- Docker real: servidor `0.0.0.0:8081` em container alcançado via IP interno por outro container + nginx reverso separado + port forwarding (resposta válida nos 3); controle negativo com bind local (connection refused entre containers).
- `cargo test --workspace`: 354 passed, 0 failed.

## Limitações e riscos conhecidos

Sem resolução DNS para `host` (literais apenas); sem HTTPS no `serve`
(terminar TLS no proxy); IPv6 depende do ambiente.

## Referências

- Missão 0.6.0 §4; `docs/WEB.md`, `docs/DOCKER.md`; branch `feat/rhp-0.6.0`.
