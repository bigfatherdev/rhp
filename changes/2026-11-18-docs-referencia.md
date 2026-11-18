# [Docs] — Referência completa da linguagem (`docs/REFERENCIA.md`)

- ID: 2026-11-18-docs-referencia
- Data: 2026-11-18
- Versão: unreleased
- Tipo: Docs
- Compatibilidade: Compatível (só documentação nova)

## Resumo

Novo `docs/REFERENCIA.md`: panorama consolidado da linguagem e da
biblioteca (74 builtins com assinatura e efeito, 10 tipos + `type()`,
sintaxe, operadores, controle de fluxo, funcoes/closures, tipos,
classes, geradores, async, modulos/pacotes, erros+códigos E0xxx–E3xxx,
CLI completa, exemplo comentado, divergencias honestas do PHP).

## Motivação

Pedido do usuário: "todas as funcoes e informacoes do RHP em um markdown".

## O que mudou

- `docs/REFERENCIA.md` (novo, ~400 linhas). Toda afirmacao de
  comportamento verificada contra implementacao/testes antes de
  publicar: o processo achou e corrigiu 2 erros ainda no rascunho
  (exemplo `map`+`inc` com soma errada; `a == a` falso — ver
  `changes/2026-11-18-fix-eq-novos-tipos.md`).

## Compatibilidade e migração

Nenhuma (só docs).

## Segurança e performance

N/A.

## Testes e validação

- Exemplos do documento executados contra o binario (`map`+`inc`,
  geradores, `==`); saidas conferidas e corrigidas no texto.
- `cargo test --workspace`: 324 passed, 0 failed (regressao zero).

## Limitações e riscos conhecidos

Documento e um panorama, nao a norma: em divergencia, valem as
specs (`LANGUAGE_SPEC.md` etc.) — declarado no cabecalho do arquivo.

## Referências

- Todas as RFCs 0001–0020; `LANGUAGE_SPEC.md`; `docs/DOCKER.md`.
