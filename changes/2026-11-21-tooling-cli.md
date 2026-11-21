# Tooling — REPL sobrevive a erro + multilinha; `rhp test` desce subdirs

- ID: 2026-11-21-tooling-cli
- Data: 2026-11-21
- Versão: 0.7.2
- Tipo: Tooling
- Compatibilidade: Compatível

## Resumo
Erro de sintaxe/compilação matava a sessão do REPL (doc dizia o
contrário); `fn` multilinha era impossível; `rhp test dir/` ignorava
`*_test.rhp` em subdiretórios em silêncio.

## Motivação
Auditoria 0.7.2: CLI-001 (P3), CLI-002 (P4).

## O que mudou
- `run_repl`: parse/compile com erro reportam e continuam (sem `?`);
  entrada incompleta (`fim de arquivo`, `nao terminada`) acumula com
  prompt `... ` até fechar; `sair` sozinho cancela a continuação;
  sessão só estende após compilação ok (antes, linha quebrada entrava
  na sessão e envenenava recompilações do `async`).
- `run_tests`: DFS em subdiretórios, ordem determinística por caminho.

## Detalhes técnicos
`crates/rhp-cli/src/main.rs` (`run_repl`, `run_tests`).

## Compatibilidade e migração
Adições compatíveis. `rhp test` passa a executar mais arquivos (quem
dependia de subdir ignorado, move ou renomeia). Sem migração geral.

## Segurança e performance
Sem impacto.

## Testes e validação
- `rhp_repl_survives_syntax_error_and_multiline` (stdin por pipe): ok.
- `rhp_test_descends_into_subdirectories`: ok (`2 teste(s), 0 falhas`).
- `cargo test --workspace`: 395 passed, 0 failed.

## Limitações e riscos conhecidos
Continuação é heurística (mensagem de erro); string multilinha real
(RHP não tem) segue fora. REPL sem highlight/debugger (0.8.0+).

## Referências
CLI-001, CLI-002 (auditoria 0.7.2); `docs/CLI_REFERENCE.md`.
