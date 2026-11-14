# [Added] — Fase 4: módulos (`import`)

- ID: 2026-11-14-feat-fase-4-modulos
- Data: 2026-11-14
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (programas sem `import` inalterados)

## Resumo

`import "caminho.rhp";` com resolução em profundidade: cada módulo executa
uma vez antes do importador, caminhos relativos ao arquivo, dedup automático,
ciclo e arquivo ausente como erro. Namespace global único.

## Motivação

Reuso multi-arquivo antes de I/O e ferramentas. Desenho em `ADRs/0003`
(módulos v1 = inclusão com `once`; exports ficam para RFC futura).

## O que mudou

- `rhp-lexer`: keyword `import`.
- `rhp-parser`: `Stmt::Import(String)` (caminho precisa ser string),
  coleta recursiva de imports em qualquer aninhamento.
- `rhp-bytecode`: compilação direta ignora `Import` (documentado: programa
  deve chegar resolvido pelo loader).
- `rhp-cli`: `load_program` (canonicalização, DFS, detecção de ciclo com
  cadeia `a -> b -> a`, dedup); `rhp run` passa pelo loader.
- Specs e `ROADMAP.md`/`CHANGELOG.md` atualizados.

## Detalhes técnicos

Redeclaração entre módulos é erro alto (`variavel ja declarada`), não
silencioso. Loader só na CLI (núcleo segue puro/funcional).

## Compatibilidade e migração

Compatível. Migração: não necessária.

## Segurança e performance

Imports limitados ao filesystem local via caminho relativo; ciclo não
estoura pilha (erro antes). Sem `unsafe`; sem unwrap/expect em produção.

## Testes e validação

`cargo fmt --check` ok; `cargo clippy ... -D warnings` ok;
`cargo test --workspace`: 93 testes, 0 falhas (inclui parser de import,
loader com dedup/ordem, circular e ausente em unit + integração fim a fim
com `mat.rhp` → `42`).

## Limitações e riscos conhecidos

Sem `export`/privacidade; namespace único global. Diretório sem Git.

## Referências

- ADR: `ADRs/0003-fase-4-5-modulos-ferramentas.md`. Autorização: "até a última fase".
