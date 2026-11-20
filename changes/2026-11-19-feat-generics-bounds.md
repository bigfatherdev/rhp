# Added — Bounds de generics com checagem em compilacao (RFC 0029)

- ID: 2026-11-19-feat-generics-bounds
- Data: 2026-11-19
- Versão: 0.5.0
- Tipo: Added
- Compatibilidade: Compatível com ressalvas (novos erros de compilacao so para codigo sem sentido ou com nomes errados; ver Migracao)

## Resumo

Parametros de tipo ganham bounds (`<T: Classe|Interface>`) verificados em
compilacao no uso (`new`/`call` com type args). Apagamento mantido: zero
custo em runtime.

## Motivação

Item "bounds de generics" das limitacoes: sem bounds, erros so apareciam
dentro do corpo generico, longe do contrato. RFC 0029 (proposta) virou
implementacao do nucleo.

## O que mudou

- Parser: `GenericParam { name, bound }`; bounds qualificaveis
  (`V: App\Modelo`); `CallData`/`NewData` com `type_args` preservados;
  `Expr::{Call,New}` encaixotados (reduz `sizeof(Expr)`, preserva o guarda
  de profundidade — sem regressao nos testes de nesting).
- Compilador: tabelas de generics (classes + funcoes de topo), escopo de
  tipos herdado (adiamento para parametro visivel), `check_type_args`
  (aridade; nao-generica com args = erro; builtin com args = erro; bound
  violado/desconhecido = erro; primitiva contra bound = erro).
- `fmt`: imprime `<T, U: Bound>` e `f<int>(...)` (idempotente, reparse igual).
- Linker renomeia bounds como referencias; testes: parser, bytecode
  (`checks_generic_bounds_at_compile_time`), fmt, integracao
  (`nextgen_generic_bounds`).

## Detalhes técnicos

`GenericParam` em `rhp-parser`; `ClassInfo.generics`, `functions`,
`type_params` e helpers (`check_bound_known`, `is_subclass_of`,
`class_implements`, `check_type_args`) em `rhp-bytecode`; `CallData`/`NewData`
propagados em fmt/cli/http_server/diagnostico (sem quebras: suite verde).

## Compatibilidade e migração

Chamadas sem type args nao mudam nada (apagadas). Novos erros so para:
aridade divergente, nao-generica com args, builtin com args, bound violado
ou desconhecido, tipo desconhecido no uso (inclui primitivas). Documentado
como `Changed` menor na RFC 0029.

## Segurança e performance

So compilacao (sem valores novos, sem recursion extra alem do parse
limitado por `MAX_PARSE_DEPTH`); zero custo em runtime (apagamento).

## Testes e validação

- `checks_generic_bounds_at_compile_time` (7 cenarios), parser
  (`parses_generic_bounds_and_type_args`), fmt idempotente, integracao
  `nextgen_generic_bounds` (satisfeito/viola/interface/adiado).
- Suite integral verde; `fmt --check` e `clippy` limpos; nesting depth ok.

## Limitações e riscos conhecidos

Bounds de um tipo (sem `A+B`); sem bounds em metodos individuais (`fn m<T>`);
`where` adiado; sem inferencia/especializacao. Ver RFC 0029.

## Referências

RFC 0029 (aceita e implementada); `LANGUAGE_SPEC.md` §12; testes citados.
