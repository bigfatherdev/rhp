# [Added] — HTTPS: `https://` no `http_fetch` via rustls (ADR 0012)

- ID: 2026-11-15-feat-https
- Data: 2026-11-15
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (`http://` byte-idêntico; `https://`
  antes era erro, agora funciona; redirect http→https passa a seguir)

## Resumo

`http_fetch` aceita `https://`: handshake + verificação de cadeia,
hostname e SNI via `rustls` puro + `webpki-roots`, com CA extra opcional
via `RHP_CA_BUNDLE` (PEM). Downgrade `https→http` recusado.

## Motivação

Fase D1/D2: sem HTTPS não há API real (a web inteira redireciona para
TLS). Primeira dependência externa de crates do projeto, conforme ADR.

## O que mudou

- `rhp-vm/Cargo.toml`: `rustls 0.23` (sem default, `ring`+`std`+`tls12`),
  `webpki-roots 0.26`, `rustls-pemfile 2`; `Cargo.lock` atualizado.
- `rhp-vm/src/tls.rs` (novo): `Transport::{Plain,Tls}`, config com
  raízes embutidas + bundle, SNI/hostname (DNS ou IP).
- `builtins.rs::do_request`: parse de esquema, porta padrão 443, upgrade
  permitido; leitura por framing (`Content-Length` manda; sem ele, EOF
  limpo — necessário no TLS, onde close sem `close_notify` é erro).
- `resolve_location`: upgrade seguido; downgrade recusado com erro claro.
- `http_get` preservado byte a byte (pré-checagem + wrapper no mesmo
  caminho de leitura).
- Diagnóstico `E3011` passa a cobrir `http_fetch`.
- Specs: `LANGUAGE_SPEC.md` (§6), `ADR 0012`, exemplo de teste e2e.

## Detalhes técnicos

Sem `unsafe` novo; sem `Send` manual. Timeout de I/O vale para o
handshake. SNI automático. Sem ALPN (HTTP/1.0). `RHP_CA_BUNDLE` lido por
chamada (simples; documentado como v0).

## Compatibilidade e migração

`http://` inalterado. URLs `https://` que antes falhavam agora
funcionam (mudança pretendida da fase). Servidor TLS segue fora de
escopo (documentado).

## Segurança e performance

Verificação completa por padrão (cadeia + hostname + SNI); sem
`dangerously_*` em produção. Build +~1–2 min (ring). Leitura para no
`Content-Length` (sem esperar close; teto 1 MiB mantido).

## Testes e validação

- Servidor TLS local real (Python stdlib, PKI própria CA+servidor com
  SAN IP, validada via `RHP_CA_BUNDLE`): 200 OK, redirect interno,
  upgrade http→https, downgrade recusado, CA errada rejeitada.
- `cargo test --workspace`: 216 passed, 0 failed (servidor TLS local,
  redirect interno, upgrade, downgrade recusado, CA errada rejeitada;
  mais pacotes/build/rate-limit/sessao/template).
- `cargo fmt --check` ok; `cargo clippy --workspace --all-targets
  -D warnings` ok; `tools/check-changes.sh origin/main` ok.

## Limitações e riscos conhecidos

Sem `ServerName` internacionalizado (ASCII/IP). Sem client-cert auth.
Sem servidor TLS (Fase D planeja; exige gestão de certificados). Sem
cache de sessão TLS entre requests (handshake por request; aceito no v0).

## Referências

ADR 0012. `docs/EXEMPLO_SERVIDOR.md` continua HTTP-only (TLS terminaria
num proxy; documentar quando houver servidor TLS).
