# ADR 0007 — Diagnósticos estruturados (Checkpoint 2)

- Data: 2026-11-14
- Estado: aceita (missão Next Generation, CP2; sem breaking change)

## Contexto

Erros eram strings ad-hoc por crate, sem códigos, sem spans, sem forma
legível por máquina — bloqueando LSP, debugger e documentação consistente
(NG-201).

## Decisão

1. **Crate `rhp-diagnostic`**: `Diagnostic { code, severity, path, span,
   message, help }` + `render()` (formato de referência com snippet e
   carets) + mapeamentos `lex_diagnostic`/`parse_diagnostic`/
   `compile_code`/`runtime_code`. Dado separado de apresentação (LSP-ready).
2. **Códigos estáveis**: `E0xxx` léxicos, `E1xxx` sintáticos, `E2xxx`
   compilação, `E3xxx` execução (loader incluído). `len == 0` = até o fim
   da linha. Compilação/execução não têm posição de fonte (bytecode) e
   saem como `Error[CODIGO]: mensagem` + `help:`.
3. **CLI usa o renderer** para lex/parse (com fonte para snippet) e prefixa
   códigos em compile/runtime. Mensagens originais preservadas (testes
   antigos baseados em `contains` continuam verdes).
4. **`fmt` também renderiza** via lex/parse diretos (sem `parse_source`).

## Alternativas consideradas

- Campos `code`/`span` nos tipos de erro de cada crate: rejeitado por agora —
  100+ call sites para pouco ganho; mapeamento centralizado é auditável e
  testável (tabela em `docs/DIAGNOSTICS.md`).
- Spans exatos com offsets de byte: adiado — exige tokens com span + AST
  anotada (dependência registrada para o LSP completo).

## Consequências

- Erros de CLI mudam de forma (`Error[Exxxx]: …` + snippet + `help:`),
  mantendo o conteúdo — documentado aqui e em `docs/DIAGNOSTICS.md`.
- Testes por categoria em `rhp-diagnostic` + integração fim a fim.
