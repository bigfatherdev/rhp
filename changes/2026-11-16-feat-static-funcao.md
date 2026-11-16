# [Added] — `static` em função (RFC 0010, Fase 4)

- ID: 2026-11-16-feat-static-funcao
- Data: 2026-11-16
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (ressalva: `static` vira keyword)

## Resumo
`static n = expr;` dentro de função: estado por definição, atribuído só
na primeira chamada; lido/escrito normalmente depois; compartilhado entre
instâncias da mesma closure.

## Motivação
Contadores, memoização e singletons locais sem global. Branch:
`feat/php-100-fase4-chamadas`.

## O que mudou
- Lexer: `Static` + keyword + teste.
- Parser: `Stmt::Static`, `parse_static` (init obrigatório), dispatch,
  docs, testes.
- Bytecode: **só `Op::DeclareStatic`** (+ validador); decisão documentada:
  `LoadStatic`/`StoreStatic` seriam código morto (nunca emitidos) — o
  fallback em `lookup`/`assign` (escopos → statics → globais) cobre tudo,
  com shadowing natural. `static` fora de função = erro (via `in_function`).
- VM: `Frame.func: Option<Rc<FuncDef>>`, `statics: Vec<(Rc<FuncDef>, map)>`
  com `ptr_eq` (linear, poucas definições), `static_vars`, fallback em
  lookup/assign, braço `DeclareStatic` (insere se ausente), testes.
- Fmt + CLI (renamer/checkers) + specs + testes.

## Detalhes técnicos
- Init com efeito colateral roda por chamada, mas só a primeira atribui
  (divergência PHP documentada; teste cobre: `n = 2` após 2 chamadas).
- `let` após `static` no mesmo escopo sombreia (documentado, evite).
- Arquivos: lexer/parser/bytecode/vm/fmt/cli + `RFCs/0010-static-funcao.md`.

## Compatibilidade e migração
Aditivo + keyword nova (`static` como nome = erro claro de parse).

## Segurança e performance
- Sem `unsafe`; lookup extra só após miss nos escopos (caminho quente igual).
- Sem `Send`; pinagem por `Rc` evita colisão ABA (mapa limpo no `reset`).

## Testes e validação
- Ver seção "Testes e validação" do registro consolidado da Fase 4
  (suite final: fmt/clippy/test + check-changes verdes).
- Novos: parser (forma, sem-init); VM (contador, isolamento por definicao,
  instancias compartilhadas, init-uma-vez, nao-inicializada, fora-de-funcao);
  bytecode (DeclareStatic, fora-de-funcao); fmt; CLI (e2e, fora-de-funcao).

## Limitações e riscos conhecidos
- Sem `static` fora de função; sem múltiplos nomes por statement.
- Init sempre avaliado (mesmo após a primeira — custo documentado).

## Referências
- `RFCs/0010-static-funcao.md` (aceita)
- Branch `feat/php-100-fase4-chamadas`
