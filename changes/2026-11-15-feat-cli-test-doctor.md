# [Added] — `rhp test` e `rhp doctor`

- ID: 2026-11-15-feat-cli-test-doctor
- Data: 2026-11-15
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (comandos novos; nada existente muda)

## Resumo

`rhp test [dir|arquivo]`: descobre `*_test.rhp`, executa cada um via
loader+VM com saída capturada, relata `ok`/`FALHOU` + resumo e retorna
exit 0/1. `rhp doctor`: versão, toolchain e autoteste em memória
(`6 * 7 == 42`). Exemplo versionado: `examples/saudacao_test.rhp`.

## Motivação

Missão Next Generation §9 (só comandos com funcionalidade real) e §15
(testes). `test` é o primeiro consumidor real do `assert`.

## O que mudou

- `rhp-cli`: subcomandos `test` e `doctor`, `USAGE` atualizada.
- `examples/saudacao_test.rhp` (novo; coberto pelo teste de exemplos).
- `docs/CLI_REFERENCE.md`: seções novas.

## Arquivos ou áreas afetadas

- `crates/rhp-cli/src/main.rs`, `crates/rhp-cli/tests/integration.rs`,
  `examples/saudacao_test.rhp`, `docs/CLI_REFERENCE.md`.

## Compatibilidade e migração

Compatível. Migração: não necessária.

## Segurança e performance

`test` executa código arbitrário dos arquivos alvo (mesma confiança de
`rhp run`; documentado como ferramenta de desenvolvimento, não sandbox).

## Testes e validação

- `rhp test` com 1 ok + 1 falho: exit 1 e resumo correto; dir vazio de
  testes: erro limpo; arquivo único: ok.
- `rhp doctor`: exit 0 com versão + autoteste.
- `cargo test --workspace`: suite verde, 0 falhas.
- `cargo fmt --check` ok; `cargo clippy --workspace --all-targets -- -D warnings` ok.

## Limitações e riscos conhecidos

Descoberta só no nível do diretório (sem recursão); sem filtros por nome;
sem paralelismo (sequencial e determinístico por desenho).

## Referências

Missão Next Generation §9/§15. `rhp build`/`inspect` continuam planejados
(não implementados — sem stub).
