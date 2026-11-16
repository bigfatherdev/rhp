# [Added] — Chamadas mistas: nomeados + spread (RFC 0009, Fase 4)

- ID: 2026-11-16-feat-chamadas-mistas
- Data: 2026-11-16
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (formas novas eram erro de parse)

## Resumo
`f(x: 1, y: 2)` em qualquer ordem e `f(...arr)` com splice em ordem
(misturáveis com posicionais), via `Op::CallMixed` + descritor. Chamadas
só-posicionais preservam o `Call` clássico byte a byte.

## Motivação
Completar a história de chamada (defaults+variádicos existiam, só
posicionais). Dia a dia PHP. Branch: `feat/php-100-fase4-chamadas`.

## O que mudou
- Parser: `CallArg::{Pos, Named, Spread}`, `parse_call_args` (regras
  estáticas: pos-após-nome, duplicata, `:` apos nao-Var), `Expr::Call/CallValue`
  com `Vec<CallArg>`, testes.
- Bytecode: `Op::CallMixed(u32)` + descritor `Array` nas constantes,
  `call_mixed_slots` (validador: bounds + tipo + altura), `compile_call_mixed`,
  builtin com named/spread = erro, testes (descritor, `Call` preservado).
- VM: `activate` compartilhado (refatorado de `push_call_frame` sem mudar
  comportamento), `push_call_frame_mixed` (ligacao nome/posicao, desconhecido,
  conflito posicional+named, extras/variadic, `null` explicito), braco
  `CallMixed`, testes (matriz + erros).
- Fmt: `x: v`, `...v` + idempotencia. CLI: renamer/call_arg, checkers,
  `http_server` (CallArg::Pos), testes (e2e, rename, import-em-braco).
- Specs: `LANGUAGE_SPEC.md` (gramatica + ligacao), `BYTECODE_SPEC.md`
  (op + descritor), `VM_SPEC.md` (1 linha), `PHP_DIFFERENCES.md`
  (erros estaticos vs PHP, `[...a]` futuro).

## Detalhes técnicos
- Alternativas na RFC 0009: dois arrays + append (sem op; rejeitada),
  erro em runtime estilo PHP (visivel no fonte; rejeitado), named em
  builtins (aridade fixa; rejeitado).
- `Call` classico intacto: so-`Pos` compila identico (suite confirma).
- Arquivos: parser/bytecode/vm/fmt/cli + `RFCs/0009-named-spread.md`.

## Compatibilidade e migração
Aditivo. `f(x: 1)`/`...` em chamada antes eram erro. Nenhuma migração.

## Segurança e performance
- Sem `unsafe`; descritor validado; erros sao `VmError` (capturaveis).
- Descritor por call-site misto (sem dedup, como compostos).

## Testes e validação
- `cargo fmt --check`: limpo. `cargo clippy --workspace --all-targets`:
  0 warnings/errors.
- `cargo test --workspace`: 282 passed, 0 failed (263 + 19 da Etapa 2 ate aqui).
- Novos: parser (forma + 4 erros); VM (matriz + 4 erros); bytecode
  (descritor, `Call` preservado, builtin); fmt; CLI (e2e, rename).

## Limitações e riscos conhecidos
- Sem spread em literal de array (futuro).
- Builtins nao aceitam named/spread (aridade fixa; documentado).

## Referências
- `RFCs/0009-named-spread.md` (aceita)
- Branch `feat/php-100-fase4-chamadas`
