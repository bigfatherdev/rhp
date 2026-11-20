# [Added] — Corpo `chunked`, `localhost` e reasons HTTP

- ID: 2026-11-20-feat-http-chunked-localhost
- Data: 2026-11-20
- Versão: 0.6.0
- Tipo: Added
- Compatibilidade: Compatível

## Resumo

Requests com `Transfer-Encoding: chunked` passam a funcionar (corpo
remontado e entregue ao handler); `"localhost"` virou apelido de
`127.0.0.1` no `host`; `reason()` cobre 408/413/422/502/504.

## Motivação

" Cuidar das limitações" da 0.6.0: `chunked` era a limitação com maior
impacto prático (clientes e proxies reais usam; antes degradava em erro
ou corpo errado) e `localhost` rejeitado era atrito sem valor de
segurança (não envolve DNS — resolve para o loopback).

## O que mudou

- `LeitorCorpo::ler_chunked` (RFC 9112 §7.1): tamanhos hex, extensões
  ignoradas após `;`, trailers descartados (teto 32 linhas), tudo sob
  `MAX_BODY` + prazo total; `chunked` precede `Content-Length` (§6.3).
  Malformado/truncado = 400 (sem exceção nova).
- `resolve_host`: `"localhost"` (case-insensitive) → `127.0.0.1`;
  outros nomes seguem rejeitados.
- `reason()`: 408, 413, 422, 502, 504 (antes: `Desconhecido`).

## Detalhes técnicos

Arquivo: `crates/rhp-vm/src/http_server.rs` (`LeitorCorpo`, `resolve_host`,
`reason`). Buffer compactado após 64 KiB consumidos (sem crescimento
ilimitado). Sem `unsafe`; sem dependências. Docs: `docs/WEB.md`,
`docs/REFERENCIA.md`.

## Compatibilidade e migração

`chunked` antes = 400; agora funciona (clientes corretos só ganham).
`localhost` antes = erro; agora escuta local. Migração: não necessária.

## Segurança e performance

Mesmos tetos do corpo fixo (1 MiB, prazo, linha 16 KiB). Custo por
request chunked: O(n) com 1 passada, sem alocação extra além do corpo.

## Testes e validação

- Integração `400s_e_sobrevive`: chunked válido ecoa `ola, mundo!`;
  tamanho inválido e truncado = 400; servidor segue.
- Integração `rejeita_host_invalido`: `localhost`/`LOCALHOST` anunciam `127.0.0.1` + 200.
- `cargo test --workspace`: ver suíte final; fmt/clippy limpos.

## Limitações e riscos conhecidos

Sem trailers repassados ao handler (descartados); `Content-Length` +
`chunked` simultâneos resolvem para `chunked` (sem erro — documentado);
sem keep-alive; corpo segue `Str` UTF-8.

## Referências

- Missão 0.6.0 §5; RFC 9112 §§6.3/7.1; branch `feat/rhp-0.6.0`.
