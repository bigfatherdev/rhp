# [Docs] — Remove governança duplicada

- ID: 2026-11-16-docs-remover-governanca-duplicada
- Data: 2026-11-16
- Versão: unreleased
- Tipo: Docs (Removed)
- Compatibilidade: Compatível (só arquivos `.md` de governança; nenhum
  código, spec normativa ou comportamento alterado)

## Resumo

Exclui `RHP_GOVERNANCE.md`, byte-idêntico a `GOVERNANCE.md` e sem
relevância própria. Inventário completo (123 `.md`): demais arquivos
(specs, `docs/`, `reports/`, RFCs, ADRs, `changes/`) têm função e foram
mantidos. (`RHP_PROMPT_INICIAL_v2.md` permanece excluído, conforme
decisão do responsável — sem restore.)

## Motivação

Pedido do responsável: limpar markdowns sem relevância no
desenvolvimento do RHP.

## O que mudou

- `RHP_GOVERNANCE.md` excluído (`git rm`); `AGENTS.md:9` perde a nota
  "(copiado de `RHP_GOVERNANCE.md`)" — autoridade passa a ser só
  `GOVERNANCE.md`, sem mudar a ordem de autoridade (§1).

## Detalhes técnicos

- Verificado antes de excluir: `diff -q` idênticos; única referência
  ativa era `AGENTS.md:9` (ajustada); a outra citação vive em registro
  histórico de `changes/` (não reescrito, por §21.6).
- `GOVERNANCE.md` (o canônico, citado na ordem de autoridade) intacto.

## Compatibilidade e migração

Compatível; migração não necessária.

## Segurança e performance

Sem impacto (só documentação).

## Testes e validação

- `tools/check-changes.sh origin/main`: ok (registro este).
- Sem código alterado; suite completa rodada pelo hook de pre-commit
  no commit.

## Limitações e riscos conhecidos

Nenhum: a duplicata excluída é recuperável pelo Git se preciso.

## Referências

- `GOVERNANCE.md` §1 (ordem de autoridade).
