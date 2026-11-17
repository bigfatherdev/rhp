# RFC 0016 — Anotações de tipo com checagem em runtime (Etapa 5)

- ID: RFC-0016
- Data: 2026-11-16
- Estado: aceita (autorização prévia do programa Etapas 3–5) e implementada
- Contexto: Etapa 5, item "tipos". Primeiro sistema de tipos do RHP,
  deliberadamente dinâmico (checagem em runtime, sem inferência).

## 1. Problema

RHP não expressa intenção de tipo: erros de forma só aparecem no uso
(`tipos incompativeis para +`), longe da origem. Falta documentar e
fiscalizar contratos de `let`/`const`, parâmetros e retornos.

## 2. Objetivo e não-objetivos

Objetivo: anotações opcionais `let x: int = 1`, `const N: string`,
`fn f(a: int, b = 1): int`, closures `fn (a: int): int => ...`,
checadas em runtime no vínculo (declaração, argumento após defaults,
retorno). Incompatível = erro de runtime capturável.

Não-objetivos: checagem estática/inferência; `any`/união/genéricos;
anotação em destructuring; tipos de campo/método (Etapa 5, OO);
mudança de semântica de valores (só valida, nunca converte).

## 3. Proposta

- Tipos: `int | float | string | bool | array | mapa | null | funcao`
  (nomes de `type()`; `mapa` em vez de `map` por consistência).
- AST: `TypeTag` (`Copy`); `Param.ann`, `Let/Const.ann`,
  `FnDef/ClosureData.return_ann`; `FuncDef.param_types/return_type`.
- Bytecode: `Op::CheckType(tag)` (topo da pilha, sem desempilhar) após
  `DeclareName` de `let/const` anotado; params checados no `activate`
  (após defaults); retorno checado no `Return` (se anotado).
- Erro: `tipos incompativeis: esperava int, encontrou string` (reusa a
  faixa E3001; capturável por `try` como `{tipo: "runtime", ...}`).
- `fmt` reemite anotações; idempotência mantida.

## 4. Alternativas

- Checagem estática no `compile`: rejeitada (exigiria rejeitar programas
  válidos hoje; breaking; sem prova de decidibilidade com `eval`-like).
- Desugar para `assert(type(x) == ...)`: perde posição/mensagem e
  polui bytecode; rejeitada (erro dedicado e mais barato).
- Coerção implícita (ex.: `"1"` → `1`): rejeitada (RHP não converte
  implícito; `int()` explícito continua o caminho).

## 5. Compatibilidade

Aditivo e opt-in: sem anotação, zero instruções novas (nenhum `CheckType`
emitido). `Program`/CLI/linker inalterados na ausência de tipos.

## 6. Segurança

Sem `unsafe`; checagem é comparação de tag (sem panics). Erro carrega
nomes de tipo + valor encontrado (sem vazar além do valor, como outros
erros de tipo).

## 7. Performance

Sem anotação: +0 instruções. Com anotação: 1 tag-check por vínculo
(O(1), sem alocação). Sem regressão mensurável no bench (alvos sem tipos).

## 8. Plano de implementação

1. Lexer: nada (`int` etc. já são `Ident`).
2. Parser: `: Tipo` em `let`/`const`/`Param`/`fn` (nomeada e anonima);
   `TypeTag::parse`.
3. Bytecode: `Op::CheckType`, `FuncDef` estendido, `activate`/params.
4. VM: `check_type`, checagem no `Return`; `Display`/`type()` intactos.
5. Fmt + CLI linker (Renamer passa `ann`).
6. Testes + `LANGUAGE_SPEC` + exemplo.

## 9. Plano de testes

- Cada tipo aceita/rejeita (8 x 2) + mensagem exata.
- Param após default, retorno anotado, closure, `try` captura.
- Defaults inválidos na DEFINIÇÃO passam (só o vínculo checa)? Não:
  default e avaliado e vinculado no `activate` → checado. Pinado.
- Sem anotação: suite inteira inalterada (regressão zero).

## 10. Migração e rollout

Nenhuma migração (opt-in). Rollout normal.

## 11. Critérios de aceitação

- [x] RFC aceita antes do código (autorização prévia 2026-11-16).
- [ ] 8 tipos x aceitar/rejeitar pinados; `activate`/`Return`/`let`.
- [ ] Suite verde + fmt idempotente com anotações.
- [ ] Spec + exemplo + `changes/` + CHANGELOG.
