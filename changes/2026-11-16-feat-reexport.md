# [Added] — Re-export seletivo (RFC 0011, Fase 4)

- ID: 2026-11-16-feat-reexport
- Data: 2026-11-16
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (`export { } from` antes era erro de parse)

## Resumo
`export { a, b as c } from "mod.rhp";`: nomes passam aos exports do módulo
(transitivo, com alias). Sem `export *`.

## Motivação
Fechar namespaces/exports (restante da ADR 0009): barris sem re-declarar.
Branch: `feat/php-100-fase4-chamadas`.

## O que mudou
- Parser: `Stmt::ReExport`, `parse_reexport` (+ `from` obrigatório),
  docs, testes.
- Loader: `ModuleFile.reexports`, coleta com jaula/ciclo iguais aos imports
  (`resolve_dep` extraído e compartilhado), validacao contra exports do dep
  (mesmo erro do import seletivo), aliases com o mesmo `bound` (dedup global:
  mesma origem pula, origem distinta = "colisao de importacao"), strip no
  `own`, renamer passthrough, checkers (topo ok, aninhado rejeitado).
- Melhoria incidental de linkagem: `bound` global + encaminhamento
  transparente (barris + diamantes com a mesma origem funcionam; antes,
  diamante dava "ja declarada" em runtime).
- Bytecode: `ReExport` direto = erro (como `Export`). Fmt: formato canonico.
- Testes: parser (forma + 3 erros); CLI (cadeia+alias, nao-exportado, ciclo,
  aninhado, diamante de brinde); bytecode (direto); fmt.
- Specs: `LANGUAGE_SPEC.md` (gramatica + re-export), `PHP_DIFFERENCES.md` (sem
  `export *`), `ROADMAP.md`, `RFCs/0011-reexport.md`.

## Detalhes técnicos
- Forwarding: `forward[(modulo, nome)] = (origem, nome)` com indices
  estritamente decrescentes (termina; ciclos ja sao erro na coleta).
- `import` bare inclui re-exportados (superficie publica do modulo).
- Arquivos: parser/cli/bytecode/fmt + RFC.

## Compatibilidade e migração
Aditivo. Nenhuma mudanca para programas existentes (diamante que falhava
em runtime agora funciona — melhoria documentada).

## Segurança e performance
- Sem `unsafe`; mesma jaula/teto/ciclo dos imports; alias `let` por nome
  (sem reexecucao).

## Testes e validação
- Ver consolidado da Fase 4 (suite final verde).

## Limitações e riscos conhecidos
- Sem `export * from` (futuro).
- Bare import vincula re-exportados (decisao documentada).

## Referências
- `RFCs/0011-reexport.md` (aceita; estende ADR 0009)
- Branch `feat/php-100-fase4-chamadas`
