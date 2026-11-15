# ROADMAP.md — Planejamento e estado (missao Next Generation: CP1–CP7)

## Auditoria e fortalecimento (2026-11-14, ramos `feat/engineering-audit` e `feat/next-generation`)

- [x] CHECKPOINT 1 (auditoria integral) e CHECKPOINT 1 (maturidade):
  `reports/AUDIT_MASTER.md`, `reports/NEXT_GENERATION_AUDIT.md`.
- [x] Correcoes P0/P1, P2 de nucleo, `VmLimits`, jaula/teto de imports.
- [x] Robustez: corpus malformado, property test, verificacao de exemplos.
- [x] Benchmarks + baseline (sem otimizacoes sem prova).
- [x] Docs, CLI (`check`), ADRs 0005–0010.
- [x] CP2 Diagnosticos: `rhp-diagnostic` (E0xxx–E3xxx + render).
- [x] CP3 Memoria: decisao por valor + dedup/interning + `reports/MEMORY_MODEL.md`.
- [x] CP4 Bytecode/VM: fluxo de pilha + `VmLimits` estendido + `builtins.rs`.
- [x] CP5 Modulos: ADR 0009 (semantica, sem codigo).
- [x] CP6 DevTools: `rhp check [--json]`, gramatica VSCode, desenho de debugger.
- [x] CP7 Seguranca: limites documentados/testados, sem sandbox ficticia.
- [x] CP8 Performance: harness + baseline; nenhuma otimizacao sem prova.
- [x] CP9 Documentacao: specs/ADRs/README/CHANGELOG consistentes.
- [ ] Pendente (requer aprovacao): RFC de tipos, exports/gerenciador de
  pacotes, PG/Redis com servidores reais, async (runtime), classes/generics,
  LSP, otimizacoes com protocolo antes/depois.

- [x] CHECKPOINT 1: `reports/AUDIT_MASTER.md` (1 P0, 5 P1, 13 P2, 10 P3, 7 P4).
- [x] CHECKPOINT 3: P0/P1 corrigidos (teto de profundidade 128, fmt roundtrip,
      `\u{...}`, teto HTTP 1 MiB, DNS documentado) + P2 de núcleo (params
      duplicados, `int()` estrito, `+` unário, jaula/topo de imports, teto de
      fonte, `VmLimits`/`--max-steps`, pins de spec).
- [x] Robustez: corpus malformado, property test determinístico, verificação
  de exemplos, `check-changes.sh` com renames.
- [x] Benchmarks std-only + baseline (`docs/BENCHMARKS.md`); sem otimizações
  (sem necessidade provada; `push` O(n²) é semântica documentada).
- [x] CHECKPOINT 2: README reestruturado + `docs/` + `CONTRIBUTING.md` +
  `SECURITY.md`.
- [ ] Pendente (requer aprovação/planejamento): P2 restantes de baixo risco,
  RFC de tipos, exports/gerenciador de pacotes, PG/Redis com servidores reais,
  async (runtime), classes/generics, LSP, otimizações com protocolo
  antes/depois.

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
