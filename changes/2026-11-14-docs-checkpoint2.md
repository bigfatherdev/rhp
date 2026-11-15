# [Changed] — Checkpoint 2: README reestruturado + docs/ + guias

- ID: 2026-11-14-docs-checkpoint2
- Data: 2026-11-14
- Versão: unreleased
- Tipo: Changed (documentação; zero mudança de código)
- Compatibilidade: Compatível

## Resumo

README nos 15 itens da missão (objetivo, sem virar especificação) +
`docs/` navegável: GETTING_STARTED, LANGUAGE_REFERENCE (guia, sem duplicar
specs), CLI_REFERENCE, DIAGNOSTICS (catálogo real), BENCHMARKS (à parte),
FAQ; raiz: `CONTRIBUTING.md`, `SECURITY.md` (garantias testadas + limites
honestos). `docs/ARCHITECTURE.md` e `docs/SECURITY.md` omitidos de
propósito (raiz já cobre; sem duplicação).

## Motivação

Missão CHECKPOINT 2 / §4: README profissional + documentação complementar
navegável com exemplos executáveis.

## O que mudou

- `README.md` reescrito; `docs/{GETTING_STARTED,LANGUAGE_REFERENCE,
  CLI_REFERENCE,DIAGNOSTICS,FAQ}.md` (+`BENCHMARKS.md` em registro próprio);
  `CONTRIBUTING.md`; `SECURITY.md`.

## Arquivos ou áreas afetadas

- Só documentação (nenhum `.rs`, teste ou spec normativa alterada aqui).

## Compatibilidade e migração

Compatível. Migração: não necessária.

## Segurança e performance

Documentação não executa. Exemplos do guia conferidos contra a CLI real.

## Testes e validação

- Comandos do README/docs executados manualmente contra o binário
  (`run`, `fmt --check`, suite).
- `cargo test --workspace`: 120 testes, 0 falhas (nenhum teste novo;
  mudança só-docs).

## Limitações e riscos conhecidos

Guia pode defasar com a linguagem — specs seguem normativas; CI futura
pode validar exemplos dos docs (registrado como melhoria).

## Referências

- Missão §4; auditoria apenas como contexto (sem achado novo).
