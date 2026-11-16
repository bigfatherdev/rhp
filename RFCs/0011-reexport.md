# RFC 0011 — Re-export (`export { } from`)

- ID: RFC-0011
- Data: 2026-11-16
- Estado: aceita (programa RHP-100; implementada na Fase 4)
- Contexto: fecha o restante de namespaces/exports da ADR 0009 (só faltava
  o re-export; `export *` fica de fora).

## 1. Problema

Barril (`index.rhp` re-exportando utilidades) exige re-declarar e manter
aliases manuais. Falta `export { a, b as c } from "mod.rhp"`.

## 2. Objetivo e não-objetivos

Objetivo: re-export seletivo com alias; nomes passam a compor os exports
do re-exportador (transitivo); mesmas regras de erro do import seletivo.
Nao-objetivos: `export * from` (futuro; exige enumeracao de exports com
ciclo across — anotado); re-export condicional/dinamico.

## 3. Decisao de desenho

- Sintaxe: `export { a, b as c } from "caminho";` (mesmo `ImportName`;
  `from`/`as` contextuais como no import seletivo).
- AST: `Stmt::ReExport { names: Vec<ImportName>, path: String }`
  (variante dedicada; `Export` segue so para definicao local).
- Loader: `collect_imports` coleta re-exports como dependencias (mesmo
  ciclo/jaula/teto); `ModuleFile.reexports`; ciclo via re-export =
  "importacao circular" (mesma deteccao).
- Linker (`link_modules`): passo 1 valida nomes ⊆ exports do dep
  (mesmo erro "`x` nao e exportado por `mod`") e soma aos exports do
  modulo (duplicata = "export duplicado"); passo 2 emite `let visivel =
  origem` com o mesmo `bound` (colisao real = erro); statements de
  re-export somem do bundle (como imports).
- Cadeias transitivas funcionam por construcao (exports do intermediario
  incluem os re-exportados).
- Compilacao direta de `ReExport` = erro (como `Export` fora do loader).
- Checkers/renamer: `ReExport` tratado como `Import` (topo ok, aninhado
  rejeitado; renamer passa adiante — linker remove antes).

## 4. Alternativas consideradas

- Estender `Stmt::Export` com enum interno: rejeitado (variante dedicada
  e mais clara nos matchs exaustivos).
- `export * from`: rejeitado nesta fase (enumeracao + ciclos; anotado).

## 5. Compatibilidade

Aditivo. `export ... from` antes era erro de parse. Nenhum programa
valido muda.

## 6. Seguranca

Mesma jaula/teto/ciclo dos imports (re-export coleta igual); sem `unsafe`.

## 7. Performance

Alias `let` por nome (como imports; sem reexecucao).

## 8. Plano de implementacao

1. Parser: `Stmt::ReExport`, parse (`export { } from`), checkers,
   testes.
2. CLI loader/linker (coleta, validacao, aliases, renamer passthrough,
   `compile` direto = erro), testes (cadeia, alias, erro de nao-exportado,
   ciclo via re-export, aninhado rejeitado).
3. Fmt (formato canonico) + specs + testes + `changes`.

## 9. Plano de testes

- Basico + alias; cadeia a→b→c; nao-exportado (erro); duplicata com
  export local (erro); ciclo a↔b via re-export (erro circular);
  re-export em bloco/fn (erro nivel superior); `compile` direto (erro).
- Suite sem regressao.

## 10. Migracao e rollout

Nenhuma (aditivo).

## 11. Criterios de aceitacao

- [ ] Testes da §9 passam; suite sem regressao.
- [ ] `fmt`/`clippy`/`test` + `check-changes.sh` verdes.
- [ ] Specs + `changes`.
- [ ] Esta RFC marcada `aceita` antes do merge.
