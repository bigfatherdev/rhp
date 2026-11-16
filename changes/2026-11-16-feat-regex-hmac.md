# [Added] — Regex, hashing e HMAC (RFC 0014, Fase 6)

- ID: 2026-11-16-feat-regex-hmac
- Data: 2026-11-16
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (6 builtins novos, ids 58–63)

## Resumo
`regex_match`/`regex_replace`/`regex_split` (sem lookaround/backrefs;
tempo linear garantido), `sha256` (hex) e `hmac_sha256`/`hmac_verify`
(const-time). Tudo Rust puro, sem C.

## Motivação
Segunda aplicacao da politica de dependencias (ADR 0013, estendida pela
ADR 0014): usar crates Rust estabelecidas em vez de implementar motor
regex ou cripto propria.

## O que mudou
- Bytecode: `Builtin::{RegexMatch, RegexReplace, RegexSplit, Sha256,
  HmacSha256, HmacVerify}` (ids 58–63), nomes, aridades (1/2/3),
  anti-deriva.
- VM: `compile_regex` (teto 1 MiB via `RegexBuilder::size_limit`),
  `hex_encode`/`hex_decode`, 6 bracos em `call_builtin`; `verify` usa
  comparacao em tempo constante (`verify_slice`).
- Docs: `LANGUAGE_SPEC.md` (tabela), `BYTECODE_SPEC.md` (ids 0–63),
  `PHP_DIFFERENCES.md` (sem lookaround/backrefs, sem promessa 1:1),
  `ROADMAP.md`, exemplo `examples/regex_hmac.rhp` (CI), ADR-0014.
- Dependencias: `regex 1.13.1` (+ `regex-syntax`, `regex-automata`,
  `aho-corasick`, `memchr`), `sha2 0.11` (+ `digest`, `block-buffer`,
  `cpufeatures`), `hmac 0.13` (todas puras Rust, auditadas via
  `cargo audit`: zero advisories nas novas).

## Detalhes tecnicos
- Regex sem lookaround/backrefs: erro de compilacao claro (nao
  silencioso), por desenho da crate (garantia de tempo linear).
- `hmac_verify` compara em tempo constante; tag malformada = erro.
- Alternativas rejeitadas na ADR 0014: `fancy-regex` (backtracking),
  crate `pcre2` (dependencia C), motor proprio, `orion`.

## Compatibilidade e migracao
Aditivo. Nenhuma mudanca em comportamento existente.

## Seguranca e performance
- Sem `unsafe`; sem C; teto anti-DoS na compilacao de regex.
- `sha256`/`hmac` O(n); sem bench dedicado (custo linear documentado).

## Testes e validacao
- `cargo fmt --check`: limpo. `cargo clippy --workspace --all-targets`:
  0 warnings/errors.
- `cargo test --workspace`: 284 passed, 0 failed (283 baseline + 1 novo teste `runs_regex_and_hashing_builtins`, 12 asserts).
- Novos: `runs_regex_and_hashing_builtins` (match true/false, invalido,
  replace com grupos, split, vetor canonico sha256("abc"), hmac
  round-trip + mismatch + hex invalido, erros de tipo).
- `examples/regex_hmac.rhp` com exit 0 (CI).

## Limitacoes e riscos conhecidos
- Sem lookaround/backrefs (limite da crate, documentado).
- Sem `random_bytes`/CSPRNG publica (fora desta leva).

## Referencias
- `ADRs/0014-deps-regex-sha2-hmac.md` (aceita; estende a politica da ADR 0013)
- Branch `feat/php-100-fase6-ecossistema`
