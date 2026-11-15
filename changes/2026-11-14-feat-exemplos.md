# [Added] — Exemplos de uso (`examples/`)

- ID: 2026-11-14-feat-exemplos
- Data: 2026-11-14
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (só adiciona arquivos `.rhp` + 3 asserts)

## Resumo

Sete novos exemplos executáveis: `hello_world`, `fizzbuzz`, `fatorial`,
`agenda` (mapas/arrays), `mini_db` (tabela em memória), `saudacao_lib` +
`saudacao` (módulo com `import`). Cobertura automática pelo teste que roda
todos os `examples/*.rhp`; asserts de saída pinados para `hello_world`,
`fatorial` (e `fib`, preexistente).

## Motivação

Pedido direto: mais exemplos de uso na pasta `examples/`, incluindo o tema
"conexão com banco de dados".

## O que mudou

- `examples/{hello_world,fizzbuzz,fatorial,agenda,mini_db,saudacao_lib,saudacao}.rhp`
- `crates/rhp-cli/tests/integration.rs`: asserts de saída exata.
- `docs/FAQ.md`: pergunta sobre bancos (ainda sem drivers; aponta simulação e ROADMAP).
- `ROADMAP.md`: `rhp fmt` preservando comentários como item futuro.

## Detalhes técnicos

Todos executam com exit 0 e saída determinística (verificada programa a
programa). `mini_db` é **simulação explícita** com arrays/mapas — nenhuma
alegação de conectividade real; cabeçalho do arquivo e FAQ deixam isso
claro. `saudacao_lib.rhp` sozinho sai sem saída (só define `fn`), exit 0.

Limitação conhecida registrada aqui (auditoria de cobertura): o `rhp fmt`
v1 **descarta comentários e linhas em branco** ao canonicalizar — os novos
exemplos priorizam legibilidade e por isso não passam em `fmt --check`
(hello.rhp/fib.rhp continuam canônicos). Preservação de comentários fica
como trabalho futuro (ROADMAP).

## Compatibilidade e migração

Compatível. Migração: não necessária.

## Segurança e performance

Exemplos pequenos e determinísticos; sem I/O além de `echo`.

## Testes e validação

- Cada exemplo executado manualmente (`exit=0`, saídas conferidas:
  FizzBuzz 1–15 correto, `720`, agenda, tabela 2 registros, `ola, mundo!`).
- `cargo test --workspace`: suite verde (teste de exemplos cobre os novos
  automaticamente + asserts de saída).
- `cargo fmt --check` ok (Rust; `.rhp` com comentários fora do escopo do
  `fmt --check`, conforme limitação acima).

## Limitações e riscos conhecidos

Sem drivers de PostgreSQL/Redis (planejados, exigem servidores). `fmt`
não preserva comentários (v1).

## Referências

Sem RFC/ADR (exemplos usam só o implementado). Pedido direto no chat.
