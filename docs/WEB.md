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

## `http_serve(porta, handler, opts)`

- `porta` 0–65535 (0 = porta livre do SO; o anuncio `rhp: ouvindo em ...`
  mostra onde escutou). `host` (opt): interface de escuta —
  `"127.0.0.1"` (padrao, so local), `"localhost"` (apelido do padrao),
  `"0.0.0.0"` (todas IPv4) ou IP literal (v4/v6) especifico.
- `handler`: nome de `fn` como string **ou** a propria funcao como valor
  (`http_serve(8080, handler, {})` resolve no topo; closures nao sao
  suportadas — erro claro em vez de hang).
- Opts: `max_conexoes` (default 32, teto 1024; acima = 503),
  `max_requisicoes` (0 = para sempre; N = serve N e retorna — ancora de
  testes), `limite_taxa` (req/s por IP; excedeu = 429), `max_passos`
  (default 10M instrucoes por request; 0 = sem limite; estouro = 500 e o
  servidor segue), `max_heap_bytes` (0 = sem limite).
- Request malformado = 400 (`requisicao invalida`; inclui cabecalhos acima
  de 64 KiB, `Content-Length` invalido, corpo incompleto e
  `Transfer-Encoding` desconhecido). `chunked` e suportado (RFC 9112
  §7.1; tem precedencia sobre `Content-Length`); corpo sempre entregue
  como string UTF-8 ao handler.
  Falha de `accept` nao derruba o servidor (log + nova tentativa).
- `http_serve` dentro do handler = erro (500), nao hang.
- `exit(codigo)` no handler encerra o servidor (codigo de saida).

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

- Bind padrao e LOCAL (`127.0.0.1`): outro container/host nao alcanca.
  Para expor (Docker, reverse proxy, port forwarding), use
  `http_serve(8080, "handler", {"host": "0.0.0.0"})` — e entenda que
  TODAS as interfaces passam a aceitar conexoes (restrinja com firewall /
  security groups; nao exponha debug/admin sem autenticacao).
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

## Performance do roteador (trie ordenada)

O despacho usa uma trie (`__web_trie`, construida em `web_registrar`) em vez
de testar cada padrao em sequencia: um `split` do caminho por request,
descida por prefixo estatico (mapa) + ramo `:param`, e escolha do menor
indice global entre os candidatos — **exatamente** a mesma semantica do
linear (primeiro match vence; `Allow` identico, provado pelo teste
diferencial `framework/web_trie_test.rhp`, que inclui armadilhas de ordem).

Medicao (build de desenvolvimento, mesma maquina/carga: 100 rotas
estaticas, match na ultima, 100 dispatches): linear ~7s, trie ~1s (~7x;
a vantagem cresce com o numero de rotas, pois o linear repete `split` +
comparacao por rota). Para apps tipicos (dezenas de rotas) ambos ficam
bem abaixo de 1ms por request em build otimizado.

## Limitacoes v1

- Topo do programa executa por request no worker (idempotente): nao coloque
  efeitos nao-idempotentes no topo (use handlers).
- `async` em handlers: handlers sao `fn` sincronas; para I/O concorrente,
  use `async_spawn`/`await` dentro do handler (worker suporta).
