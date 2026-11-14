# changes/ — Histórico detalhado e auditável do RHP

Conforme `GOVERNANCE.md` (seção 21), cada mudança efetiva incorporada ao projeto
(correção, recurso, refatoração relevante, otimização, segurança, deps, sintaxe/
semântica, bytecode/VM/memória, config de build/execução) deve ter seu próprio
arquivo aqui.

Não é necessário registro para experimentos temporários locais não incorporados.
Se incorporado, exige registro.

## Convenção de nomes

`changes/YYYY-MM-DD-<tipo>-<slug>.md`

- Use a data real do sistema (`date +%F`). Nunca invente datas.
- `<tipo>`: `fix`, `feat`, `perf`, `security`, `deps`, `refactor`, `docs`, `tooling`, etc.
- Se colidir slug no mesmo dia, acrescentar sufixo/sequencial. Nunca sobrescrever.

Exemplos:

- `changes/2026-11-13-fix-parser-nested-generics.md`
- `changes/2026-11-13-feat-async-functions.md`

## Modelo obrigatório

```markdown
# [Tipo] — Título da mudança

- ID: YYYY-MM-DD-tipo-slug
- Data: YYYY-MM-DD
- Versão: unreleased
- Tipo: Fixed
- Compatibilidade: Compatível

## Resumo
Descrição clara da mudança para usuários.

## Motivação
Qual problema ou necessidade originou o trabalho?

## O que mudou
- Mudança 1
- Mudança 2

## Detalhes técnicos
Componentes e comportamento interno afetados.

## Compatibilidade e migração
Impactos e passos necessários, ou justificativa de que não há migração.

## Segurança e performance
Análise de impacto, com medições quando aplicáveis.

## Testes e validação
- `comando`: resultado real.

## Limitações e riscos conhecidos
Pendências que permanecem.

## Referências
RFCs, ADRs, issues e commits relacionados.
```

Conteúdo mínimo por registro (seção 21.3 de `GOVERNANCE.md`):

1. Título e identificador
2. Data
3. Versão afetada (`unreleased` se indefinida)
4. Tipo: Added, Changed, Fixed, Deprecated, Removed, Security, Performance, Refactor ou Tooling
5. Resumo para usuários
6. Motivação
7. Detalhes técnicos
8. Arquivos/áreas afetadas
9. Compatibilidade
10. Migração
11. Segurança
12. Performance
13. Testes (comandos + resultados reais)
14. Limitações e riscos
15. Referências (RFC, ADR, commit)

Relação: `changes/` = histórico detalhado permanente. `CHANGELOG.md` = resumo
consolidado por release (Added, Changed, Fixed, Deprecated, Removed, Security,
Performance). Um não substitui o outro. Não apagar registros após consolidar.
