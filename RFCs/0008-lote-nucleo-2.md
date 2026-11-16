# RFC 0008 — Lote núcleo-2: `match`, Elvis, arrow, `for(;;)`, `break N`

- ID: RFC-0008
- Data: 2026-11-16
- Estado: aceita (programa RHP-100; implementada na Fase 3)
- Contexto: fecha o núcleo imperativo (P do plano). Itens pequenos, sem
  arquitetura; um lexer token novo (`=>`), duas keywords (`match`, ... —
  `for`/`break`/`continue` já existem).

## 1. Problema

Faltam no núcleo: `match` como expressão (PHP 8), Elvis `?:`, arrow
`fn =>`, `for` clássico e `break`/`continue` com nível — todos dia a dia PHP.

## 2. Objetivo e não-objetivos

Objetivo: os cinco, com semântica RHP (sem truthiness, sem `===` nativo).
Não-objetivos: `match` com guardas/patterns profundos; `for` com múltiplas
exprs por seção (PHP permite `for ($i=0,$j=0; ...)` — só uma por seção);
`break` com nível dinâmico (só literal); `goto` (nunca).

## 3. Decisões de desenho

- **`match expr { a => x, b => y, default => z }`**: expressão; igualdade
  **estrita** (mesmo tipo + `==`; `Int`/`Float` promovem como no resto —
  documentado vs `switch` frouxo e vs `===` PHP); sem fallthrough; braços em
  ordem, primeiro match vence; `default` único, qualquer posição (casa quando
  alcançado); sem match e sem `default` = erro de runtime; vazio = parse erro.
  Vírgula final opcional. Compila como o `switch` (testes→corpos, `Dup` por
  braço, `Pop` final), sem `break` (cada braço desvia ao fim).
- **Elvis `a ?: b`**: açúcar de parse para `a ?? b` (RHP não tem truthiness;
  divergência PHP documentada: lá, falsy aciona `b`; aqui, só `null`).
  Sem token novo (`?` + `:` com lookahead).
- **Arrow `fn (params) => expr`**: açúcar para closure com corpo
  `[return expr]` (mesmo `ClosureData`; fmt normaliza para bloco).
  Token `FatArrow` (`=>`); `=` seguido de `>` antes era erro — sem quebra.
- **`for init; cond; passo { corpo }`** (todas as partes opcionais;
  `for (;;)` = infinito): `Stmt::ForC { init, cond, step, body }` com
  `init` = `let`/`const`/atribuicao/expressao (ou ausente),
  `cond: Option<Expr>` (`None` = `true`), `step` = atribuicao/expressao
  (ou ausente; `let`/`const` no passo = erro, redeclararia a cada iteracao).
  Compilação própria (espelha `compile_for`): `continue` salta ao passo
  (não à condição). Desambiguação `for-in` vs `for-C` por lookahead:
  `Ident` + `in` = for-in; `Ident` + `;`/`=`/`(` = for-C; `Ident` + resto
  cai no caminho for-in (erro útil "esperado `in` para typos).
- **`break N;` / `continue N;`** (`N` = literal `>= 1`; ausente = 1):
  `break N` sai dos N breakables mais internos (laço ou `switch`);
  `continue N` mira o N-ésimo laço externo (pulos de `switch` transparentes,
  como no `continue` simples). N além do aninhamento = erro de compilação.
  `Pop` por `switch` cruzado generalizado (mesma regra da RFC 0004).

## 4. Alternativas consideradas

- `match` frouxo (= `switch`): rejeitado (PHP `match` é estrito; duas
  semânticas distintas já documentadas valem mais que uniformidade falsa).
- Elvis com truthiness: rejeitado (RHP não tem truthiness em lugar nenhum;
  `if` exige `Bool`).
- Desugar `for-C` para `while` no parse: `continue` pularia o passo;
  rejeitado (compilação própria, trivial com `LoopCtx`).
- `break` dinâmico (`break $n`): rejeitado (saltos são estáticos; validador
  exige alturas determinísticas).

## 5. Compatibilidade

Aditivo, com ressalvas: `match` vira keyword (renomear identificadores;
erro claro); `=>` antes era erro léxico/sintático. `for`/`break`/`continue`
já eram reservadas. Demais programas: nenhuma mudança.

## 6. Segurança

Sem `unsafe`; `match`/`for-C`/`break N` cobertos pelo validador de pilha;
`N` limitado pelo aninhamento real (sem salto selvagem).

## 7. Performance

`match`: um `Dup`+`Eq` por braço até o match (como `switch`). Resto: custo
zero vs formas existentes (açúcar/desugar estrito).

## 8. Plano de implementação

1. Lexer: `FatArrow` (`=>`; `=`+`>`), `Match` keyword, testes.
2. Parser: `Expr::Match { subject, arms: Vec<MatchArm>, }`
   (`MatchArm { cond: Option<Expr>, value: Expr }`), `parse_match`;
   Elvis no `parse_ternary` (desugar p/ `NullCoalesce`); arrow no braço `Fn`;
   `Stmt::ForC` + lookahead `for-in` vs `for-C`; `break`/`continue` com nível
   (`Option<u32>` em `Stmt::Break/Continue`), testes.
3. Bytecode: `compile_match` (testes→corpos, braço→fim, `Pop` final),
   `compile_for_c`, `break/continue N` (alvo N-ésimo + `Pop` por switch
   cruzado), testes.
4. VM: nada (reuso).
5. Fmt (todas as formas + idempotência) + CLI (renamer/checkers nos novos
   nós) + specs + exemplos + `changes/` + commit.

## 9. Plano de testes

- `match`: valor retornado, estrito (`1` vs `"1"`/`true`), ordem, default
  meio/só, sem-match (erro), vazio (parse erro), aninhado, em expressão
  (`let x = match ...`), `default` duplo (parse erro).
- Elvis: `null` vs valor; avaliação única do lado esquerdo.
- Arrow: valor, captura, chamada direta, fmt normaliza p/ bloco.
- `for-C`: clássico, partes vazias, infinito com `break`, `continue` roda
  o passo, `for-in` intacto, ambiguidade `for x in` preservada.
- `break/continue N`: níveis 1–3, switch-no-caminho, além do aninhamento
  (erro), `continue 2` pulando `switch`.
- Suite sem regressão.

## 10. Migração e rollout

Renomear identificadores `match` (erro claro); `=>` antes inválido.
Demais: nenhuma.

## 11. Critérios de aceitação

- [ ] Testes da §9 passam; suite sem regressão.
- [ ] `fmt`/`clippy`/`test` + `check-changes.sh` verdes.
- [ ] Specs + divergências em `PHP_DIFFERENCES.md`.
- [ ] Esta RFC marcada `aceita` antes do merge.
