# [Added] — RFC 0002: concorrência e servidor HTTP

- ID: 2026-11-15-docs-rfc-servidor
- Data: 2026-11-15
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (só documento)

## Resumo

`RFCs/0002-concorrencia-servidor.md` (estado: proposta): thread-per-conexão
std, nada compartilhado entre requests (compilar-na-worker a partir de
`Arc<Program>`), `http_serve(porta, handler, opts)` com handlers stateless,
500 sem vazar, `exit` encerrando o servidor.

## Motivação

Fase C1 do plano de sistemas: decidir o runtime antes de codificar o
servidor (isolamento entre requests é decisão arquitetural, não detalhe).

## O que mudou

- `RFCs/0002-concorrencia-servidor.md` (novo): problema, modelo,
  proposta de API, alternativas (async próprio / closures / chunk
  compartilhado / arquivo-handler — todas rejeitadas com motivo),
  compatibilidade, segurança, performance, planos de implementação e
  testes, migração e critérios de aceitação.

## Compatibilidade e migração

Nenhuma (documento). Implementação (C2) após aceite.

## Segurança e performance

Sem impacto (documento). Requisitos registrados para C2 (sem `unsafe`,
sem `Send` manual, limites anti-DoS, 500 sem vazamento).

## Testes e validação

Etapa de documento (C2 traz a matriz da §10). `check-changes.sh` ok.

## Limitações e riscos conhecidos

RFC em estado `proposta`: implementar antes do aceite seria violação —
C2 aguarda aprovação explícita.

## Referências

Plano Fase C. Reabre-se se async virar requisito com caso de uso.
