# [Added] — Fase 5: ferramentas (`rhp fmt`, CI de `changes/`)

- ID: 2026-11-14-feat-fase-5-ferramentas
- Data: 2026-11-14
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (só adiciona comandos/arquivos)

## Resumo

Crate `rhp-fmt` (formato canônico idempotente) + `rhp fmt [--check]` e
`tools/check-changes.sh` (CI que exige registro em `changes/` para diffs
relevantes, com `--bypass "motivo"` para exceções explícitas).

## Motivação

Determinismo de estilo e auditoria automática (proposta da Fase 1, agora
implementada). Desenho em `ADRs/0003`.

## O que mudou

- Novo crate `rhp-fmt` (`format`, `format_source`; 2 espaços, `;` sempre,
  parênteses mínimos por precedência, `\n` final único).
- `rhp-cli`: subcomando `fmt [--check]` (exit 0/1/2); `examples/fib.rhp`
  canonicalizado com a própria ferramenta (dogfooding, saída `55` mantida).
- `tools/check-changes.sh`: falha sem `changes/YYYY-MM-DD-*.md` adicionado
  quando o diff toca crates/specs/exemplos/ferramentas/manifestos;
  ignora diffs irrelevantes; `--bypass` justificado passa com aviso.
- `ARCHITECTURE.md`/`ROADMAP.md`/`CHANGELOG.md` atualizados.

## Detalhes técnicos

Idempotência testada (`fmt(fmt(x)) == fmt(x)` + preservação de AST, incluindo
o próprio `fib.rhp`). Script em shell puro (sem dependências), operando em
`base...HEAD`.

## Compatibilidade e migração

Compatível. Migração: não necessária.

## Segurança e performance

Formatador não executa código (só parse+impressão). Sem `unsafe`.

## Testes e validação

`cargo fmt --check` ok; `cargo clippy ... -D warnings` ok;
`cargo test --workspace`: 93 testes, 0 falhas (6 unit do `rhp-fmt` +
integração `fmt`/`--check` reescreve-e-verifica).
Script verificado manualmente em repo Git de teste (4 cenários reais):
relevante sem registro → exit 1 com mensagem; com registro → exit 0;
`--bypass "tipografia pura"` → exit 0; só README → exit 0.

## Limitações e riscos conhecidos

Sem LSP/incremental; formatação total reescreve o arquivo. Diretório do
projeto segue sem Git (script testado em fixture).

## Referências

- ADR: `ADRs/0003-fase-4-5-modulos-ferramentas.md`. Autorização: "até a última fase".
