# [Added] — Destructuring raso (RFC 0007, Fase 2)

- ID: 2026-11-16-feat-destructuring
- Data: 2026-11-16
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (`[`/`{` após `let`/`const` era erro de parse)

## Resumo
`let [a, b] = arr`, `let {x} = mapa`, `let {k: v} = mapa` (e formas `const`
que congelam todos): RHS avaliado uma vez em temporário oculto, erros
herdados do `Index`/`DeclareName` (sem `null` silencioso). Raso apenas.

## Motivação
Último item P3 do plano aprovado (Fase 2 do programa RHP-100): ergonomia
de dados sobre arrays/mapas por valor. Branch: `feat/php-100-fase2`.

## O que mudou
- Parser: `DestructureTarget::{Positional, ByKey}`, `Stmt::Destructure`
  (variante dedicada — `Stmt::Let` simples intacto), `parse_let`/`parse_const`
  estendidos (vazio/aninhado/placeholder = erro; `const` exige init, regra
  vigente), `describe` inalterado (sem tokens novos), docs, testes.
- Bytecode: braço `compile` (temp oculto `\0tmp:` + `LoadName`/`Index`/
  `DeclareName` por alvo + `declare_const`), `hidden_name`, testes
  (3 declares, const, erros).
- VM: nada (ops existentes).
- Fmt: `let [a, b] = ...` / `const {k: v, x} = ...` + golden/idempotência.
- CLI: renamer declara nomes (init no escopo externo), `check_stmt_expr_imports`
  cobre `init` + testes (e2e, rename).
- Specs: `LANGUAGE_SPEC.md` (gramática + semântica), `PHP_DIFFERENCES.md`
  (sintaxe sem `$`, raso, estrito vs `null` PHP), `ROADMAP.md`, exemplo
  `examples/destructure.rhp` (CI).

## Detalhes técnicos
- Alternativas rejeitadas na RFC 0007: estender `Stmt::Let` (churn em todos
  os matchs); `for [a,b] in` (futuro); `null` em ausente (RHP estrito);
  defaults/placeholders/rest no padrão (futuro ou fora).
- Arquivos: `crates/rhp-parser/src/lib.rs`, `crates/rhp-bytecode/src/lib.rs`,
  `crates/rhp-fmt/src/lib.rs`, `crates/rhp-cli/src/main.rs`,
  `RFCs/0007-destructuring.md` (+ specs, exemplo).

## Compatibilidade e migração
Aditivo. Nenhum programa válido muda. Nenhuma migração.

## Segurança e performance
- Sem `unsafe`; sem ops novos; validador cobre a sequência; temporário com
  NUL nunca colide.
- Um `Index` por alvo (mesmo custo do manual).

## Testes e validação
- `cargo fmt --check`: limpo. `cargo clippy --workspace --all-targets`:
  0 warnings/errors.
- `cargo test --workspace`: 251 passed, 0 failed (244 + 7 novos).
- Novos: parser (formas + 5 erros); bytecode (declares, const); VM (matriz
  + 4 erros + const-compilação); fmt (goldens, idempotência); CLI (e2e, rename).
- `examples/destructure.rhp` com exit 0 (CI).

## Limitações e riscos conhecidos
- Raso apenas; sem placeholder/default/rest/`for`-in (futuro documentado).

## Referências
- `RFCs/0007-destructuring.md` (aceita)
- Branch `feat/php-100-fase2`
