# [Changed] — README no padrão de apresentação (estilo php-src)

- ID: 2026-11-14-docs-readme-php-style
- Data: 2026-11-14
- Versão: unreleased
- Tipo: Changed (documentação; zero mudança de código)
- Compatibilidade: Compatível (só o README muda)

## Resumo

README reestruturado como página de apresentação: bloco herói + tagline +
badges estáticos honestos, tour da linguagem com exemplos executados e
verificados, instalação, CLI, testes, arquitetura, segurança, roadmap,
governança e licença — espelhando a organização do README do PHP.

## Motivação

Apresentar a linguagem a novos leitores sem exigir mergulho nas specs.

## O que mudou

- `README.md` (único arquivo).

## Detalhes técnicos

Todos os exemplos de saída foram executados contra o binário do HEAD
(`rhp 0.1`, `7`, `true`, `3`, `[9, 2, 3]`, `[a, b]`, `0/um/2`, `120`,
`fib.rhp` => `55`). Números de testes conferem com a suíte (133, 0 falhas).
Nada planejado apresentado como implementado.

## Compatibilidade e migração

Compatível. Migração: não necessária.

## Segurança e performance

Não aplicável (documentação).

## Testes e validação

- Exemplos do tour executados 1 a 1 (saídas acima).
- `cargo test --workspace`: 133 passed, 0 failed.
- `cargo fmt --check` ok (Markdown não afetado; verificado mesmo assim).

## Limitações e riscos conhecidos

Badges estáticos podem defasar (atualizar junto de mudanças de estado,
conforme governança).

## Referências

Referência estrutural: `php/php-src` README (licença BSD deles; a nossa
permanece `MIT OR Apache-2.0`, inalterada).
