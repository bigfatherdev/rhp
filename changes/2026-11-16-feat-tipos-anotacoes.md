# [Added] — Anotações de tipo com checagem em runtime (RFC 0016, Etapa 5)

- ID: 2026-11-16-feat-tipos-anotacoes
- Data: 2026-11-16
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (opt-in; sem anotação, zero instruções novas)

## Resumo

`let x: int`, `const N: string`, `fn f(a: int, b = 1): int`,
closures anotadas — 8 tipos (`int|float|string|bool|array|mapa|null|funcao`),
checados em runtime no vínculo (declaração, argumento pós-defaults,
retorno). Erro `tipos incompativeis: esperava T, encontrou V` (E3001,
capturável). `fmt` reemite; `examples/tipos.rhp` roda em CI.

## Motivação

Etapa 5, item "tipos" (RFC 0016, aceita sob autorização prévia do
programa): documentar e fiscalizar contratos perto da origem, sem
mudar semântica de valores (só valida, nunca converte).

## O que mudou

- Parser: `TypeTag` (`Copy`), `Param.ann`, `Let/Const.ann`,
  `FnDef/ClosureData.return_ann`, `parse_type_ann` (`:` + `Ident`,
  `null` como keyword). `mapa`/`funcao` seguem `type()`.
- Bytecode: `Op::CheckType(tag)` (espia o topo; validador: altura
  inalterada) emitido após o `init` e antes do `DeclareName`;
  `FuncDef.param_types/return_type`; `compile_func_body` recebe o
  retorno; fuzz `fuzz_op` cobre a nova op (braço 47).
- VM: `check_type` (O(1), sem alocação); `activate` checa params
  vinculados; `Return` checa contra `return_type`; `VmError` inalterado
  (mensagem com prefixo `tipos incompativeis` → E3001 existente).
- Fmt: reemite `: tipo` (let/const/params/retorno/closure); idempotente.
- Linker (Renamer): propaga `ann`/`return_ann` (renomeação não apaga tipos).
- `LANGUAGE_SPEC.md` (§2 gramática + regra), `examples/tipos.rhp`,
  `CHANGELOG.md` (este registro).

## Compatibilidade e migração

Opt-in; nenhuma migração. Tipos `pub` novos (`TypeTag`, campos) são
aditivos. `static`/destructuring sem anotação (não-objetivo declarado).

## Segurança e performance

Sem `unsafe`; checagem sem panics (topo ausente = erro). Performance:
sem anotação, +0 instruções; com anotação, 1 tag-check por vínculo.
Bench: alvos sem tipos, sem regressão (suite 309 verdes inclui
conformidade que roda o exemplo novo).

## Testes e validação

- `checks_type_annotations` (VM): 8 tipos aceitam, mismatches com
  mensagem exata (`esperava int`), nulo implícito falha, params/defaults,
  retorno, closure, `try` preserva mensagem, sem anotação inalterado.
- `preserves_type_annotations` (fmt): 4 formas + idempotência.
- `cargo test --workspace`: 309 passed, 0 failed; `fmt --check` limpo;
  `clippy -D warnings` 0 warnings.
- `examples/tipos.rhp`: `11, 3, rhp, 100, 42`, exit 0 (entra no
  `rhp_run_all_examples_succeed`).

## Limitações e riscos conhecidos

- Runtime apenas (sem estática/inferência); `any`/união/generics fora.
- `for`/`static`/destructuring sem anotação; `CheckType` não conta no
  teto de heap como metadado (só valores vivos contam).
- Comentários dentro de `fn (a: int /*c*/)`: preservados via próximo
  statement (limitação geral da Etapa 4, documentada).

## Referências

- RFC 0016 (`RFCs/0016-tipos-anotacoes.md`, aceita e implementada);
- `LANGUAGE_SPEC.md` §2; `VM_SPEC.md` (inalterado — semântica de valores intacta).
