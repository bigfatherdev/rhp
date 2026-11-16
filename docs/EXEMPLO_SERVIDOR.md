# Exemplo — servidor HTTP em 10 linhas

`http_serve(porta, "app", opts)` sobe um servidor real (thread por
conexao). O handler recebe `req` (mapa) e devolve `Str` (corpo, 200) ou
mapa (`status`/`cabecalhos`/`corpo`).

```rhp
fn app(req) {
  if req["caminho"] == "/oi" {
    return "ola " + req["query"]["nome"];
  }
  if req["caminho"] == "/eco" {
    return {status: 201, cabecalhos: {"x_eco": "sim"}, corpo: req["corpo"]};
  }
  throw "rota desconhecida: " + req["caminho"];
}

http_serve(8080, "app", {});
```

```bash
rhp run servidor.rhp &
curl "http://127.0.0.1:8080/oi?nome=mundo"   # ola mundo
curl -X POST http://127.0.0.1:8080/eco -d abc  # 201 + ecoa "abc"
curl http://127.0.0.1:8080/nao-existe         # erro interno (500, sem vazar)
```

Regras que valem:

- `req` = `{metodo, caminho, query, cabecalhos, corpo}` (chaves de
  cabecalho em minusculas; query decodificada).
- Erro no handler = 500 `erro interno` (detalhe so no stderr do servidor);
  servidor segue atendendo.
- `exit(codigo)` no handler termina o SERVIDOR com o codigo.
- Handlers nao tem estado: cada request compila e executa isolado
  (abra `db_*` por request).
- `{"max_requisicoes": N}` serve N requests e sai (util em testes);
  `{"max_conexoes": N}` responde 503 acima do teto.
