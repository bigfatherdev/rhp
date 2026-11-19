# Added — Metodos async e transporte de instancias (snapshot)

- ID: 2026-11-19-feat-async-metodos
- Data: 2026-11-19
- Versão: 0.5.0
- Tipo: Added
- Compatibilidade: Compatível (aditivo; 1 correcao de fronteira)

## Resumo

Metodos `async fn` em classe/trait: `obj.m()`, destacada e `super.m()`
geram tarefa sobre snapshot do `this` (sem write-back). Instancias cruzam
a fronteira com a classe (envelope `__rhp_instancia__`); chave reservada.

## Motivação

Item "metodos async" das limitacoes: `this` era rejeitado e metodos nao
podiam ser `async`, o que excluía OO da concorrencia.

## O que mudou

- Parser: `async fn` em corpos de classe/trait (`ClassMethod.is_async`).
- VM: `call_method`/`call_super` geram tarefa quando `is_async` (sem
  `pending_method`/`pending_super`: mutacoes nao voltam; documentado).
- Fronteira: `encode_shareable`/`decode_shareable` com envelope de
  instancia; spawn rejeita mapas com a chave reservada; retornos codificam
  e ambos os joins decodificam (instancia volta como instancia, nao mapa).
- Testes: parser (`parses_async_methods`), fmt, integracao
  (`nextgen_async_metodo_snapshot`, `nextgen_async_metodo_trait_super`,
  `nextgen_async_instancia_com_classe`, rejeicao de chave reservada).

## Detalhes técnicos

`crates/rhp-vm/src/lib.rs` (fronteira, spawn, joins, `call_method`,
`call_super`); `rhp-parser` (`ClassMethod.is_async` + parse);
`rhp-bytecode` (repasse da flag); `rhp-fmt` (`async fn` em metodos);
`rhp-cli` (renamer propaga a flag).

## Compatibilidade e migração

Aditivo. Correcao de fronteira: retorno instancia agora decodifica (antes
degradava em mapa no `join`); codigo que dependia do mapa perde (caso
patologico, documentado).

## Segurança e performance

Sem `unsafe`; envelope validado nos dois lados (forma exata + classe
conhecida); sem compartilhamento de memoria (snapshot por copia).

## Testes e validação

Integracao acima + suite integral verde; `fmt --check` e `clippy` limpos.

## Limitações e riscos conhecidos

Sem write-back em async (arquitetura); sem named/spread em chamada async;
`super` async sem write-back; globais mutaveis nao atravessam.

## Referências

RFC 0023 (atualizada); `LANGUAGE_SPEC.md` §10; `docs/ASYNC.md`.
