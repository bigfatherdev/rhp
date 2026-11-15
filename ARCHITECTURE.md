# ARCHITECTURE.md — Arquitetura do RHP (Fases 3–6)

Estado real: implementado e validado em 2026-11-14. Nada aqui descreve
funcionalidade futura como pronta. Desenho: `ADRs/0001-marco-2.md` (nucleo),
`0002` (stdlib), `0003` (modulos/ferramentas), `0004` (rede/fronteira).

## Crates do workspace

| Crate | Responsabilidade | Depende de |
|---|---|---|
| `rhp-lexer` | Tokens manuais (+`[ ] :`, `import`), erros linha:coluna | — |
| `rhp-parser` | Precedencia, AST (+compostos, `AssignTarget`, `Import`) | `rhp-lexer` |
| `rhp-bytecode` | `Value` (+`Array`/`Map`), `Builtin` (10, ids 0–9), `CallBuiltin` etc., `compile()`, `validate()` recursivo | `rhp-parser` |
| `rhp-vm` | Frames (limite 512), escopos + globais, builtins (+`http_get` std-only) | `rhp-bytecode` (+`rhp-parser` so em dev/testes) |
| `rhp-fmt` | Formatador canonico idempotente (`format`, `format_source`) | `rhp-parser`, `rhp-lexer` |
| `rhp-cli` | `rhp run` (com loader de modulos), `rhp fmt [--check]`, `rhp check [--json]` (sem executar), integracao | todos |
| `rhp-diagnostic` | `Diagnostic` + codigos `E0xxx`–`E3xxx` + `render()` (dado separado de apresentacao; base p/ LSP) | `rhp-lexer`, `rhp-parser` |

Toolchain: Rust stable, edicao 2021.
Dependencias externas: zero (auditoria: workspace + std; `cargo tree` sem
terceiros). `unsafe` proibido e ausente. Sem `unwrap()`/`expect()` em
producao (apenas `#[cfg(test)]`, testes de integracao e `tools/`).

## Fluxo lexer → parser → bytecode → VM

```text
app.rhp --loader--> (mat.rhp + app.rhp) --lex/parse--> Program
        --compile--> Chunk { constants, names, funcs, code: [..., CallBuiltin, ...] }
        --vm.run--> frames + scopes + globais + builtins, stdout
```

- Loader (`rhp-cli`): DFS com canonicalizacao; modulo uma vez, antes do
  importador; ciclo/ausente = erro; namespace global unico.
- `rhp fmt`: parse -> AST -> texto canonico (2 espacos, `;` sempre,
  parenteses minimos, `\n` final); `--check` para CI.
- `tools/check-changes.sh`: CI que exige `changes/YYYY-MM-DD-*.md` para diffs
  relevantes (verificado em repo de teste, 4 cenarios).
- `http_get`: `TcpStream` std-only, 5s timeout, so `http://`/2xx/UTF-8.

## Proposta de CI (implementada na Fase 5)

`tools/check-changes.sh [base] [--bypass "motivo"]` em CI para mudancas em
`crates/`, `tools/`, `examples/`, `ADRs/`, specs, manifests, `hello.rhp`.
Excecao explicita via `--bypass` justificado.
