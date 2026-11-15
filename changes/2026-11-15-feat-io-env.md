# [Added] — I/O e ambiente: `fs_*`, `argv`, `env`, args na CLI

- ID: 2026-11-15-feat-io-env
- Data: 2026-11-15
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (`rhp run arquivo args...` antes rejeitava
  argumentos extras com erro de uso; agora os entrega a `argv()`)

## Resumo

Builtins `fs_read/fs_write/fs_append/fs_exists`, `argv()` e `env()`;
CLI aceita argumentos apos o arquivo e repassa a VM (`Vm::set_argv`).

## Motivação

Missão Next Generation §18 (lote imediato): I/O de arquivos e ambiente
para scripts reais, sem dependencias e sem mudar a VM alem do necessario.

## O que mudou

- `rhp-bytecode`: builtins 30–35 com aridades; teste anti-deriva estendido.
- `rhp-vm/src/builtins.rs`: `fs_*` (teto de leitura 8 MiB,
  `MAX_FILE_BYTES`), `argv` (via slice passado a `call_builtin`),
  `env` (ausente => `null`).
- `rhp-vm`: campo `argv` + `set_argv`; `run_file*` repassa args extras.
- Sem jaula de diretorio para `fs_*`: roda com as permissoes do usuario
  (mesmo modelo de qualquer linguagem de script; documentado).
- Specs: tabela de builtins; `docs/CLI_REFERENCE.md` (`run` com args).

## Arquivos ou áreas afetadas

- `crates/rhp-{bytecode,vm}/src/{lib.rs,builtins.rs}`,
  `crates/rhp-cli/src/main.rs` (+teste de integracao com args reais),
  `LANGUAGE_SPEC.md`, `docs/CLI_REFERENCE.md`.

## Compatibilidade e migração

Compatível (builtins novos; `run` com args extras passa a funcionar em vez
de erro de uso). Migração: nenhuma.

## Segurança e performance

Leitura com teto (8 MiB, erro limpo); escrita cria/sobrescreve com as
permissoes do processo — documentado como comportamento de script local.
`fs_exists` nunca falha (falso em qualquer erro), por desenho.

## Testes e validação

- Falha-antes: builtin novo sem implementacao falhava como nao-definido.
- `cargo test --workspace`: suite verde, 0 falhas (teste de integracao com
  `run arquivo a b` + asserts de `argv()`).
- `cargo fmt --check` ok; `cargo clippy --workspace --all-targets -- -D warnings` ok.

## Limitações e riscos conhecidos

Sem lock de arquivos, sem streaming, sem binario (so UTF-8), sem `cd`/`env`
de escrita, sem `exit(codigo)` (exit code continua 0 ok / 1 erro / 2 uso).

## Referências

Missão Next Generation §11.6/§11.11 (lote imediato; TLS/rede avancada e
processos com spawn ficam para RFC).
