# Docs — RFCs proposta do item 3 (autoload TOML, bounds, cancel/timeout)

- ID: 2026-11-19-docs-rfcs-fase3
- Data: 2026-11-19
- Versão: 0.4.0
- Tipo: Docs
- Compatibilidade: Compatível (so propostas; nada implementado)

## Resumo

Tres RFCs em status `proposta` para o item 3 das pendencias (cada mudanca
significativa exige RFC antes de implementar, cf. governanca).

## Motivação

`rhp.toml [autoload]` + `use function`, bounds de generics e
cancel/timeout/combinadores async sao decisoes de linguagem com impacto em
compatibilidade — nao podem ser assumidas sem proposta formal.

## O que mudou

- `RFCs/0028-autoload-toml-use-function.md` (proposta): `[autoload]`
  PSR-4 + `use`/`use function` + precedencia + validacao no `check`.
- `RFCs/0029-generic-bounds.md` (proposta): `<T: Classe|Interface>` com
  checagem em compilacao e apagamento mantido.
- `RFCs/0030-async-cancel-timeout.md` (proposta): `await_timeout`,
  `async_cancel`/`async_cancelled` cooperativo, `await_all`/`await_race`,
  sem preempcao.

## Detalhes técnicos

Apenas documentacao; nenhum comportamento alterado.

## Compatibilidade e migração

Nao se aplica (propostas).

## Segurança e performance

Discutidas em cada RFC; sem codigo, sem impacto.

## Testes e validação

N/A (propostas aguardam discussao/aprovacao antes de implementar).

## Limitações e riscos conhecidos

Escopo propositalmente v1 em cada RFC (detalhes na secao de nao-objetivos).

## Referências

RFCs 0028/0029/0030; `GOVERNANCE.md` §17.
