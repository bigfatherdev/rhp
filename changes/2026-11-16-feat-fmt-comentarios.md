# [Added] — `fmt` preserva comentários e linhas em branco (Etapa 4, DevX)

- ID: 2026-11-16-feat-fmt-comentarios
- Data: 2026-11-16
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (saída idêntica sem comentários)

## Resumo

O lexer emite `//` como token `Comment` (trivia); o `Cursor` filtra e
acumula; cada statement carrega `leading`/`trailing`/`blank_before`
(`StmtNode`); o `fmt` reemite na mesma ordem relativa. Sem spans na AST
(associação por ordem, não por linha).

## Motivação

Etapa 4, item "fmt comentários" (parte do plano autorizado): `rhp fmt`
não destrói mais comentários nem linhas em branco (máx. 1).

## O que mudou

- Lexer: `TokenKind::Comment(String)` (linha de origem preservada).
- Parser: `StmtNode{leading,trailing,blank_before,stmt}`,
  `Program.trailing_comments`; `Cursor` com `pending`/`last_line`
  (`RefCell`/`Cell`, `peek` continua `&self`); `parse_stmt` drena por
  ordem; comentários dentro de expressões/entre chaves vão para o
  próximo statement (documentado).
- Fmt: `fmt_node` (blank + leading + stmt + trailing na mesma linha);
  EOF comments no fim; idempotente (`fmt(fmt(x))==fmt(x)`).
- Linker (Renamer) propaga a decoração (módulos renomeados mantêm
  comentários).
- `LANGUAGE_SPEC.md` (§1, comentários), testes `preserves_*`.

## Compatibilidade e migração

Compatível; sem migração. Sem comentários no fonte = byte a byte igual.

## Segurança e performance

Sem `unsafe`; sem panic novo (`fuzz` cobre `Dot`/`Comment`? — `Comment`
não tem braço dedicado no fuzz; parser ignora via filtro central).
Performance: 1 `Cell`/`RefCell` por peek (sem contenção real).

## Testes e validação

- `preserves_line_comments`, `preserves_comments_in_blocks`,
  `preserves_single_blank_line`, `comments_do_not_change_semantics`
  (AST idêntica com/sem comentários).
- `cargo test --workspace`: 321 passed, 0 failed; `fmt --check` limpo;
  `clippy -D warnings` 0 warnings (verificação final do lote).

## Limitações e riscos conhecidos

- Comentario dentro de expressao/bloco vazio e realocado para o
  proximo statement (preservado, nao perdido).
- Linha em branco *entre* comentarios leading e colapsada (o `fmt`
  normaliza; conteudo preservado em ordem).
- Quebra de linha antes do primeiro statement e removida (como antes).

## Referências

- `LANGUAGE_SPEC.md` §1; `tests/conformance/` (nao afetada: comentarios
  nao mudam stdout).
