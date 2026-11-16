# ADR 0013 — Política de dependências externas + hashing de senhas

- Data: 2026-11-16
- Estado: aceita (autorização do programa RHP-100; implementada: `hash_password`/`verify_password`)
- Contexto: o projeto operava sob postura std-only (ADR 0002), com exceções
  pontuais via ADR (`libsqlite3` na ADR 0011, `rustls`/`ring` na ADR 0012).
  RHP precisa de hashing de senhas (bloqueador de auth web), e a forma
  correta em Rust são crates estabelecidas — o que exige formalizar o rito
  em vez de abrir uma exceção ad hoc por vez.

## Decisão

1. **Política (adendo, não revogação):** std-first continua o padrão.
   Crate externa exige, em ADR própria ou seção dedicada: necessidade real
   demonstrada; preferência por puro Rust (sem C/`unsafe` transitivo salvo
   justificativa como a do `ring`); manutenção ativa; licença MIT/Apache-2.0;
   sem vulnerabilidades conhecidas sem patch; MSRV compatível; versões
   pinadas; `cargo audit`/`deny` no CI. Esta ADR revoga pontualmente a
   cláusula std-only da ADR 0002 **apenas** para os casos aqui avaliados.
2. **`hash_password(senha) -> Str`** (id 56): Argon2id via crate `argon2`
   (defaults OWASP: m=19MiB, t=2, p=1), saída em formato PHC
   (`$argon2id$v=19$...`). Nome próprio do RHP (não `password_hash` do PHP:
   sem promessa de agility de algoritmo).
3. **`verify_password(senha, hash) -> Bool`** (id 57): `$2a$`/`$2b$`/`$2x$`/`$2y$`
   via crate `bcrypt` (verifica hashes `$2y$` gerados pelo PHP — migração de
   bancos legados); `$argon2id$` via `argon2`. Prefixo desconhecido = erro
   (extensível depois, ex.: scrypt/pbkdf2 via `password-hash` traits).
4. **Semântica RHP**: senha incorreta = `false`; hash malformado = erro
   (explícito, em vez do `false` silencioso do PHP). Truncamento de 72 bytes
   do bcrypt herdado e documentado. Init sem efeito colateral observável.
5. **Limites anti-DoS na verificação**: parâmetros vêm do próprio hash
   (atacante controla); antes de verificar, validar tetos
   (memória ≤ 64 MiB, iterações ≤ 16). Sem isso, hash malicioso esgota
   memoria (advertencia documentada do `orion`).
6. **CSPRNG fora do escopo**: `rand()` segue nao-criptografico (documentado);
   o salt usa `getrandom` interno das crates. `random_bytes` dedicado e
   `srand` ficam para proposta futura.

## Alternativas consideradas

- Manter std-only e nao ter hashing: perpetua o bloqueio de auth web;
  rejeitada (autorizacao do programa cobriu este track).
- Implementar bcrypt/argon2 proprios: proibido (cripto propria —
  erros silenciosos de timing/padding/nonce que testes nao pegam);
  rejeitada sem discussao.
- `orion` (API mais simples, pure Rust): declara abertamente "sem
  auditoria de terceiros, uso por sua conta"; rejeitada em favor das
  crates RustCrypto mainline (+ `blake3` oficial quando preciso).
- Adotar `password_hash`/`password_verify` como nomes: rejeitado —
  prometeria semantica PHP (constantes `PASSWORD_*`, agility) que nao
  entregamos; nomes proprios evitam falsa paridade.
- scrypt/pbkdf2 nesta leva: sem caso de uso (PHP nunca os produziu como
  hash de senha); ponto de extensao documentado via `password-hash` traits.

## Consequencias

- `Cargo.lock` passa a registrar 21 pacotes novos, todos Rust puro sem C
  nem build scripts (verificado via `cargo tree -e normal`; o unico `ring`
  na arvore e o pre-existente do `rustls`, ADR 0012, intocado):
  `argon2 0.6.0`, `bcrypt 0.19.3`, `password-hash 0.6.1` (diretas) +
  `base64`, `base64ct`, `blake2`, `block-buffer`, `blowfish`, `byteorder`,
  `cipher`, `cmov`, `cpufeatures`, `crypto-common`, `ctutils`, `digest`,
  `getrandom`, `hybrid-array`, `inout`, `phc`, `r-efi`, `typenum`
  (transitivas; `subtle`/`zeroize` ja estavam na arvore via `rustls`).
- `cargo audit 0.23.0` executado nesta leva (sem CI config no repo; roda local): exit 0, zero advisories nas 21 novas. Unico achado do workspace: `rustls-pemfile 2.2.0` RUSTSEC-2025-0134 (unmaintained, pre-existente da ADR 0012, fora de escopo; registrar follow-up).
- `verify_password` aceita `$argon2id$` e `$2a$`/`$2b$`/`$2x$`/`$2y$`;
  qualquer outro prefixo = erro claro (lista fechada, extensivel).
- Documentar em `SECURITY.md`: `rand()` segue nao-criptografico.
