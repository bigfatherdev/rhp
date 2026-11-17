# [Added] — Trace de execução `--trace` + servidor LSP mínimo (Etapa 4, DevX)

- ID: 2026-11-16-feat-trace-lsp
- Data: 2026-11-16
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (flags/comandos novos; padrao desligado)

## Resumo

- `rhp run --trace`: `Vm::set_trace` despeja `f<frame>:<ip> <op>` em
  stderr a cada passo; stdout do programa intacta.
- `rhp lsp`: servidor LSP sobre stdio (JSON-RPC + `Content-Length`),
  `initialize`/`didOpen`/`didChange`/`shutdown`/`exit`,
  `publishDiagnostics` com erros lexicos/sintaticos (faixa de 1 char).
  Parser JSON proprio, std-only (sem `serde`).

## Motivação

Etapa 4, itens "debugger/LSP": tracer cobre depuracao dinamica leve;
LSP cobre diagnostics no editor. Escopo honesto v1 (sem DAP/breakpoints;
LSP so diagnosticos).

## O que mudou

- VM: campo `trace: bool` + `set_trace`; 1 `eprintln!` por passo.
- CLI: `--trace` (exclusivo com tetos, mesma limitacao do parser de
  flags); `lsp.rs` (~700 linhas: `Json`, parser, framing, `diagnose`,
  `run`, 4 testes incl. sessao scriptada fim a fim + smoke real).
- `docs/CLI_REFERENCE.md` (secoes `--trace`, `rhp lsp`).

## Compatibilidade e migração

Novos comandos/flags; sem migração. `trace` desligado por padrao
(zero custo: 1 branch).

## Segurança e performance

Sem `unsafe`; JSON proprio recursivo em profundidade (entradas vêm de
editores confiaveis; teto de profundidade = follow-up documentado). Trace em stderr
(nao polui stdout do programa).

## Testes e validação

- `rhp_run_trace_prints_ops_to_stderr` (CLI) + 4 testes LSP (parse,
  round-trip de escape, diagnosticos, sessao scriptada).
- Smoke real via pipe (`initialize` + `didOpen` quebrado => resposta
  com `result` + `publishDiagnostics`).
- Suite final 321/0; fmt/clippy limpos.

## Limitações e riscos conhecidos

- Sem breakpoints/step/debugger DAP (futuro; registrado).
- LSP: so diagnosticos (sem completion/hover); stdin/stdout puros.
- Parser JSON recursivo em profundidade (sem teto; v1).

## Referências

- `docs/CLI_REFERENCE.md`; `docs/DEBUGGER_DESIGN.md` (desenho previo,
  inalterado).
