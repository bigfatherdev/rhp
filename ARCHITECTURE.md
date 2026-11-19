# ARCHITECTURE.md — Arquitetura do RHP (Next Generation)

Estado real: implementado e validado em 2026-11-19. Nada aqui descreve
funcionalidade futura como pronta. Desenho: `ADRs/0001-marco-2.md` (nucleo),
`0002` (stdlib), `0003` (modulos/ferramentas), `0004` (rede/fronteira),
RFCs 0021–0027 (heranca, namespaces, async, tipadas, generics, extensoes, web).

## Crates do workspace

| Crate | Responsabilidade | Depende de |
|---|---|---|
| `rhp-lexer` | Tokens (+`\`, `extends`/`async`/etc.), erros linha:coluna | — |
| `rhp-parser` | Precedencia, AST (heranca, interfaces, traits, namespaces, async/await, tipadas, generics) | `rhp-lexer` |
| `rhp-bytecode` | `Value`, `FuncDef.is_async/native_id`, `ClassDef.parent`, ops (`BuildClassEx`, `IsInstanceOf`, `Await`, `CallSuper`, `LoadNative`), `compile()`, `validate()` | `rhp-parser` |
| `rhp-vm` | Frames (512), escopos + globais, builtins, tarefas async, `extensions` (registro estatico), worker `http_serve` com topo | `rhp-bytecode` (+`rhp-parser` so em dev/testes) |
| `rhp-fmt` | Formatador canonico idempotente (nova sintaxe) | `rhp-parser`, `rhp-lexer` |
| `rhp-cli` | `rhp run` (loader + autoload `A\B`->`A/B.rhp`), `fmt`, `check`, `test`, integracao | todos |
| `rhp-diagnostic` | `Diagnostic` + codigos `E0xxx`–`E3xxx` + `render()` (base p/ LSP) | `rhp-lexer`, `rhp-parser` |

Framework: `framework/web.rhp` (rotas, req/res, middleware, seguranca,
templates) sobre `http_serve`; exemplo `framework/exemplo_api.rhp`.
Extensoes: `crates/rhp-vm/src/extensions.rs` (`demo`; ABI 1; sem `unsafe`/`.so`).

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
