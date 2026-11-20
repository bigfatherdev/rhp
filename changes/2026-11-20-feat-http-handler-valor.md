# [Added] — Handler como valor + limites do handler + anti-reentrada

- ID: 2026-11-20-feat-http-handler-valor
- Data: 2026-11-20
- Versão: 0.6.0
- Tipo: Added
- Compatibilidade: Compatível

## Resumo

`http_serve(8080, handler, {})` (função como valor) funciona: resolve pelo
nome no `fn` de nível superior (equivale à string). Closures/aninhadas dão
erro claro. Novos opts `max_passos` (default 10M) e `max_heap_bytes`
(default 0 = sem limite) impedem handler preso de segurar worker para
sempre. `http_serve` dentro do handler vira erro (500) em vez de hang.

## Motivação

Missão 0.6.0 §6 (divergência doc-vs-runtime: docs sugeriam handler inline,
runtime exigia string) e §5 (handler em loop eterno ocupava a thread para
sempre — `VmLimits::default()` é ilimitado e o worker usa default;
`exit`/hang sem resposta travavam a conexão até timeout do cliente).

## O que mudou

- `serve_builtin`: aceita `Value::Func` cujo nome exista como `FnDef` no
  topo; demais valores de função = erro explicando a limitação.
- `ServeOpts.max_passos`/`max_heap_bytes` aplicados via `vm.set_limits`
  por request; estouro = 500 opaco + servidor segue.
- Guarda RAII `EM_HANDLER` (thread-local): `http_serve` na worker vira
  erro imediato (cobre chamada direta e via replay do topo).
- Erros novos → E3000 (reword para não tropeçar no tripwire `exige` do E3001).

## Detalhes técnicos

Arquivo: `crates/rhp-vm/src/http_server.rs` (`serve_builtin`, `ServeOpts`,
`parse_serve_opts`, `serve`, `handle_one`, `run_handler`, `GuardaHandler`).
Sem `unsafe`; sem dependências. Docs: `docs/WEB.md`, `docs/REFERENCIA.md`
(linha do `http_serve` não promete mais closure inline).

## Compatibilidade e migração

String-handler inalterado. Valor-de-função do topo passa a funcionar
(antes: erro). Observável: loop eterno no handler agora = 500 após
`max_passos` (antes: hang); `http_serve` aninhado agora = 500 (antes:
hang). `max_passos: 0` restaura ilimitado. Migração: não necessária.

## Segurança e performance

Worker limitado por construção; sem thread nova por request além do
modelo existente. Overhead por request: 2 campos copiados + 1 flag.

## Testes e validação

- Integração: `handler_como_valor` (valor ok; closure → erro `nivel superior`), `handler_loop_limitado` (500 + servidor vivo), `aninhado_rejeitado` (2× 500 + saída normal), `conexoes_concorrentes` (8× 200).
- `cargo test --workspace`: 354 passed, 0 failed; fmt/clippy limpos.

## Limitações e riscos conhecidos

Closures com captura não suportadas como handler (ambiente não cruza o
replay do programa sintetizado); `max_stack_depth` do handler segue
ilimitado (recursão tem teto próprio de 512 frames).

## Referências

- Missão 0.6.0 §§5–6; RFC 0003 (funções de primeira classe); branch `feat/rhp-0.6.0`.
