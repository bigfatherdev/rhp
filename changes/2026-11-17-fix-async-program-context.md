# [Fixed] — `async_spawn` sem programa no REPL, `rhp test` e handlers

- ID: 2026-11-17-fix-async-program-context
- Data: 2026-11-17
- Versão: unreleased
- Tipo: Fixed
- Compatibilidade: Compatível (só preenche contexto que faltava)

## Resumo

`async_spawn` falhava com "sem programa-fonte (interno)" no REPL, no
`rhp test` e nos handlers do `http_serve`, pois só `rhp run` chamava
`Vm::set_program` (a thread do worker recompila o programa e precisa
dele). Achado na verificação pós-merge do PR #18 (+ auditoria de todos
os caminhos que instanciam `Vm`).

## Motivação

Correção de bug real encontrado em verificação manual (não coberto
pelos testes anteriores, que só exerciam `async_spawn` via `rhp run`).

## O que mudou

- `rhp test` (`run_tests`): `vm.set_program(Arc::new(program.clone()))`
  antes de `run` (mesmo padrao de `run_program`).
- `http_serve` (`handle_one`): mesmo `set_program` por request.
- REPL (`run_repl`): acumula AST da sessao (`Vec<StmtNode>`) e chama
  `set_program` a cada linha (worker enxerga funcoes de linhas anteriores).
- Bench/`doctor`/autotestes: nao executam codigo com `async`
  (verificados um a um; sem alteracao).
- Testes: `rhp_repl_async_sees_previous_lines`,
  `rhp_test_supports_async`,
  `rhp_run_http_serve_com_async_no_handler` (3 testes; todos falhavam
  antes da correcao).

## Compatibilidade e migração

Compatível; sem migração.

## Segurança e performance

Sem impacto (só contexto; worker recompila como antes).

## Testes e validação

- 3 testes novos de regressao (REPL, `rhp test` e handler HTTP com
  `async_spawn`; servidor real in-process no ultimo).
- `cargo test --workspace`: 323 passed, 0 failed; `fmt --check` limpo;
  `clippy -D warnings` 0 warnings.

## Limitações e riscos conhecidos

Nenhum adicional (ver RFC 0019 para limitacoes do async).

## Referências

- RFC 0019; PR #18 (verificacao pos-merge que revelou o bug).
