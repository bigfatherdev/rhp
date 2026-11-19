# RFC 0027 — RHP Web (framework oficial)

- ID: RFC 0027
- Status: aceita e implementada
- Data: 2026-11-19
- Versao: unreleased (0.4.0 planejada)

## 1. Problema

`http_serve` e primitivo (handler unico, sem rotas/params/middleware).
APIs exigem roteamento, JSON, erros 404/405 e seguranca padrao.

## 2. Objetivo e nao-objetivos

- Objetivo: `framework/web.rhp` com rotas (`GET/POST/PUT/PATCH/DELETE/HEAD/
  OPTIONS`), params `:id`, middlewares, `web_json`/`web_text`/`web_redirect`,
  404/405, cabecalhos de seguranca, teto de payload, templates via
  `template()`, exemplo funcional.
- Nao-objetivos: ORM, sessoes prontas, validacao declarativa, websockets,
  TLS proprio (usa o do `http_serve`/`http_fetch`).

## 3. Proposta

```rhp
import { web_get, web_json, web_despachar } from "web.rhp";
web_get("/users/:id", fn(req) { return web_json({ id: req["params"]["id"] }); });
fn handler(req) { return web_despachar(req); }
http_serve(8080, "handler", {});
```

- Primeiro match vence; metodo errado com padrao certo = 405 + `Allow`.
- Middleware `fn(req)` retorna `null` (continua) ou resposta (curto).
- `web_finalizar`: normaliza `str` e injeta `x-content-type-options: nosniff`,
  `x-frame-options: DENY`.
- App declara `handler` no entry (o `http_serve` exige nome global do
  modulo de entrada; o framework nao chama `http_serve` por si).
- Worker `http_serve` reconstrói o topo por request (inclui registros de
  rota) — ver mudanca em `synthetic_program` (executa topo exceto a chamada
  `http_serve`; topo deve ser idempotente).

## 4. Alternativas consideradas

- Rotas como classe `App` com estado mutavel compartilhado: rejeitada
  (workers sao isolados; estado global por request e reconstruido).
- Macros de rota em compilador: rejeitada (biblioteca em RHP basta).

## 5. Compatibilidade

Aditiva (novo arquivo + mudanca no worker descrita abaixo). `http_serve`
puro segue igual para programas sem topo com efeitos.

## 6. Seguranca

Sem stack traces (500 opaco do `http_serve`); escape via `template()`;
teto de payload 1 MiB; rate limiting via `opts` do `http_serve`; cookies
seguras por app (documentado); sem `eval` de templates.

## 7. Performance

Roteamento O(R) por request (R = rotas; tipico < 100); sem alocacao alem
de mapas/arrays do RHP. Thread-por-conexao herdada do `http_serve`.

## 8. Plano de implementacao

`framework/web.rhp` + `framework/exemplo_api.rhp` + teste HTTP real (rotas,
params, 404) + docs (`docs/WEB.md`) + estrutura de projeto recomendada.

## 9. Plano de testes

Rotas, params, 405, middleware, JSON, seguranca (headers), exemplo executa.

## 10. Migracao e rollout

Novos projetos vendorizam `web.rhp` (copia) ou importam por caminho relativo;
futuro `rhp pkg` distribuira.

## 11. Criterios de aceitacao

API REST funcional sobre HTTP real; 404/405 corretos; exemplo roda.
