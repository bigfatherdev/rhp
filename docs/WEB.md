# RHP Web — guia do framework oficial

Framework em RHP puro sobre `http_serve` (`framework/web.rhp`). Sem
dependencias externas.

## Instalacao (via `rhp pkg`, recomendado)

```sh
rhp init meu-projeto
rhp pkg add rhp_web /caminho/do/repositorio-rhp/framework
```

Isso vendoriza o framework em `pacotes/rhp_web/` (com `rhp.lock` para CI:
`rhp pkg install --check`). No codigo:

```rhp
import { web_get, web_post, web_json, web_text, web_use, web_logger, web_despachar } from "pacotes/rhp_web/web.rhp";
```

Alternativa sem `pkg` (vendorizacao manual): copie `framework/web.rhp`
para o projeto e importe por caminho relativo.

## API minima

```rhp
import { web_get, web_post, web_json, web_text, web_use, web_logger, web_seguranca, web_despachar } from "web.rhp";

web_use(web_logger);
web_use(web_seguranca);

web_get("/", fn(req) {
  return web_text("RHP Web");
});

web_get("/users/:id", fn(req) {
  return web_json({ id: req["params"]["id"] });
});

web_post("/users", fn(req) {
  return { status: 201, cabecalhos: { "content-type": "application/json" }, corpo: json_stringify({ criado: true }) };
});

fn handler(req) {
  return web_despachar(req);
}

http_serve(8080, "handler", {});
```

## Rotas

- Metodos: `web_get/post/put/patch/delete/head/options(padrao, handler)`.
- Padroes: `/users/:id` (params em `req["params"]`). Primeiro match vence.
- Sem match = 404; metodo errado com padrao certo = 405 + `Allow`.

## Request/response

- `req`: `{metodo, caminho, query, cabecalhos, cookies, corpo}` (+`params`
  apos casar). `query` e mapa; `corpo` e string.
- Resposta: `str` (200 texto) ou mapa `{status?, cabecalhos?, corpo?}`.
- Helpers: `web_json(valor)`, `web_text(s)`, `web_html(s)`,
  `web_redirect(url)`, `web_not_found()`, `web_erro(msg)`.

## Middleware

```rhp
web_use(fn(req) {
  if req["caminho"] == "/privado" {
    return { status: 401, corpo: "nao autorizado" };
  }
  return null; // continua
});
```

`web_logger` (log) e `web_seguranca` (teto de payload 1 MiB) inclusos.

## Seguranca

- Cabecalhos `x-content-type-options: nosniff`, `x-frame-options: DENY`.
- Teto de payload 1 MiB (413). Rate limiting via `http_serve`:
  `http_serve(8080, "handler", { limite_taxa: 100 })`.
- Erros de handler viram 500 opaco (sem stack trace). Escape via
  `template()`/`web_render` (nunca concatene HTML com dados sem escape).

## Templates

```rhp
let html = web_render("Ola, {{nome}}!", { nome: "Rex" });
```

## Banco de dados

Use `db_open/db_query` dentro dos handlers (cada request tem worker
proprio; abra/fecha por request ou use pool externo):

```rhp
web_get("/users", fn(req) {
  let h = db_open("app.sqlite");
  let linhas = db_query(h, "SELECT id, nome FROM users", []);
  db_close(h);
  return web_json(linhas);
});
```

## Estrutura de projeto recomendada

```text
meu-projeto/
├── rhp.toml
├── web.rhp              # copia de framework/web.rhp
├── src/
│   ├── main.rhp         # rotas + handler + http_serve
│   ├── routes/
│   ├── controllers/
│   ├── services/
│   └── views/
└── tests/
```

## Limitacoes v1

- Roteamento O(R) linear (suficiente para dezenas/centenas de rotas).
- Topo do programa executa por request no worker (idempotente): nao coloque
  efeitos nao-idempotentes no topo (use handlers).
- `async` em handlers: handlers sao `fn` sincronas; para I/O concorrente,
  use `async_spawn`/`await` dentro do handler (worker suporta).
