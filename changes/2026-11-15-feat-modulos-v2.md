# [Added] — Módulos v2: `export` opt-in e `import { } from` seletivo (ADR 0009)

- ID: 2026-11-15-feat-modulos-v2
- Data: 2026-11-15
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (bare `import` preserva semântica legada;
  `export` vira reservada — zero usos como identificador no repo)

## Resumo

Implementa a ADR 0009: `export fn/let/const` opt-in por definição,
`import { a, b as c } from "m.rhp"` vinculando só exportados
(privacidade real), alias, colisão só entre nomes visíveis, ciclos ainda
proibidos, execução única preservada. `from`/`as` contextuais (como `in`).

## Motivação

Fase A3 do plano de sistemas: sem privacidade/alias não há como organizar
código de um sistema (stdlib própria, handlers, pacotes futuros).

## O que mudou

- Lexer/parser: keyword `export`; `Stmt::Export{def}`, `Stmt::ImportNames`,
  `ImportName{name, alias}`; erros claros (export só admite fn/let/const;
  `import{}` vazio; `from` ausente; nested import/export rejeitados no loader).
- Loader (`rhp-cli`): coleta em 2 fases (DFS com jaula/ciclo/teto 8 MiB
  preservados) + `link_modules`: renomeação scope-aware por módulo
  (prefixo com NUL; preserva shadowing, `for`/`catch`/params, chamadas),
  aliases `let` no topo da seção do importador (nunca reexecuta),
  dedup por (importador, visível, origem) — mesma origem pula, origem
  distinta = erro `colisao de importacao`.
- Bare mantém legado (vincula tudo, inclusive privados); seletivo exige
  exportado (`não e exportado` senão). Bare+seletivo mistos sem
  reexecução. Identidade = caminho canonicalizado (links distintos do
  mesmo arquivo seguem dedupados; hash de conteúdo registrado como
  trabalho futuro de cache, cf. ADR item 6 — sem alegação além do
  implementado).
- Compilador rejeita `ImportNames`/`Export` diretos (só via loader);
  `rhp-fmt` imprime ambas as formas (idempotente); gramática VSCode +
  teste (`export`; `from`/`as` de fora, por serem contextuais).
- Spec: `LANGUAGE_SPEC.md` §1 reescrito (módulos v2).

## Detalhes técnicos

Linker é AST-para-AST (sem op novo, VM intacta). `rename_module` espelha
escopos do compilador; `emit` de alias é `let` simples (sem saída/efeito).
`check_no_nested_imports` cobre `try`/`catch`/`finally` e `export`.

## Compatibilidade e migração

Programa sem `export`/seletivo executa byte-idêntico (suite legada verde,
incl. `loads_modules_once`, circular, jaula). `export` reservada:
renomear eventual variável homônima (nenhuma no repo).

## Segurança e performance

Sem `unsafe`, sem I/O novo. Alias-lets O(nomes) por import; rename O(nós).
Reexport e pacotes seguem pendentes (fora do escopo, como na ADR).

## Testes e validação

- Falha-antes: seletivo/privacidade/alias/colisão/ciclo/nested-export
  (5 testes novos em `rhp-cli` + `rename_respects_shadowing` + 2 de
  parser + 1 de fmt, todos verdes).
- `cargo test --workspace`: 188 passed, 0 failed (181 antes + 7 novos).
- `cargo fmt --check` ok; `cargo clippy --workspace --all-targets
  -D warnings` ok; `tools/check-changes.sh origin/main` ok.

## Limitações e riscos conhecidos

Sem `export { }` em lista nem reexportação (sintaxe ficou para RFC
futura, cf. ADR). Sem cache entre execuções. Erro de colisão agora é em
link-time (mensagem nova, mesma classe de erro de antes).

## Referências

ADR 0009 (implementada; reexport segue pendente de RFC). Sem ADR nova.
