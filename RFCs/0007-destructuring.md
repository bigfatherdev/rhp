# RFC 0007 — Destructuring raso: `let [a, b]` / `let {x}` / `let {k: v}`

- ID: RFC-0007
- Data: 2026-11-16
- Estado: aceita (programa RHP-100; implementada na Fase 2)
- Contexto: último item P3 do plano aprovado; completa a ergonomia de
  dados (arrays/mapas por valor) com desempacotamento direto.

## 1. Problema

Sem destructuring, cada campo exige `let t = expr; let a = t[0]; ...`
(temporário manual, ruído, risco de avaliar 2x).

## 2. Objetivo e não-objetivos

Objetivo: `let [a, b] = arr` (por posição), `let {x, y} = mapa`
(atalho: chaves = nomes), `let {k: v} = mapa` (renomeia), com `const`
(`const [a, b] = ...` congela todos). RHS avaliado uma única vez em
temporário oculto.

Não-objetivos: padrões aninhados (`[[a]]`, `{p: [x]}` = erro de parse
claro); defaults dentro do padrão (`[a = 1]` = erro — use `let` simples);
`for [a, b] in` (futuro); `...rest` no padrão (futuro); ignorar posição
(`[, b]` — sem placeholder nesta fase, erro de parse).

## 3. Decisão de desenho

- AST: `Stmt::Destructure { target: DestructureTarget, init: Expr, is_const: bool }`
  com `DestructureTarget::{Positional(Vec<String>), ByKey(Vec<(String, String)>)}`
  (`(chave, nome)`; atalho `{x}` = `("x", "x")`). `Stmt::Let` simples
  intacto (churn zero nos caminhos existentes).
- Parse: `let`/`const` + `[`/`{` = padrão (vazio = erro; aninhado = erro;
  `const` exige init como no simples). O resto do `let` não muda.
- Compilação: `init` + `DeclareName` em temporário oculto (`\0...`,
  impossível no fonte, como o `for`); por alvo: `LoadName(tmp)` +
  `Constant(indice|chave)` + `Index` + `DeclareName(nome)`
  (+ `declare_const` se `is_const`). Sem ops novos.
- Semântica herdada (sem regras novas): índice fora dos limites e chave
  ausente = erro do `Index` vigente; duplicata no padrão = erro de
  `DeclareName` vigente; `const` respeita `is_const` (atribuição posterior
  = erro de compilação, como no simples).

## 4. Alternativas consideradas

- Estender `Stmt::Let` com `LetTarget`: churn em todos os matchs
  (`bytecode`/`fmt`/`CLI`/testes); rejeitado em favor da variante dedicada.
- `for [a,b] in` junto: escopo por iteração + temporário por volta;
  rejeitado nesta fase (futuro, com RFC própria se preciso).
- `null` em posição ausente (PHP `list()`): rejeitado (RHP é estrito;
  erro herdado do `Index` é mais útil que `null` silencioso).

## 5. Compatibilidade

Aditiva. `[`/`{` após `let`/`const` antes era erro de parse ("esperado
identificador"); nenhum programa válido muda.

## 6. Segurança

Sem `unsafe`; temporário oculto não colide (NUL); sem ops novos; validador
cobre a sequência (`LoadName`/`Index`/`DeclareName` padrão).

## 7. Performance

Uma avaliação do RHS + um `Index` por alvo (mesmo custo do manual).

## 8. Plano de implementação

1. Parser: `DestructureTarget`, `Stmt::Destructure`, `parse_let` estendido,
   erros (vazio, aninhado, `[,`, `const` sem init — já regra), testes.
2. Bytecode: braço `compile` (temp oculto + `Index` + `declare_const`),
   testes (forma compilada, const, erros).
3. VM: nada (ops existentes).
4. Fmt (`let [a, b] = ...`, `const {k: v} = ...`) + CLI (renamer declara
   nomes; checkers: `check_stmt_expr_imports` cobre `init`) + specs
   (`LANGUAGE_SPEC`, `PHP_DIFFERENCES`: estrito vs `null` do PHP,
   sem aninhados) + exemplo + testes + `changes/` + commit.

## 9. Plano de testes

- Parse: formas, erros (vazio, aninhado, `[,]`, `const` sem init).
- VM e2e: array, mapa atalho, mapa renomeado, const + tentativa de
  atribuição (erro), índice fora (erro), chave ausente (erro), duplicata
  (erro), RHS 1x (efeito colateral conta), vazio `{}`? (erro — decidir:
  `let {} = m` sem nomes = erro de parse, inútil mas válido? erro).
- Fmt idempotente + reparse; CLI rename; suite sem regressão.

## 10. Migração e rollout

Nenhuma (aditivo; `[`/`{` após `let`/`const` era erro).

## 11. Critérios de aceitação

- [ ] Testes da §9 passam; suite sem regressão.
- [ ] `fmt`/`clippy`/`test` + `check-changes.sh` verdes.
- [ ] Specs + divergências em `PHP_DIFFERENCES.md`.
- [ ] Esta RFC marcada `aceita` antes do merge.
