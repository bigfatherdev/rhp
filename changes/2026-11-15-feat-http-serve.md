# [Added] — Servidor HTTP: `http_serve` (Fase C2, RFC 0002)

- ID: 2026-11-15-feat-http-serve
- Data: 2026-11-15
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (builtin novo, id 48; nada existente muda)

## Resumo

`http_serve(porta, handler, opts)` (aridade 3): thread por conexão,
cada request recompila na worker a partir de `Arc<Program>` e chama
`fn handler(req)`; devolve `Str` (200) ou mapa
`{status?, cabecalhos?, corpo?}`. Erro no handler = 500 sem vazar;
`exit(codigo)` termina o servidor.

## Motivação

Fase C2: sem servidor não há sites/sistemas. Implementa a RFC 0002
aceita (modelo thread-per-connection, isolamento total por request).

## O que mudou

- `rhp-vm`: `Vm::program` (`Arc<Program>`) + `set_program` + `run_value`;
  `http_server.rs` novo (parse de request, accept não-bloqueante,
  worker por conexão, `max_conexoes`/`max_requisicoes`, timeouts 30 s,
  `CompileHandlerCall` com `Pop` final removido sob asserção,
  validação de cabecalhos de resposta).
- Builtin `HttpServe` (id 48): valida porta/handler/opts (`max_conexoes`
  1–1024, `max_requisicoes` >= 0, chaves desconhecidas = erro).
- CLI: `run_program` registra o programa na VM (`set_program`).
- Gramática + teste, `LANGUAGE_SPEC.md` §6, `BYTECODE_SPEC.md` §2/§4,
  `docs/EXEMPLO_SERVIDOR.md` (verificado de verdade) + link no
  `GETTING_STARTED.md`.

## Detalhes técnicos

`req` = `{metodo, caminho, query, cabecalhos, corpo}` (query
percent-decodificada; headers em minúsculas; teto de corpo 1 MiB).
Sintético por request = todas as `fn` de topo + chamada (handlers sem
estado; `db_*` por request). 503 acima de `max_conexoes`; 0 = servir
para sempre. `println!("rhp: ouvindo em ...")` (porta efetiva; 0 = SO).

## Compatibilidade e migração

Aditivo. `exit` em handler = fim do servidor (documentado; igual ao
modelo de processo). Sem keep-alive/HTTP/2/TLS (Fase D planeja TLS).

## Segurança e performance

Sem `unsafe`, sem `Send` manual (tipos provam isolamento), sem deps.
500 nunca vaza detalhe (log só no stderr). Handler com loop infinito:
timeout de I/O não o interrompe (mesmo que `while true` local; documentar
como limite: `max-steps` não atravessa requests — cada request zera o
contador). Sem pool/compilação única (custo declarado; otimizar só com
protocolo antes/depois).

## Testes e validação

- Falha-antes: builtin inexistia; servidor real inexistente.
- Servidor real como filho (`max_requisicoes` determinístico, porta 0 do
  SO lida do stdout): GET/query, POST corpo, mapa 201 + header custom,
  500 sem vazar + servidor segue vivo, handler desconhecido, `exit(3)`
  termina com código 3.
- `cargo test --workspace`: 203 passed, 0 failed (199 antes + 4 novos).
- `cargo fmt --check` ok; `cargo clippy --workspace --all-targets
  -D warnings` ok; `tools/check-changes.sh origin/main` ok.

## Limitações e riscos conhecidos

Sem keep-alive, sem chunked dedicado, sem TLS, sem cookies/sessões
(C3), sem hot-reload com garantias, sem pool. `max-steps` por request.
`examples/` não tem demo de servidor (teste de exemplos exige exit 0).

## Referências

RFC 0002 (aceita e implementada). Sem ADR nova.
