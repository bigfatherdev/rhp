# [Added] — Fase 3: stdlib núcleo (compostos + builtins)

- ID: 2026-11-14-feat-fase-3-stdlib-nucleo
- Data: 2026-11-14
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (todo programa anterior continua válido;
  `hello.rhp` e `fib.rhp` reverificados)

## Resumo

Arrays (`[1, "a"]`) e mapas (`{k: v}`) com semântica por valor (cópia, sem
aliasing), indexação estrita e `nome[i] = v` (1 nível), mais builtins
`len push pop keys str int float type assert` com aridade checada em
compilação.

## Motivação

Base de valores compostos para o resto da visão (módulos, I/O). Desenho em
`ADRs/0002-fase-3-stdlib-nucleo.md` (cópia em vez de referência; erros
explícitos em vez de coerção silenciosa).

## O que mudou

- `rhp-lexer`: tokens `[` `]` `:`.
- `rhp-parser`: `Expr::{Array, Map, Index}`, `AssignTarget::{Var, Index}`,
  nível posfixo, alvo inválido (`f() = v`) como erro de parse.
- `rhp-bytecode`: `Value::{Array, Map}`, ops `BuildArray BuildMap Index
  StoreIndex CallBuiltin(id 0–8)`, `builtin_by_name/arity`, validação de id.
- `rhp-vm`: execução dos novos ops, conversões estritas, `Display`
  determinístico (`[1, 2]`, `{a: 1, b: 2}` com chaves ordenadas).
- Specs (`LANGUAGE_SPEC` §§5–6, `BYTECODE_SPEC`, `VM_SPEC`) atualizadas.

## Detalhes técnicos

`StoreIndex` lê-avalia `[indice, valor]` e grava atomicamente na variável
(erro preserva o valor antigo). `push` retorna novo array. `int(1.5)`,
`int("abc")`, `pop([])`, índice fora do intervalo e chave ausente são erros.
`==` com compostos compara conteúdo; `<` com compostos é erro.

## Compatibilidade e migração

Compatível. Migração: não necessária.

## Segurança e performance

Sem `unsafe`; sem unwrap/expect em produção; bytecode validado antes.
Performance não medida; cópias documentadas como não-meta.

## Testes e validação

Suíte final em 2026-11-14 (Ubuntu 26.04, Rust stable): `cargo fmt --check`
ok; `cargo clippy --workspace --all-targets -- -D warnings` ok;
`cargo test --workspace`: 93 testes, 0 falhas (inclui 16+ testes novos desta
fase: parser array/map/index/alvo, bytecode shapes/aridade/ids, VM
build/index/store/builtins/erros; integração cobre compostos via programas
temporários).

## Limitações e riscos conhecidos

Atribuição indexada só em 1 nível; sem métodos; sem tipos compostos
aninhados além do suportado (funcionam, sem otimização). Diretório sem Git.

## Referências

- ADR: `ADRs/0002-fase-3-stdlib-nucleo.md`. Autorização: "até a última fase".
