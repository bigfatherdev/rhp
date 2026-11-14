# ADR 0003 — Módulos e ferramentas (Fases 4 e 5)

- Data: 2026-11-14
- Estado: aceita (autorização "até a última fase" + execução verificada)

## Contexto

Programa de arquivo único não escala; era preciso reuso (`import`) e
ferramentas (formatação determinística + auditoria `changes/` em CI).

## Decisão

1. **Módulos v1 = inclusão com `once`.** `import "relativo/ao/arquivo.rhp";`
   resolve por DFS com canonicalização: cada módulo executa uma vez, antes do
   importador; ciclo e arquivo ausente são erros; namespace global único
   (redeclaração entre módulos é erro alto, não silencioso). `Stmt::Import` é
   consumido pelo loader; compilação direta o ignora (documentado).
2. **`rhp-fmt` como crate.** `format(&Program)` canônico (2 espaços, `;`
   sempre, parênteses mínimos por precedência, `\n` final único) e idempotente
   (teste: `fmt(fmt(x)) == fmt(x)` + preservação de AST). CLI: `rhp fmt` e
   `rhp fmt --check` (exit 1 fora do formato).
3. **CI `tools/check-changes.sh`.** Compara `base...HEAD`: se o diff toca
   áreas relevantes (crates, specs, exemplos, ferramentas, manifests) sem um
   `changes/YYYY-MM-DD-*.md` adicionado, falha; `--bypass "motivo"` registra
   exceção explícita. Verificado em repo de teste (4 cenários reais).

## Alternativas consideradas

- Sistema de módulos com exports/privacidade: adiado — v1 resolve reuso;
  exports ficam para RFC.
- Formatter externo/incremental: rejeitado — AST própria já existe; formatação
  total é simples e idempotente.

## Consequências

- `rhp run` passa pelo loader; `examples/` e `tools/` entram no escopo do CI.
