# [Changed] — README raiz com o estado real do projeto

- ID: 2026-11-14-docs-readme-raiz
- Data: 2026-11-14
- Versão: unreleased
- Tipo: Changed
- Compatibilidade: Compatível (só documentação; nenhum código alterado)

## Resumo

O `README.md` raiz (que continha só o stub do template GitHub) passa a
descrever o RHP de verdade: requisitos, build/teste/uso, CLI, resumo da
linguagem, estrutura do repo, fluxo seguro e licença.

## Motivação

Porta de entrada do repositório estava vazia; quem chega pelo GitHub precisa
do essencial sem garimpar specs.

## O que mudou

- `README.md`: reescrito (comandos verificados, tabela de crates, exemplo,
  regras centrais, licença `MIT OR Apache-2.0`).

## Detalhes técnicos

Conteúdo restrito ao implementado e validado (93 testes verdes, `fib.rhp`
canônico). Sem alegação sobre itens planejados (aponta `ROADMAP.md`/ADR 0004).

## Compatibilidade e migração

Compatível. Migração: não necessária.

## Segurança e performance

Não aplicável (documentação).

## Testes e validação

- `cargo fmt --check` e suíte passam (hook pre-commit executado no commit).
- Comandos do README conferidos contra a CLI real (`run`, `fmt --check`).

## Limitações e riscos conhecidos

README é resumo; contrato normativo segue nas specs.

## Referências

Sem RFC/ADR (documentação de estado). Autorização: pedido direto no chat.
