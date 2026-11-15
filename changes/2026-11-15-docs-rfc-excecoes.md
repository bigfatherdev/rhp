# [Added] — RFC 0001: exceções (`try`/`catch`/`finally`, `throw`)

- ID: 2026-11-15-docs-rfc-excecoes
- Data: 2026-11-15
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (só documento; nenhum comportamento muda)

## Resumo

Primeira RFC do projeto: `RFCs/0001-excecoes.md` (estado: proposta)
desenhando exceções com propagação entre funções, `finally` em todos os
caminhos (`exit()` não aciona, como no PHP) e erro como valor
(mapa `{tipo, mensagem}` para erros internos).

## Motivação

Pré-requisito do plano "RHP para sites/sistemas" (Fase A1): sem
tratamento de erro local, um request inválido derrubaria o servidor
(Fase C). A RFC destrava o desenho antes de qualquer código, conforme
governança §17.

## O que mudou

- `RFCs/0001-excecoes.md` (novo): problema, objetivos, sintaxe,
  semântica (incl. `return`/`throw` em `finally` substituindo o fluxo),
  valor do erro, desenho de bytecode/VM (`TryBegin`/`TryEnd`/`Throw`,
  pilha de handlers por frame), alternativas, compatibilidade,
  segurança, performance, planos de implementação/testes, migração e
  critérios de aceitação.
- `ROADMAP.md`: esqueleto das Fases A–D (sistemas) com A1 marcada.

## Detalhes técnicos

Nenhum código alterado. Decisões registráveis: custo O(1) no caminho
feliz; unwinding limitado por `MAX_CALL_DEPTH` + `max_steps`; validador
de pilha com altura 1 na entrada do handler; 4 novas palavras reservadas
com impacto medido (zero usos como identificador no repo).

## Compatibilidade e migração

Nenhuma (documento). A implementação (A2) seguirá após aprovação da RFC.

## Segurança e performance

Sem impacto (documento). Requisitos registrados para A2 (sem `unsafe`,
sem I/O novo, benchmark do caminho feliz).

## Testes e validação

- `tools/check-changes.sh origin/main`: ok (este registro cobre o diff).
- Sem testes executáveis (etapa de documento; A2 traz a matriz da §9).

## Limitações e riscos conhecidos

RFC em estado `proposta`: implementar antes do aceite seria violação do
processo — A2 aguarda aprovação explícita.

## Referências

GOVERNANCE.md §17. Plano "RHP para sites/sistemas" (Fases A–D).
