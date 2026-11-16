# [Added] — Closures de primeira classe (RFC 0003, PHP-100 P1)

- ID: 2026-11-16-feat-closures-primeira-classe
- Data: 2026-11-16
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível

## Resumo
`fn (params) { corpo }` agora é expressão: cria função de primeira classe com
captura do ambiente por valor. Chamadas de callee arbitrário (`(fn (x) {...})(41)`,
`f(x)(y)`) funcionam; `f(x)` direto preserva o caminho clássico (precedência
de builtins). Sem `use`, sem referência, sem `$this` (fase futura).

## Motivação
Primeiro passo (P1) do plano de paridade 100% com PHP (RFC 0003 aceita):
funções anônimas e primeira-classe destravam callbacks (`sort`/`filter`),
fábricas e IIFE. Branch: `feat/php-100-fase1-closures`.

## O que mudou
- Parser: `parse_primary` aceita `TokenKind::Fn` (nome ausente => closure);
  `parse_postfix` com sufixos `[i]` e `(args)` extraídos para
  `parse_index_suffix`/`parse_call_suffix` (`#[inline(never)]`); novos
  `Expr::CallValue { callee, args }` e `Expr::Closure(Box<ClosureData>)`
  (box mantém `Expr` em 48 bytes / `Stmt` em 120, idênticos ao baseline).
- Bytecode: `Value::Func(FuncValue { def, env })`, op `MakeFunc(u32)`
  (empilha func com snapshot; efeito de pilha +1 no validador), helper
  `compile_func_body` compartilhado por `FnDef` e `Closure` (nome
  sintético `{closure}`); `CallValue` com callee `Ident` reusa `Call`
  clássico (builtins primeiro).
- VM: `capture_env` (fusão internos→externos), `MakeFunc`/`DefineFunc`
  capturam (uniforme), ativação com escopo único = clone do env + params
  (params sombreiam; guarda de `ExitScope` preservada); `==` de funções =
  identidade de definição + igualdade do env; `json.rs` ajustado
  (`func.def.name`).
- Fmt: `CallValue` via `fmt_atom`; `Closure` multilinha com indentação
  relativa (idempotente + reparsável; §7 da RFC documenta o nivelamento).
- CLI/loader: renamer desce em `Closure` (params como sombra) e
  `CallValue`; `import`/`export` dentro de closure rejeitado com o mesmo
  erro claro das funções (`check_stmt_expr_imports`/`check_expr_imports`).
- Specs: `LANGUAGE_SPEC.md` (sintaxe/semântica/gramática/`==`),
  `BYTECODE_SPEC.md` (`FuncValue`, `MakeFunc`, validador),
  `VM_SPEC.md` (ativação/env), `MEMORY_MODEL.md` (`FuncValue`),
  `PHP_DIFFERENCES.md` (captura implícita vs `use`, sem `&`, sem `$this`),
  `ROADMAP.md` (item marcado feito).

## Detalhes técnicos
- `MAX_PARSE_DEPTH = 128` mantido; regressão de stack evitada extraindo os
  braços do loop (frame de `parse_postfix` enxuto no caminho quente de
  parênteses; teste `rejects_excessive_expression_nesting` verde sem
  `RUST_MIN_STACK` elevado).
- `let` no nível superior vive em `globals` (fora da cadeia de escopos):
  closure e `fn` nomeada enxergam o valor atual em paridade (não congela
  comportamento existente — breaking evitado). Snapshot real vale em
  ativações/blocos (teste `mk()` comprova `10` após `n = 20`).
- Arquivos: `crates/rhp-parser/src/lib.rs`, `crates/rhp-bytecode/src/lib.rs`,
  `crates/rhp-vm/src/lib.rs`, `crates/rhp-vm/src/json.rs`,
  `crates/rhp-fmt/src/lib.rs`, `crates/rhp-cli/src/main.rs`,
  `RFCs/0003-closures-primeira-classe.md` (+ specs/roadmap acima).

## Compatibilidade e migração
Compatível: nenhum programa válido muda de comportamento. `f(x)` direto,
builtins, `fn` nomeada, `==` de funções nomeadas e lookup de globais
inalterados. Nenhuma migração necessária.

## Segurança e performance
- Sem `unsafe`; sem `unwrap`/`expect` em produção (apenas testes).
- `env` clonado por `MakeFunc`/`Call`: custo O(escopo) por criação/chamada
  de closure — assumido e documentado (mesma classe das cópias por valor
  do modelo de memória). Sem medição de bench dedicada (sem regressão
  observada na suíte).
- `import` em closure não é mais silenciosamente ignorado (erro claro).

## Testes e validação
- `cargo fmt --check`: limpo. `cargo clippy --workspace --all-targets`:
  0 warnings/errors.
- `cargo test --workspace`: 224 passed, 0 failed (baseline 218 + 6 novos).
- Novos: parser `parses_anonymous_fn_and_general_calls`;
  bytecode `compiles_closure_and_general_calls`;
  VM `closures_capture_by_value_and_call` (IIFE, snapshot via `mk()`,
  paridade top-level, sombra de param, encadeada, erro `como funcao`);
  fmt `formats_closures_and_general_calls` (golden + idempotência/reparse);
  CLI `runs_closures_end_to_end`, `rejects_import_inside_closure`,
  `rename_closures_shadow_params`.

## Limitações e riscos conhecidos
- Indentação de closures aninhadas no fmt não herda nível absoluto
  (estável/reparsável; ver RFC 0003 §7).
- Sem `use`/`&`, sem `$this`, sem `static`/arrow `fn` — fases futuras.
- `==` de closures compara env inteiro (O(n)); aceito nesta etapa.

## Referências
- `RFCs/0003-closures-primeira-classe.md` (aceita)
- Branch `feat/php-100-fase1-closures` (base `main@100e26f`, PR #5, tag v0.1.0)
