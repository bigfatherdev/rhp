# RHP — Regras de Governança, Atualização e Manutenção da Linguagem

**Documento:** `GOVERNANCE.md`  
**Projeto:** RHP (nome provisório)  
**Status:** Documento fundador  
**Aplicação:** obrigatória para toda implementação, correção, refatoração, atualização e release.

---

## 1. Objetivo e autoridade deste documento

Este documento define como o RHP deve evoluir sem comprometer a linguagem, o compilador, a VM, o runtime, as ferramentas e os programas já existentes.

Estas regras são obrigatórias para desenvolvedores humanos e agentes de IA, incluindo o Muse utilizado pelo OpenCode.

A ordem de autoridade dos documentos deve ser:

1. `GOVERNANCE.md`: regras de mudança, compatibilidade, qualidade e releases.
2. `LANGUAGE_SPEC.md`: comportamento normativo da linguagem.
3. `BYTECODE_SPEC.md`: formato e semântica do bytecode.
4. `VM_SPEC.md`: comportamento normativo da máquina virtual.
5. `MEMORY_MODEL.md`: regras de memória e ciclo de vida.
6. `ARCHITECTURE.md`: arquitetura e limites internos.
7. ADRs: decisões arquiteturais aprovadas.
8. `ROADMAP.md`: planejamento e estado de implementação.
9. Código-fonte e testes: implementação executável das especificações.

Se houver contradição, não escolha silenciosamente uma interpretação. Identifique o conflito, explique o impacto e proponha a atualização coordenada dos documentos antes de alterar o comportamento.

O código não deve ser tratado automaticamente como especificação correta: ele pode conter bugs.

## 2. Princípios inegociáveis

Toda mudança deve respeitar estes princípios:

- Correção antes de velocidade de implementação.
- Compatibilidade antes de conveniência.
- Segurança antes de micro-otimizações.
- Semântica explícita e documentada.
- Mudanças pequenas, revisáveis e testáveis.
- Nenhuma funcionalidade pode ser declarada pronta sem evidências.
- Nenhum teste pode ser removido ou enfraquecido apenas para fazer a suíte passar.
- Nenhum erro pode ser ocultado para produzir uma aparência de sucesso.
- Nenhuma alteração incompatível pode entrar sem processo formal de breaking change.
- Não implementar recursos especulativos sem necessidade, especificação e critérios de aceitação.

## 3. Classificação obrigatória de mudanças

Antes de modificar o projeto, classifique a mudança:

### 3.1 Correção de bug

Corrige um comportamento contrário à especificação atual.

Obrigatório:
1. Reproduzir o bug com um teste que falhe antes da correção.
2. Identificar a causa raiz.
3. Corrigir a menor área possível.
4. Adicionar regressão.
5. Executar testes relacionados e a suíte aplicável.
6. Atualizar documentação se o comportamento documentado estiver incorreto.

Uma correção não pode alterar deliberadamente a semântica pública. Se alterar, deixa de ser uma simples correção e deve passar pelo processo de mudança incompatível.

### 3.2 Melhoria interna / refatoração

Altera a implementação sem alterar comportamento observável.

Obrigatório:
- Demonstrar que a semântica pública foi preservada.
- Manter testes de conformidade.
- Não misturar refatoração ampla com novos recursos sem justificativa.
- Comparar benchmarks quando houver impacto potencial em desempenho.
- Registrar ADR se houver mudança arquitetural relevante.

### 3.3 Novo recurso compatível

Adiciona capacidade sem invalidar programas válidos existentes.

Obrigatório:
- Especificação antes ou junto da implementação.
- Definição de sintaxe, semântica, tipos e erros.
- Testes positivos e negativos.
- Documentação e exemplos.
- Análise de interação com recursos existentes.
- Atualização do roadmap.

### 3.4 Mudança incompatível (breaking change)

Altera sintaxe, semântica, ABI, bytecode, runtime, biblioteca padrão ou comportamento de forma que programas, pacotes ou ferramentas existentes possam quebrar.

Exige:
1. Proposta formal de mudança (RFC).
2. Justificativa técnica e alternativas consideradas.
3. Inventário de código e recursos afetados.
4. Estratégia de migração.
5. Política de depreciação, quando viável.
6. Atualização de especificações e testes.
7. Plano de versão principal.
8. Aprovação explícita do responsável pelo projeto antes de implementação definitiva.

Agentes de IA não podem aprovar autonomamente uma breaking change.

## 4. Processo padrão para qualquer atualização

Toda tarefa de manutenção deve seguir esta sequência:

1. **Ler o contexto:** `GOVERNANCE.md`, especificações relevantes, ADRs e estado Git.
2. **Inspecionar:** localizar implementação, dependências, chamadas e testes relacionados.
3. **Classificar:** bug, refatoração, recurso compatível ou breaking change.
4. **Definir escopo:** listar arquivos, módulos e contratos que podem ser afetados.
5. **Criar evidência:** reproduzir o problema ou escrever testes de aceitação.
6. **Planejar:** propor a menor alteração segura.
7. **Implementar:** manter mudanças focadas e coerentes.
8. **Verificar:** formatar, compilar, lintar e testar.
9. **Revisar:** procurar regressões, riscos de segurança e efeitos colaterais.
10. **Documentar:** atualizar especificações, changelog, roadmap e ADRs necessários.
11. **Registrar:** apresentar diff, resultados reais e limitações.
12. **Checkpoint:** criar commit somente após validação e conforme autorização do responsável.

Não avance se uma etapa obrigatória falhar. Não declare sucesso parcial como conclusão integral.

## 5. Especificação da linguagem como contrato

`LANGUAGE_SPEC.md` é o contrato normativo para quem implementa compiladores, runtimes, ferramentas e bibliotecas RHP.

Para cada recurso, documentar:

- Gramática e exemplos válidos.
- Exemplos inválidos.
- Regras de escopo e resolução de nomes.
- Tipos envolvidos e conversões permitidas.
- Ordem de avaliação.
- Semântica de execução.
- Erros de compilação e runtime.
- Interação com funções, módulos, concorrência e memória.
- Compatibilidade e versão em que foi introduzido.

Não introduzir comportamento implícito ambíguo. Em caso de dúvida, preferir regras previsíveis e diagnósticos claros.

Toda alteração de semântica deve atualizar a especificação e os testes de conformidade na mesma mudança.

## 6. Compatibilidade e versionamento

Adotar Semantic Versioning para releases públicas, respeitando a maturidade real do projeto.

- `0.y.z`: fase inicial; APIs e linguagem ainda podem evoluir, mas mudanças devem ser documentadas e testadas.
- `1.0.0` ou superior: contrato público estável.
- PATCH: correções compatíveis.
- MINOR: novos recursos compatíveis.
- MAJOR: mudanças incompatíveis.

Antes da 1.0, não usar a flexibilidade de `0.x` como justificativa para quebrar tudo sem aviso. Manter notas de migração para mudanças que afetem programas existentes.

Definir separadamente as versões de:
- Linguagem RHP.
- Compilador/CLI.
- Bytecode e formato de módulos.
- Runtime/VM.
- Biblioteca padrão.

Quando esses componentes forem distribuídos juntos, documentar a matriz de compatibilidade e evitar combinações incompatíveis.

O compilador deve informar claramente a versão da linguagem utilizada e rejeitar versões ou recursos não suportados, em vez de interpretá-los silenciosamente de outra forma.

## 7. Política de depreciação

Antes de remover uma API, sintaxe ou comportamento público:

1. Marcar como deprecated, quando tecnicamente possível.
2. Explicar o motivo e a alternativa recomendada.
3. Indicar a primeira versão de depreciação.
4. Fornecer diagnóstico útil.
5. Documentar migração.
6. Remover somente na janela de versão principal definida.

Não deprecar algo sem alternativa ou justificativa concreta, salvo risco grave de segurança ou impossibilidade técnica.

## 8. Testes obrigatórios

Manter uma suíte em camadas:

### 8.1 Testes unitários
Lexer, parser, AST, resolução de nomes, tipos, bytecode, VM, memória, runtime e biblioteca padrão.

### 8.2 Testes de conformidade da linguagem
Uma coleção independente de programas RHP com resultado esperado. Deve cobrir:
- Programas válidos.
- Programas inválidos.
- Erros de sintaxe.
- Erros de tipo.
- Escopo e closures.
- Ordem de avaliação.
- Exceções/erros.
- Módulos.
- Memória e ciclo de vida.
- Async/await e concorrência, quando disponíveis.

### 8.3 Testes de integração
Compilador + VM, CLI + runtime, módulos + dependências, HTTP + I/O, PostgreSQL/Redis + pool e transações.

### 8.4 Testes de regressão
Todo bug corrigido deve gerar um caso de regressão que falharia antes da correção.

### 8.5 Fuzzing e propriedades
Aplicar fuzzing ao lexer, parser, leitor de bytecode e componentes expostos a entradas não confiáveis. Usar property-based testing quando apropriado.

### 8.6 Testes de plataforma
Validar Linux e Windows para recursos declarados multiplataforma. Não anunciar suporte a uma plataforma sem execução de testes nela.

### 8.7 Regras sobre testes
- Não apagar testes falhos para obter build verde.
- Não transformar testes em `skip`, `ignore` ou equivalentes sem justificativa aprovada.
- Não reduzir assertions ou cobertura de casos sem explicar o motivo.
- Não usar apenas snapshots como prova de correção sem validar semântica.
- Não afirmar que testes foram executados se não foram.
- Informar comandos, quantidade executada, falhas e limitações reais.

## 9. Segurança e Rust

- Evitar `unsafe`; quando indispensável, isolar em módulos pequenos.
- Todo bloco `unsafe` deve explicar invariantes de segurança e possuir testes específicos.
- Não usar `unwrap()`/`expect()` em caminhos que processam código RHP não confiável sem justificativa.
- Tratar bytecode, módulos e arquivos de pacote como entradas potencialmente maliciosas.
- Validar limites de tamanho, profundidade, recursão, alocação e tempo quando aplicável.
- Prevenir stack overflow, integer overflow, use-after-free, double-free, corrupção de heap e denial of service.
- Definir limites configuráveis para execução de código não confiável.
- Não executar código externo ou comandos do sistema implicitamente.
- Não carregar pacotes sem verificação de origem e integridade.
- Não armazenar segredos em logs, exemplos, testes ou arquivos versionados.
- Revisar dependências quanto a manutenção, licença, vulnerabilidades e necessidade real.

A VM deve validar bytecode antes da execução. Nunca presumir que todo bytecode foi produzido pelo compilador oficial.

## 10. Memória, bytecode e VM

Mudanças no modelo de memória exigem revisão de:
- Ownership lógico dos valores.
- Referências e aliasing.
- Ciclos.
- Finalização/destrutores, se existirem.
- Coleta ou contagem de referências.
- Concorrência.
- Exceções e unwinding.
- Limites de heap e stack.
- Compatibilidade entre versões.

Mudanças no bytecode exigem:
- Atualização de `BYTECODE_SPEC.md`.
- Identificação de versão/formato.
- Validação de instruções e operandos.
- Testes de bytecode inválido e malformado.
- Plano de compatibilidade para módulos compilados anteriormente.

Não alterar opcode, layout ou semântica silenciosamente.

## 11. Performance e benchmarks

Performance deve ser medida, não presumida.

- Manter benchmarks reproduzíveis.
- Registrar hardware, sistema operacional, versão do compilador e configuração.
- Comparar antes/depois em mudanças críticas.
- Medir tempo, memória e comportamento sob concorrência quando relevante.
- Não sacrificar segurança ou correção por ganhos marginais.
- Evitar alegações como “mais rápido que PHP/Rust/Node” sem metodologia pública e comparação justa.
- Não criar otimizações específicas que mudem a semântica.

Uma regressão relevante deve ser investigada e documentada. Definir limites de regressão por componente quando houver baseline confiável.

## 12. Dependências e toolchain

- Fixar versões por `Cargo.lock` para aplicações e CI.
- Preferir versões estáveis do Rust.
- Atualizar dependências em mudanças focadas.
- Não atualizar toda a árvore de dependências junto com uma mudança sem relação.
- Verificar breaking changes, MSRV, licença e advisories.
- Documentar a versão mínima suportada do Rust (MSRV).
- Testar atualização do toolchain em branch ou checkpoint isolado.
- Não remover dependências apenas por preferência estética; medir impacto e verificar substituições.

Atualizações de Rust, crates ou ferramentas devem passar por build, testes, lint e auditoria aplicáveis.

## 13. API pública e biblioteca padrão

Tratar como API pública qualquer elemento que usuários possam importar, chamar, implementar ou depender.

Antes de alterar:
- Assinaturas.
- Tipos públicos.
- Visibilidade.
- Traits/interfaces.
- Erros.
- Ordem de argumentos.
- Comportamento de funções.
- Defaults.
- Formatos serializados.

Verificar usos internos e exemplos. Documentar mudanças e migração. Preferir adições compatíveis e APIs explícitas.

## 14. CLI, formatter, LSP e ferramentas

As ferramentas oficiais fazem parte da experiência da linguagem.

Mudanças devem preservar:
- Mensagens de erro compreensíveis.
- Códigos de saída consistentes.
- Saída estruturada quando oferecida.
- Comportamento documentado dos comandos.
- Compatibilidade de configuração.
- Formatação determinística.
- Integração entre CLI, compilador, VM e LSP.

O formatter deve ser idempotente: formatar duas vezes deve produzir o mesmo resultado da primeira formatação.

## 15. Documentação e changelog

Toda mudança relevante deve atualizar os documentos afetados na mesma entrega.

`CHANGELOG.md` deve separar:
- Added.
- Changed.
- Fixed.
- Deprecated.
- Removed.
- Security.

`ROADMAP.md` deve refletir estado verdadeiro: planejado, em andamento, implementado, validado ou bloqueado.

Não marcar recurso como concluído porque existe apenas um stub, uma interface vazia ou um teste superficial.

Exemplos de documentação devem ser executáveis em CI quando possível.

## 16. Git, branches e checkpoints

- Não trabalhar diretamente em branch protegida.
- Criar branch de tarefa com nome descritivo.
- Manter commits pequenos e com propósito.
- Não sobrescrever trabalho de terceiros.
- Não executar reset destrutivo, clean destrutivo ou rebase forçado sem autorização explícita.
- Inspecionar `git status` e diff antes de alterar ou commitar.
- Não incluir arquivos temporários, segredos, binários ou artefatos não intencionais.
- Não fazer push, publicar pacote ou release sem autorização explícita.

Sugestão de convenção:
- `feat/...`
- `fix/...`
- `refactor/...`
- `docs/...`
- `test/...`
- `perf/...`
- `security/...`

## 17. Processo especial para RFC e ADR

Criar RFC para:
- Novos recursos centrais da linguagem.
- Mudanças incompatíveis.
- Mudanças no modelo de memória.
- Mudanças no formato de bytecode.
- Mudanças na semântica de concorrência.
- Alterações importantes no sistema de tipos.
- Mudanças no modelo de módulos/pacotes.

Criar ADR para decisões arquiteturais duradouras.

Cada RFC deve conter:
1. Problema.
2. Objetivo e não objetivos.
3. Proposta.
4. Alternativas.
5. Compatibilidade.
6. Segurança.
7. Performance.
8. Plano de implementação.
9. Plano de testes.
10. Migração e rollout.
11. Critérios de aceitação.

Estados de RFC: proposta, em discussão, aceita, rejeitada, implementada ou substituída.

## 18. Regras específicas para agentes de IA

Antes de iniciar qualquer tarefa, o agente deve:

1. Ler este arquivo.
2. Ler os documentos relevantes.
3. Inspecionar o estado do repositório e do Git.
4. Identificar a versão atual e o estado real dos testes.
5. Classificar a mudança.
6. Informar escopo e riscos antes de mudanças amplas.

Durante o trabalho:
- Não inventar APIs ou dependências.
- Não supor que uma funcionalidade existe porque aparece no roadmap.
- Não substituir componentes inteiros sem necessidade.
- Não ampliar o escopo sem autorização.
- Não alterar especificação e implementação de forma contraditória.
- Não deixar código morto, stubs ou TODOs críticos sem registro.
- Não contornar erros de compilação ou testes.
- Não modificar arquivos fora do escopo sem explicar.

Ao finalizar, apresentar:
- Resumo da mudança.
- Classificação.
- Arquivos alterados.
- Decisões tomadas.
- Testes executados e resultados reais.
- Testes não executados e motivo.
- Riscos restantes.
- Documentação atualizada.
- Estado do Git.
- Próximo passo recomendado.

Se não houver informação suficiente para uma mudança segura, interromper e perguntar. Não adivinhar decisões de linguagem que afetem compatibilidade.

## 19. Critérios mínimos para considerar uma tarefa concluída

Uma tarefa só pode ser marcada como concluída quando:

- [ ] A implementação corresponde à especificação.
- [ ] O escopo foi respeitado.
- [ ] Testes relevantes foram adicionados ou atualizados.
- [ ] Testes aplicáveis passaram.
- [ ] Não há regressões conhecidas sem registro.
- [ ] Segurança foi considerada.
- [ ] Documentação foi atualizada.
- [ ] Changelog/roadmap foram atualizados quando necessário.
- [ ] Diff foi revisado.
- [ ] Limitações restantes foram declaradas.

Se algum item não puder ser atendido, classificar como parcial ou bloqueado e explicar.

## 20. Regra final

A estabilidade do RHP depende de preservar contratos explícitos.

Nenhuma atualização deve ser feita apenas porque parece moderna, elegante ou mais rápida.

Toda mudança precisa responder:

- Qual problema real resolve?
- Qual contrato pode afetar?
- Como sabemos que está correta?
- Como evitamos regressões?
- Como o usuário migra, se necessário?
- Como reverter se algo der errado?

**O RHP deve evoluir de forma deliberada, verificável e sustentável.**


## 21. Registro obrigatório de cada mudança (`changes/`)

O repositório deverá possuir uma pasta `changes/` na raiz. Ela será o histórico detalhado e auditável de todas as mudanças realizadas no RHP.

Antes de implementar uma tarefa, o agente deverá verificar se `changes/` existe. Se não existir, deverá criá-la. Também deverá verificar se há um modelo de registro; se não houver, deverá criar `changes/README.md` explicando o padrão.

### 21.1 Um registro por mudança

Toda mudança efetiva no código, comportamento, documentação normativa, dependências, compilador, VM, runtime, CLI, biblioteca padrão, testes ou infraestrutura deverá possuir um arquivo próprio dentro de `changes/`.

Isso inclui:
- Correções de bugs.
- Novos recursos.
- Refatorações relevantes.
- Otimizações.
- Correções de segurança.
- Atualizações de Rust, crates e ferramentas.
- Alterações de sintaxe ou semântica.
- Mudanças de bytecode, VM ou memória.
- Alterações de configuração que afetem desenvolvimento, build ou execução.

Não é necessário criar registro para alterações puramente temporárias ou experimentos locais que não sejam mantidos no repositório. Se o experimento for incorporado, deverá receber registro.

### 21.2 Convenção de nomes

Usar nomes ordenáveis e únicos, preferencialmente:

`changes/YYYY-MM-DD-<tipo>-<slug>.md`

Exemplos:
- `changes/2026-11-13-fix-parser-nested-generics.md`
- `changes/2026-11-13-feat-async-functions.md`
- `changes/2026-11-13-perf-vm-dispatch.md`
- `changes/2026-11-13-security-bytecode-validation.md`
- `changes/2026-11-13-deps-rust-toolchain.md`

Se houver várias mudanças no mesmo dia com o mesmo slug, acrescentar um identificador curto ou número sequencial. Nunca sobrescrever um registro anterior.

### 21.3 Conteúdo mínimo de cada registro

Cada arquivo deverá conter:

1. **Título e identificador**
2. **Data**
3. **Versão afetada**, se definida; caso contrário, informar `unreleased`
4. **Tipo:** Added, Changed, Fixed, Deprecated, Removed, Security, Performance, Refactor ou Tooling
5. **Resumo para usuários:** o que mudou em linguagem clara
6. **Motivação:** problema ou necessidade que originou a mudança
7. **Detalhes técnicos:** módulos, componentes e comportamento alterado
8. **Arquivos ou áreas afetadas:** lista dos principais caminhos
9. **Compatibilidade:** compatível, potencialmente incompatível ou breaking change
10. **Migração:** instruções para usuários, ou `não necessária` com justificativa
11. **Segurança:** impacto analisado e resultado
12. **Performance:** impacto medido, não aplicável ou ainda não medido
13. **Testes:** comandos executados e resultados reais
14. **Limitações e riscos conhecidos**
15. **Referências:** RFC, ADR, issue ou commit, quando existirem

Não inventar números de versão, resultados de testes, métricas ou referências.

### 21.4 Modelo obrigatório

O arquivo `changes/README.md` deverá incluir um modelo reutilizável semelhante a:

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

### 21.5 Quando criar e atualizar o registro

- Criar o registro durante a implementação, antes de considerar a tarefa concluída.
- Atualizá-lo se o escopo ou os resultados mudarem.
- Não registrar como concluído algo que ainda está em andamento.
- Se a implementação for abandonada e não entrar no projeto, remover o registro provisório ou marcá-lo claramente como não incorporado, sem deixar que pareça uma mudança publicada.
- O registro deve acompanhar a mesma alteração/commit que introduz a mudança, sempre que possível.

### 21.6 Relação com `CHANGELOG.md` e notas de release

`changes/` será o histórico detalhado, com um documento por mudança.

`CHANGELOG.md` será a visão consolidada por versão, destinada a apresentar o conjunto de alterações de cada release.

No momento de preparar uma release:
1. Revisar todos os registros ainda não publicados.
2. Agrupar as mudanças por Added, Changed, Fixed, Deprecated, Removed, Security e Performance.
3. Produzir notas de release claras, semelhantes às notas de versão de linguagens e runtimes maduros.
4. Incluir impactos incompatíveis e instruções de migração em destaque.
5. Associar cada item ao registro detalhado correspondente.
6. Marcar os registros com a versão efetiva somente depois que a release for aprovada e identificada.

Não apagar os arquivos individuais de `changes/` depois de consolidá-los no changelog. Eles formam o histórico permanente de auditoria.

### 21.7 Verificação automática

Criar, quando viável, uma verificação no CI que detecte mudanças relevantes em código, especificações, dependências ou configuração sem um registro correspondente em `changes/`.

A verificação deve:
- Identificar arquivos alterados no diff.
- Exigir pelo menos um registro de mudança para alterações relevantes.
- Permitir uma exceção explícita e justificada para alterações que não precisam de nota.
- Falhar com mensagem clara quando o registro obrigatório estiver ausente.

Não criar uma verificação frágil que possa ser contornada facilmente ou que exija registros para arquivos temporários irrelevantes. Documentar os critérios e testar a própria verificação.

### 21.8 Regra de auditoria

Em toda entrega, o agente deverá informar:
- Qual arquivo de `changes/` foi criado ou atualizado.
- Qual entrada de `CHANGELOG.md` foi alterada, se aplicável.
- Se a mudança ainda está `unreleased`.
- Se existe impacto de migração ou compatibilidade.

**Nenhuma correção ou atualização incorporada ao RHP deve ficar sem histórico individual em `changes/`.**
