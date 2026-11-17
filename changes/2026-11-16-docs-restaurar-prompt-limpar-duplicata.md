# [Docs] — Restaura prompt inicial, remove governança duplicada

- ID: 2026-11-16-docs-restaurar-prompt-limpar-duplicata
- Data: 2026-11-16
- Versão: unreleased
- Tipo: Docs (revert + Removed)
- Compatibilidade: Compatível (só arquivos `.md` de governança; nenhum
  código, spec normativa ou comportamento alterado)

## Resumo

Reverte o commit `5ef8b9a` (restaura `RHP_PROMPT_INICIAL_v2.md`) e
exclui `RHP_GOVERNANCE.md`, que era byte-idêntico a `GOVERNANCE.md`
sem relevância própria. Inventário completo (123 `.md`): demais
arquivos (specs, `docs/`, `reports/`, RFCs, ADRs, `changes/`) têm
função e foram mantidos.

## Motivação

Pedido do responsável: desfazer a deleção do prompt inicial e limpar
markdowns sem relevância no desenvolvimento do RHP.

## O que mudou

- `git revert -n 5ef8b9a`: `RHP_PROMPT_INICIAL_v2.md` restaurado
  (7.556 bytes, conteúdo original intacto).
- `RHP_GOVERNANCE.md` excluído (`git rm`); `AGENTS.md:9` perde a nota
  "(copiado de `RHP_GOVERNANCE.md")" — autoridade passa a ser só
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

Nenhum: revert preserva histórico (sem reescrita do `main`); a
duplicata excluída é recuperável pelo Git se preciso.

## Referências

- Commit revertido: `5ef8b9a` ("Delete RHP_PROMPT_INICIAL_v2.md");
- `GOVERNANCE.md` §1 (ordem de autoridade).
