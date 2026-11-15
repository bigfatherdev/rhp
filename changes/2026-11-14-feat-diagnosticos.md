# [Added] — CP2: diagnósticos estruturados (`rhp-diagnostic`)

- ID: 2026-11-14-feat-diagnosticos
- Data: 2026-11-14
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível na semântica (forma da mensagem muda,
  conteúdo preservado; testes antigos baseados em `contains` intactos)

## Resumo

Crate `rhp-diagnostic`: `Diagnostic` + códigos estáveis (`E0xxx`–`E3xxx`) +
`render()` no formato de referência com snippet e `help:`. CLI usa o
renderer para lex/parse (com fonte) e prefixa códigos em compile/runtime.

## Motivação

Missão Next Generation, CP2 (NG-201): erros ad-hoc bloqueavam LSP, debugger
e documentação consistente.

## O que mudou

- Novo crate `rhp-diagnostic` (tipos, `render`, mapeamentos lex/parse/
  compile/runtime + 5 testes).
- `rhp-cli`: `parse_with_diagnostics`, `with_code`, `fmt` via lex/parse
  diretos; `docs/DIAGNOSTICS.md` reescrito como catálogo com códigos.
- ADR 0007. Sem mudança em lexer/parser/bytecode/VM/fmt (mapeamento
  centralizado, sem reescrita do núcleo).

## Arquivos ou áreas afetadas

- `crates/rhp-diagnostic/` (novo), `crates/rhp-cli/{Cargo.toml,src/main.rs,
  tests/integration.rs}`, `docs/DIAGNOSTICS.md`, `Cargo.toml`, `ADRs/0007*`.

## Compatibilidade e migração

Forma da saída de erro muda (prefixo `Error[CODIGO]`, snippet, `help:`);
códigos de saída e conteúdo preservados. Migração: scripts que fazem parse
exato do stderr devem tolerar o novo formato.

## Segurança e performance

Render O(linha); sem I/O extra. Sem `unsafe`.

## Testes e validação

- 5 testes unitários (formato exato, sem fonte, 3 tabelas de mapeamento) +
  integração (`Error[E0001/E1004/E3001]` + caret + snippet + help).
- `cargo test --workspace`: 126 testes, 0 falhas.
- `cargo fmt --check` ok; `cargo clippy ... -D warnings` ok.

## Limitações e riscos conhecidos

Spans aproximados (ponto + resto-da-linha); sem offsets de byte (dependência
para LSP completo). `Warning`/`Note` ainda não existem.

## Referências

- Auditoria: `reports/NEXT_GENERATION_AUDIT.md` (NG-201). ADR:
  `ADRs/0007-diagnosticos.md`.
