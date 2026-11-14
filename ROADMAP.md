# ROADMAP.md — Planejamento e estado (Fases 3–6 concluidas)

## Fase 1 — Fundacao (concluida em 2026-11-14)

- [x] Governanca, workspace, `hello.rhp`, lexer/parser/bytecode/VM/CLI.
- [x] Testes + `fmt`/`clippy`/`test` verdes; registro em `changes/`.

## Marco 2 — Nucleo imperativo (concluido em 2026-11-14)

- [x] Expressoes, variaveis, `if`/`else`, `while`, funcoes, `echo` geral.
- [x] ADR 0001; specs junto do codigo; compatibilidade Fase 1 preservada.

## Fase 3 — Stdlib nucleo (concluida em 2026-11-14)

- [x] Arrays/mapas (semantica por valor), indexacao e `a[i] = v` (1 nivel).
- [x] Builtins: `len push pop keys str int float type assert` (ADR 0002).

## Fase 4 — Modulos (concluida em 2026-11-14)

- [x] `import "x.rhp"` com DFS, dedup, ciclo/ausente como erro (ADR 0003).

## Fase 5 — Ferramentas (concluida em 2026-11-14)

- [x] `rhp-fmt` canonico idempotente + `rhp fmt [--check]`.
- [x] `tools/check-changes.sh` de CI (verificado em 4 cenarios reais).

## Fase 6 — Rede minima + fronteira (concluida em 2026-11-14)

- [x] `http_get` std-only verificado contra servidor local (ADR 0004).
- [x] Fronteira honesta: PostgreSQL/Redis, async/concorrencia,
      classes/generics, exports/privacidade, metodos — planejados, NAO
      implementados (exigem servidores de teste, runtime ou RFC de tipos).

Nada do item "planejado" deve ser assumido como existente.
