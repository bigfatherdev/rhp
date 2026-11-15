# CHANGELOG.md — Resumo consolidado por release

Formato inspirado em Keep a Changelog. Detalhes auditaveis ficam em `changes/`.

## [unreleased]

### Added

- Primeira fatia vertical do RHP: `hello.rhp`, lexer, parser/AST, geracao de
  bytecode, VM em pilha e CLI `rhp run hello.rhp` (saida `Hello from RHP!`).
  Registro: `changes/2026-11-14-feat-fase-1-fatia-vertical.md`.
- Marco 2 (nucleo imperativo, compativel): expressoes (int/float/bool/null,
  aritmetica checada, comparacoes), `let`/atribuicao, blocos/escopos,
  `if`/`else`, `while`, `fn`/`return`/chamadas com recursao limitada (512),
  `echo` de qualquer valor, comentarios `//` e exemplo `examples/fib.rhp`.
  Registro: `changes/2026-11-14-feat-marco-2-nucleo-imperativo.md`.
- Fase 3 (stdlib nucleo): arrays/mapas por valor, indexacao e `a[i] = v`,
  builtins `len push pop keys str int float type assert` (ADR 0002).
  Registro: `changes/2026-11-14-feat-fase-3-stdlib-nucleo.md`.
- Fase 4 (modulos): `import "x.rhp"` com DFS/dedup/ciclo como erro (ADR 0003).
  Registro: `changes/2026-11-14-feat-fase-4-modulos.md`.
- Fase 5 (ferramentas): crate `rhp-fmt` + `rhp fmt [--check]` idempotente e
  `tools/check-changes.sh` de CI (ADR 0003).
  Registro: `changes/2026-11-14-feat-fase-5-ferramentas.md`.
- Escape `\u{HEX}` em strings.
  Registro: `changes/2026-11-14-feat-escape-unicode.md`.
- Fase 6 (rede minima): builtin `http_get` std-only (so `http://`, 5s, 2xx)
  verificado contra servidor local; fronteira honesta para PostgreSQL/Redis,
  async, classes/generics (ADR 0004, planejados e NAO implementados).
  Registro: `changes/2026-11-14-feat-fase-6-rede-minima.md`.

### Fixed

- Teto de profundidade sintatica (128 niveis) e de constantes: entradas
  profundamente aninhadas geram erro em vez de abortar o processo.
  Registros: `changes/2026-11-14-fix-limite-profundidade.md`.
- `rhp fmt` com roundtrip garantido: floats sem notacao cientifica e chaves
  de mapa citadas quando necessario.
  Registro: `changes/2026-11-14-fix-fmt-roundtrip.md`.
- `http_get` com teto de corpo de 1 MiB e DNS documentado.
  Registro: `changes/2026-11-14-fix-http-limites.md`.
