# Docs — Arquitetura de concorrencia permanente + padroes (`docs/ASYNC.md`)

- ID: 2026-11-19-docs-async-arquitetura
- Data: 2026-11-19
- Versão: 0.5.0
- Tipo: Docs
- Compatibilidade: Compatível (so documentacao)

## Resumo

Item "globais mutaveis" das limitacoes: documentado como arquitetura
permanente (valores `Rc`, nao `Send`/`Sync`; compartilhar exigiria
redesenho da `Value`), com tabela de fronteira e padroes (fan-out/in,
timeout com reaproveitamento, cancelamento, agregacao/corrida, metodos
async snapshot, identidade do `await`).

## Motivação

Fechar o item sem fingir solucao: explicar o porquê, os limites exatos e
como compor sistemas reais dentro do modelo.

## O que mudou

- `docs/ASYNC.md` (novo): modelo em uma frase, tabela do que atravessa,
  justificativa tecnica da decisao, 6 padroes com codigo, limites e operacao.
- `docs/REFERENCIA.md`: secao de concorrencia atualizada (modelo, tabela
  com os 5 novos builtins, contagem 74 -> 80).

## Detalhes técnicos

Apenas markdown; nenhum comportamento alterado.

## Compatibilidade e migração

Nao se aplica.

## Segurança e performance

N/A (docs). Reafirma: 1 thread/tarefa, junte sempre, sondas em lacos longos.

## Testes e validação

N/A (docs). Exemplos do guia sao padroes ja cobertos pelos testes `nextgen_async_*`.

## Limitações e riscos conhecidos

Reavaliar compartilhamento real exigiria RFC propria com prova de seguranca
de threads (redesenho de `Value`).

## Referências

RFCs 0019/0023/0030; `docs/ASYNC.md`.
