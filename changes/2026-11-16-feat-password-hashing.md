# [Added] — Hashing de senhas: `hash_password`/`verify_password` (RFC 0013, Fase 5)

- ID: 2026-11-16-feat-password-hashing
- Data: 2026-11-16
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (2 builtins novos, ids 56–57)

## Resumo
`hash_password(senha)` (Argon2id, PHC `$argon2id$v=19$...`) e
`verify_password(senha, hash)` (`Bool`; aceita `$argon2id$` e bcrypt
`$2a$`/`$2b$`/`$2x$`/`$2y$` — migração de bancos PHP legados).

## Motivação
Bloqueador de auth web: sem isso, nenhum sistema com login. Primeira
aplicação da política de dependências (ADR 0013): usar o que o Rust usa
(`argon2` + `bcrypt`, puros Rust, sem C) em vez de implementar cripto.

## O que mudou
- Bytecode: `Builtin::{HashPassword, VerifyPassword}` (ids 56/57), nomes,
  aridades (1/2), anti-deriva.
- VM: `Builtin::HashPassword` (Argon2id, salt aleatorio via `getrandom`),
  `Builtin::VerifyPassword` (dispatch por prefixo; DoS bounds `m<=65536KiB,
  t<=16, p<=4` antes de alocar; `is_ok` => `Bool`; malformado/desconhecido
  = erro), `check_argon2_bounds`.
- Docs: `LANGUAGE_SPEC.md` (tabela), `BYTECODE_SPEC.md` (ids 56–57),
  `PHP_DIFFERENCES.md` (nomes proprios, `false` vs erro, teto anti-DoS,
  truncamento 72 bytes), `ROADMAP.md`, `SECURITY.md` (`rand` nao e CSPRNG),
  exemplo `examples/auth.rhp` (CI), ADR-0013.
- Dependências: `argon2 0.6.0`, `bcrypt 0.19.3`, `password-hash 0.6.1`
  (+ transitivas puras; `cargo tree` auditado, sem C).

## Detalhes técnicos
- DoS bounds verificados antes de qualquer hashing (teste dedicado).
- `verify` usa os parametros do proprio hash (`Argon2::default` como base).
- Alternativas na ADR 0013: `orion` (sem auditoria declarada), cripto
  propria (rejeitada), scrypt/pbkdf2 agora (sem caso de uso).
- Arquivos: `crates/rhp-bytecode/src/lib.rs`, `crates/rhp-vm/src/builtins.rs`,
  `crates/rhp-vm/Cargo.toml`, specs, exemplo, ADR (`ADRs/0013-deps-hashing-senhas.md`),
  `Cargo.lock`.

## Compatibilidade e migração
Aditivo. Hashes `$2y$` existentes verificam sem mudança (teste dedicado).

## Segurança e performance
- Sem `unsafe`; sem C; comparação em tempo constante (`verify_slice`
  interno das crates); salt aleatorio por hash; zeroizacao interna.
- Custo Argon2 (~0,2–0,5 s/hash nos defaults) fora do `max_steps`
  (documentado; mesma classe do `sleep`).
- Sem auditoria externa dedicada das crates (declarado na ADR; mitigações:
  crates mainline amplamente usadas, pinning, `cargo audit 0.23.0`
  executado nesta leva: exit 0 para as 21 novas, zero advisories; unico
  achado do workspace e pre-existente — `rustls-pemfile 2.2.0`
  RUSTSEC-2025-0134 (unmaintained, da ADR 0012, fora de escopo aqui).

## Testes e validação
- `cargo fmt --check`: limpo. `cargo clippy --workspace --all-targets`:
  0 warnings/errors.
- `cargo test --workspace`: 283 passed, 0 failed (282 + 1 novo com 8 asserts).
- Novos: `hashes_and_verifies_passwords` (PHC round-trip + formato,
  vetores independentes $2a$-online-tool e $2b$-Python, `$2y$`
  relabelado, malformado/desconhecido/teto/tipos).
- `examples/auth.rhp` com exit 0 (CI).

## Limitações e riscos conhecidos
- Sem `sCrypt`/PBKDF2-verify (extensao documentada via `password-hash` traits).
- Sem `random_bytes`/CSPRNG publica (futuro); `rand()` segue nao-criptografico.
- Truncamento bcrypt de 72 bytes herdado.

## Referências
- `ADRs/0013-deps-hashing-senhas.md` (decisao + avaliacao de crates)
- Branch `feat/php-100-fase5-auth`
