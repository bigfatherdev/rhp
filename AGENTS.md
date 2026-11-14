# AGENTS.md — Instruções obrigatórias para agentes (humanos e IA)

> Este arquivo é normativo para operação. A autoridade máxima é `GOVERNANCE.md`.

## 1. Leitura obrigatória

Antes de qualquer tarefa, todo agente DEVE:

1. Ler integralmente `GOVERNANCE.md` (copiado de `RHP_GOVERNANCE.md`).
2. Ler as especificações e ADRs relevantes para a tarefa:
   - `LANGUAGE_SPEC.md`, `BYTECODE_SPEC.md`, `VM_SPEC.md`, `MEMORY_MODEL.md`,
     `ARCHITECTURE.md`, `ROADMAP.md`.
3. Inspecionar o estado do repositório e do Git (`git status`, diff, log).
4. Classificar a mudança (bug / refatoração / recurso compatível / breaking change)
   conforme seções 3 e 4 de `GOVERNANCE.md`.
5. Consultar `changes/README.md` e criar/atualizar o registro individual em `changes/`.

Em caso de contradição entre documentos, NÃO escolher silenciosamente.
Identificar o conflito, explicar o impacto e propor atualização coordenada
(ordem de autoridade: seção 1 de `GOVERNANCE.md`).

## 2. Regras permanentes de execução (réplica da seção 7 do prompt / regras de GOVERNANCE)

- Trabalhe em etapas pequenas e verificáveis.
- Leia a governança antes de cada tarefa.
- Não invente funcionalidades concluídas.
- Não ignore falhas de compilação ou de testes.
- Não enfraqueça testes para obter resultado verde.
- Toda correção inclui teste de regressão.
- Toda mudança semântica atualiza a especificação.
- Toda decisão arquitetural importante tem ADR.
- Não faça breaking changes sem aprovação explícita.
- Não execute operações Git destrutivas (`reset --hard`, `push --force`, `clean -fd`, etc.).
- Não publique releases ou pacotes sem autorização.
- Atualize `ROADMAP.md` e `CHANGELOG.md` quando aplicável.
- Crie o registro individual em `changes/` para toda mudança incorporada.
- Em caso de dúvida sobre escopo ou decisão não coberta aqui, pare e pergunte em vez de assumir.

Regras complementares de `GOVERNANCE.md` (seções 18–19) também valem:

- Não inventar APIs ou dependências.
- Não supor que funcionalidade existe porque está no roadmap.
- Não ampliar escopo sem autorização.
- Não deixar código morto, stubs ou TODOs críticos sem registro.
- Não contornar erros de compilação ou testes.
- Não modificar arquivos fora do escopo sem explicar.
- Não apagar/enfraquecer testes, nem afirmar execução sem evidência real.
- `unsafe` proibido na Fase 1; sem `unwrap()`/`expect()` em produção fora de testes.

## 3. Processo padrão (resumo da seção 4 de GOVERNANCE.md)

1. Ler contexto → 2. Inspecionar → 3. Classificar → 4. Definir escopo →
5. Criar evidência (teste que falha) → 6. Planejar menor alteração segura →
7. Implementar → 8. Verificar (`fmt`, `clippy`, `test`) → 9. Revisar →
10. Documentar (specs, changelog, roadmap, ADR) → 11. Registrar em `changes/` →
12. Checkpoint (commit só com autorização).

## 4. Auditoria (`changes/`)

- Verificar se `changes/` existe; se não, criar.
- Toda mudança incorporada exige arquivo próprio `changes/YYYY-MM-DD-<tipo>-<slug>.md`
  com o conteúdo mínimo da seção 21.3 de `GOVERNANCE.md`.
- Nunca inventar versões, testes, métricas ou resultados.
- Em toda entrega informar: arquivo `changes/` criado, entrada de `CHANGELOG.md`,
  status `unreleased`, impacto de migração/compatibilidade.
