# ADR 0012 — TLS para HTTPS (cliente primeiro, servidor depois)

- Data: 2026-11-15
- Estado: aceita (autorização do plano; implementada: cliente)
- Contexto: Fase D do plano de sistemas. `http_fetch` só fala `http://`;
  APIs reais exigem `https://`. ADR 0011 adiou TLS para esta fase.

## Decisão

1. **Cliente TLS via `rustls` (puro Rust) + `webpki-roots`** (raízes Mozilla
   embutidas, determinístico, sem depender do SO). Primeira dependência
   externa de crates do projeto — justificada abaixo; `cargo tree` passa
   a incluir `rustls`/`webpki-roots` (+ transitivas) só para TLS.
2. **Escopo v0 = cliente** (`https://` no `http_fetch`, upgrades
   `http→https` seguidos, `https→http` recusado). Sem `unsafe` novo
   (todo no `http_server.rs`/`builtins.rs` existente, via APIs seguras).
3. **Servidor TLS adiado** (requer gestão de certificados: ACME/self-signed,
   rotação, SNI). Registrar como pendente explícito, não como promessa.

## Alternativas consideradas

- `native-tls` (OpenSSL/SChannel do SO): menos código, mas FFI de sistema
  (varia por SO, `libssl` ausente = binário quebrado); rejeitada (mesmo
  motivo do FFI manual só ter valido para `libsqlite3`: onipresença —
  libssl não é onipresente nem estável entre distros).
- Implementar TLS à mão: proibido pela governança (cripto própria);
  rejeitada sem discussão.
- Ignorar HTTPS: inviabiliza APIs reais; rejeitada (é o motivo da Fase D).
- `hyper`/`reqwest`: runtime async + árvore de deps muito maior;
  rejeitada (nossa pilha é `TcpStream` std + protocolo próprio).

## Consequências

- `Cargo.lock` passa a registrar as crates TLS (revisar diff no review).
- Build fica ~1–2 min mais lento (compilação do `ring`).
- `https://` com hostname não-DNS (IP literal) funciona via `IpAddress`
  no SNI; IDNA/punycode fora do escopo (erro claro).
- Sem verificação customizável de certificado nesta fase (só roots
  embutidas; `SSLKEYLOGFILE`/interceptação não suportada — documentado).

## Plano de testes

- Servidor TLS **local e real** (implementação TLS do Python/`openssl`,
  não mock nosso): setup do teste gera certificado autoassinado
  (`openssl req -x509`, SAN `IP:127.0.0.1`) e sobe `http.server` com
  `ssl`; o cliente RHP valida contra o cert via `RHP_CA_BUNDLE` (bundle
  PEM extra, somado às raízes embutidas). Exercita handshake, SNI,
  verificação de cadeia/hostname e HTTP sobre TLS de verdade.
- `RHP_CA_BUNDLE` é feature legítima (proxies corporativos com MITM),
  não gambiarra de teste: documentada em `docs/CLI_REFERENCE.md`.
- Casos: GET 200 via `https://127.0.0.1:PORTA`, redirect http→https,
  downgrade https→http recusado, hostname divergente rejeitado.
- Pré-requisitos do teste (como o Docker do PG): `python3` com `ssl` +
  `openssl` CLI. Ausentes = falha explícita pedindo o ambiente.

## Revisão de dependências (governança §12)

- `rustls 0.23` (Apache2/ISC/MIT, mantida, auditorias públicas) +
  `webpki-roots` (dados, MPL-2.0) — sem `unsafe` nosso; `ring` (C/asm
  auditado, build via `cc`).
- Alternativa `aws-lc-rs` exigiria `cmake`; `ring` usa só `cc`.
