# RFC 0002 — Concorrência e servidor HTTP (`http_serve`)

- ID: RFC-0002
- Data: 2026-11-15
- Estado: proposta
- Contexto: Fase C do plano de sistemas; sem runtime de rede não há
  sites/sistemas. Pré-requisitos prontos: `try` (isolamento de falha por
  request), JSON (APIs), `rhp test` (contratos de handler).

## 1. Problema

Servir HTTP exige atender conexões em paralelo e isolar falhas por
request (um request inválido não derruba o servidor). A VM é
single-thread, valores usam `Rc` (não `Send`), e a linguagem não tem
closures/funções de primeira classe (handler não pode ser valor).

## 2. Objetivo e não-objetivos

Objetivo: `http_serve(porta, handler, opts)` std-only, thread-per-conexão
com `std::thread`, cada request isolado, erros virando 500 sem derrubar.
Não-objetivos: async/await ou executor próprio; keep-alive/HTTP/2;
WebSockets; TLS (Fase D); pool de conexões; hot-reload com garantias.

## 3. Decisão: modelo de concorrência

Threads do std, uma por conexão (`thread-per-connection`):

- Compartilhamento: **nada é compartilhado entre requests**. Cada request
  compila o programa a partir da AST (`Arc<Program>`: só dados owned,
  `Send+Sync`) **na própria worker thread**, executa em `Vm` própria e
  morre com ela. `Chunk`/`Rc` nunca cruzam threads (compilar-na-worker
  elimina a questão; sem `unsafe`, sem `Send` manual).
- Estado entre requests: não existe (handlers stateless). Recursos
  (`db_*`) abrem e fecham **por request**. Conexões de banco nunca cruzam
  threads (handles vivem na `Vm` que os criou).
- Limites: `max_conexoes` (acima = 503 imediato, sem spawn) e
  `max_requisicoes` (0 = infinito; N > 0 = serve N e retorna — âncora de
  testes determinísticos). Timeout de I/O 30 s por conexão.
- Custo: ~1 compilação + 1–2 threads por request (adequado p/ v0;
  documentado; pool/compilação única ficam para otimização com protocolo
  antes/depois, proibida sem prova).

## 4. Proposta: `http_serve(porta, nome_handler, opts)`

Aridade 3 (`opts` obrigatório; `{}` = padrões):

```rhp
http_serve(8080, "app", {});
http_serve(8080, "app", {"max_conexoes": 64, "max_requisicoes": 100});
```

- `porta`: `Int` 1–65535. `nome_handler`: `Str` com nome de `fn` global
  de 1 parâmetro (`fn app(req)`). `opts`: mapa.
- Por request: parse (método, caminho limpo, `query` decodificada,
  `cabecalhos` em minúsculas, `corpo` UTF-8 ou 500) → `req` (mapa) →
  handler executado como `app(req)` num programa sintético
  ([todas as `fn` de topo..., `app(req)`]) → retorno:
  `Str` = corpo 200 `text/plain`; mapa `{status?, cabecalhos?, corpo?}`
  (padrões 200/`text/plain; charset=utf-8`/erro se ausente tipo errado).
- Falha no handler (throw/erro): 500 `erro interno` (sem vazar detalhe),
  log em stderr, servidor segue. `exit(codigo)` no handler: encerra o
  SERVIDOR com o código (semântica de `exit` preservada; documentado).
- Sem `main` implícito: o programa que chama `http_serve` continua dono
  do processo; o builtin bloqueia servindo.

## 5. Alternativas consideradas

- Executor async próprio: runtime novo inteiro + sintaxe; rejeitado (G+
  sem necessidade provada; threads atendem o v0).
- Handler como valor/closure: exige primeira-classe (planejado, não
  existe); string com nome resolve sem mudar a linguagem. Revisitar
  quando houver primeira-classe.
- Recompilar 1× e compartilhar `Chunk` com `Arc<Mutex>`: `Rc` interno
  impede `Send`; `Mutex` serializaria tudo. Rejeitado (inseguro/inútil).
- Arquivo-handler por request (modelo PHP-FPM literal): re-lê disco por
  request e perde o programa chamador (config, `let`s); nome-de-função
  mantém um programa só. Rejeitado para o v0.

## 6. Compatibilidade

Aditivo (1 builtin novo). Nenhum comportamento existente muda.
`max_requisicoes: 0` = servir para sempre (cuidado em exemplos: nunca
rodar servidor infinito em `examples/`, que o teste executa).

## 7. Segurança

- Sem `unsafe`; sem deps; sem `Send` manual (tipos provam isolamento).
- Parser HTTP tolerante só no necessário: linha de status rígida,
  Content-Length limitado ao teto de corpo (1 MiB), resto descartado;
  header injection impossível na resposta (só valores do handler, sem
  `\r\n` — validar e/ou sanear: **validar, erro 500 se inválido**).
- Erros internos nunca vazam no corpo do 500. `trust` do PG continua
  restrito a rede local/testes.
- DoS: `max_conexoes` + timeouts + teto de corpo; sem keep-alive (fecha
  sempre — mitiga slow-loris básico, não avançado: declarado).

## 8. Performance

Baseline com `bench_rhp` + `ab`-like manual antes/depois (harness
existente). Sem otimização nesta etapa; latência dominada por
compilação-por-request (declarado; pool futuro exige protocolo).

## 9. Plano de implementação (C2)

1. `Vm::run_value` + `set_program(Arc<Program>)` (aditivos).
2. `rhp-vm/src/http_server.rs`: parse de request, loop accept,
   worker (compila sintético, chama handler, responde), 500s, limites.
3. Builtin `HttpServe` (id 48, aridade 3) + validação de args.
4. Testes contra o próprio servidor (processo filho + `max_requisicoes`;
   sem mock de socket).
5. Specs + `CLI_REFERENCE` + exemplo fora de `examples/` (bloquearia o
  teste de exemplos) + gramática.

## 10. Plano de testes

- Unidade: parse de request (método/caminho/query/headers/corpo,
  malformados), `resolve` não se aplica (sem redirects aqui).
- Integração (servidor real = o próprio `rhp`): GET simples, POST com
  corpo, query decodificada, header custom ecoado, mapa de resposta
  (status/headers), erro no handler => 500 sem vazar, `max_requisicoes`
  determinístico, handler inexistente = erro claro na chamada.
- `cargo test --workspace` verde; sem flakiness (sem sleeps longos).

## 11. Migração e rollout

Nenhuma (aditivo). Rollout via PR da Fase C após gates.

## 12. Critérios de aceitação

- [ ] Todos os testes da §10 passam, inclusive contra servidor real.
- [ ] `fmt`/`clippy`/`test` + `check-changes.sh` verdes.
- [ ] Specs atualizadas; exemplo documentado (fora de `examples/`).
- [ ] Esta RFC marcada `aceita` antes do merge da implementação.
