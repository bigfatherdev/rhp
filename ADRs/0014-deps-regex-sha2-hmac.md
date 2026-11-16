# ADR 0014 — Dependencias para regex e hashing geral (Fase 6)

- Data: 2026-11-16
- Estado: aceita (programa RHP-100; implementada: regex_* + sha256/hmac_*)
- Contexto: continuacao da politica da ADR 0013 (std-first; crates via ADR).
  Regex proprio foi descartado (ReDoS, incompatibilidade silenciosa com
  PCRE); cripto propria idem (erros silenciosos). Restava avaliar *quais*
  crates adotar para regex e hashing geral (nao-senha).

## Decisao

1. **Regex: crate `regex` (rust-lang-nursery).** Tempo linear garantido
   `O(m*n)` em todas as entradas (sem ReDoS por construcao); `1.4B+`
   downloads; mantida e atualizada (1.13.x, MSRV ~1.65); licencas MIT/Apache-2.0;
   sem dependencias C. Divergencia assumida e documentada: sem lookaround
   e sem backrefs (a crate *recusa compilar* esses padroes — erro claro,
   nao comportamento silencioso).
2. **Hashing geral: `sha2` + `hmac` (RustCrypto).** Puros Rust, `no_std`-capable;
   `verify_slice` em tempo constante (anti-timing); MSRV ~1.85. Hex manual
   via `format!` (sem crate `hex` — superficie minima).
3. **Anti-DoS em regex**: `RegexBuilder::size_limit(1 MiB)` em toda
   compilacao (padrao + usuario). Padroes gigantes sao rejeitados antes de
   executar; haystacks hostis sao seguros por construcao (tempo linear).
4. **API RHP (nomes proprios, sem `preg_*`):** `regex_match(s, padrao)`,
   `regex_replace(s, padrao, subst)` (grupos `$1`/`$name` da crate),
   `regex_split(s, padrao)`, `sha256(s)` (hex), `hmac_sha256(chave, msg)`
   (hex), `hmac_verify(chave, msg, tag)` (const-time, `Bool`).

## Alternativas consideradas

- `fancy-regex` (lookaround/backrefs via backtracking): rejeitada como
  padrao (nega a garantia linear; seria opt-in futuro no maximo).
- Crate `pcre2` (bindings C do PCRE2): rejeitada (dependencia C, contra a
  politica; reavaliar so com caso de compatibilidade comprovado).
- Motor regex proprio: rejeitado (ver auditoria §20 + discussao aprovada).
- `orion` para hashing/MAC: API boa, mas declara abertamente ausencia de
  auditoria de terceiros; rejeitada em favor de RustCrypto mainline.
- Cripto propria / `exec`: fora de escopo aqui (decisao mantida).

## Consequencias

- `Cargo.lock` passa a registrar: `regex`, `regex-syntax`, `regex-automata`,
  `aho-corasick`, `memchr` (arvore do `regex`); `sha2`, `hmac`, `digest`,
  `cpufeatures`/`block-buffer` ja presentes via `blake2`; demais
  transitivas auditadas no diff. Tudo Rust puro, sem C (verificar com
  `cargo tree -e normal` antes do merge).
- `cargo audit` executado nesta leva para as novas (gate: zero advisories
  nas novas; pre-existente `rustls-pemfile` segue registrado).
- Divergencias documentadas em `PHP_DIFFERENCES.md` (sem lookaround/backref;
  sem `preg_*`; `sha256`/`hmac_*` sem equivalente 1:1 no PHP — la e
  `hash('sha256', ...)`/`hash_hmac(...)`).
