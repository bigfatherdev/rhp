# RHP — PROMPT DE INÍCIO DO PROJETO (v2)

Atue como engenheiro principal responsável pela criação da linguagem RHP, uma evolução independente do PHP, com compilador, bytecode, VM e runtime desenvolvidos em Rust.

Este prompt cobre a **Fase 1** (fundação + primeira fatia vertical). A Fase 2 só começa após o relatório final da Fase 1 e aprovação do responsável pelo projeto.

---

## 1. Governança obrigatória

Antes de escrever qualquer código:

1. Procure `RHP_GOVERNANCE.md`. **Se não existir, pare e avise o responsável.** Não invente uma governança.
2. Leia-o integralmente e copie-o para a raiz do projeto como `GOVERNANCE.md`, preservando o conteúdo.
3. Crie `AGENTS.md` instruindo todos os agentes a ler e respeitar `GOVERNANCE.md`. As regras permanentes de execução (seção 7) devem constar também no `AGENTS.md`.
4. Não altere nem enfraqueça a governança sem proposta formal e aprovação do responsável.
5. Consulte as especificações e ADRs relevantes antes de cada tarefa.

---

## 2. Histórico obrigatório de mudanças

Crie ou verifique a pasta `changes/` na raiz do repositório. Se não existir, crie-a, com um `README.md` contendo o modelo de registro definido em `GOVERNANCE.md`.

A partir da primeira alteração incorporada ao projeto, cada correção, recurso, refatoração relevante, otimização, atualização de dependência, mudança de especificação ou alteração de infraestrutura deve ter seu próprio arquivo em `changes/`.

Padrão de nome: `changes/YYYY-MM-DD-<tipo>-<slug>.md`

- Use a data real do sistema (`date +%F`). Nunca invente datas.

Cada registro deve explicar:

- O que mudou para o usuário.
- Por que a mudança foi necessária.
- Componentes e arquivos afetados.
- Impacto de compatibilidade ou migração.
- Testes executados e seus resultados reais.
- Riscos ou limitações que permanecem.
- Referências a RFC, ADR ou commit, quando aplicável.

Não invente versões, testes, métricas ou resultados.

`changes/` é o histórico detalhado e permanente. `CHANGELOG.md` é o resumo consolidado por release, agrupado em Added, Changed, Fixed, Deprecated, Removed, Security e Performance. Um não substitui o outro.

Na Fase 1, apenas **avalie e proponha** (no `ARCHITECTURE.md` ou em ADR) uma verificação de CI que impeça mudanças relevantes sem registro em `changes/`, com exceções explícitas e justificadas. A implementação do CI fica para a Fase 2.

---

## 3. Visão da linguagem (contexto, fora do escopo da Fase 1)

O RHP deverá, ao longo de várias fases:

- Ter sintaxe familiar ao PHP, mas sem depender do PHP.
- Ter tipagem moderna e inferência.
- Possuir funções, classes, interfaces, módulos e generics.
- Ter async/await e concorrência segura.
- Usar compilador e runtime próprios em Rust, priorizando bytecode e VM própria.
- Possuir biblioteca padrão e CLI próprias.
- Evoluir para HTTP, PostgreSQL, Redis e ferramentas de desenvolvimento.

**Não implemente nada disso na Fase 1.** Especifique, implemente e valide por etapas.

---

## 4. Decisões técnicas da Fase 1

Estas decisões são obrigatórias. Se achar que alguma deve mudar, registre a proposta em ADR e peça aprovação antes de divergir.

- **Edição Rust:** 2021 ou superior, toolchain stable.
- **Workspace Cargo** com os crates:
  - `rhp-lexer`
  - `rhp-parser` (contém a AST)
  - `rhp-bytecode`
  - `rhp-vm`
  - `rhp-cli` (binário `rhp`)
- **Lexer e parser escritos à mão.** Sem geradores de parser (pest, lalrpop, nom, etc.).
- **VM baseada em pilha** (stack-based), com instruções simples e um `enum` de opcodes documentado.
- **Valores:** `enum Value` simples (ex.: `Null`, `Bool`, `Int`, `Float`, `Str`). Sem GC nem contagem de referência na Fase 1; qualquer decisão sobre modelo de memória fica para ADR na Fase 2.
- **`unsafe` proibido** na Fase 1. Se for indispensável, justifique em ADR antes.
- **Dependências externas:** mínimas. Qualquer crate de terceiros precisa de justificativa registrada em `changes/`.
- **Erros:** tipos de erro próprios por crate, com mensagens contendo posição (linha/coluna) quando aplicável. Sem `unwrap()`/`expect()` em código de produção fora de testes.

---

## 5. Entrega da Fase 1

Inspecione o ambiente e o repositório. Não modifique outros projetos.

### 5.1 Documentação fundadora (enxuta e honesta)

Crie:

- `GOVERNANCE.md`
- `AGENTS.md`
- `ARCHITECTURE.md` (crates, fluxo lexer → parser → bytecode → VM, decisões da seção 4)
- `ROADMAP.md` (Fase 1 concluída/parcial, Fase 2 e seguintes como plano)
- `CHANGELOG.md`
- `changes/README.md`
- `LANGUAGE_SPEC.md`, `BYTECODE_SPEC.md` e `VM_SPEC.md` — **somente o que foi de fato implementado** (hello world, opcodes usados). Marque o restante como "não especificado ainda".
- `MEMORY_MODEL.md` e `PHP_DIFFERENCES.md` — arquivos curtos com estrutura e seção "A definir"; sem preencher com conteúdo genérico ou especulativo.

Regra: documentação descreve o que existe ou é decisão registrada. Nada de texto de enchimento.

### 5.2 Primeira fatia vertical

1. Arquivo `hello.rhp` (ex.: `echo "Hello from RHP!";`).
2. Lexer.
3. Parser.
4. AST.
5. Geração de bytecode.
6. Execução na VM.
7. CLI: `rhp run hello.rhp`.
8. Testes automatizados.

Saída esperada, exatamente:

```
Hello from RHP!
```

### 5.3 Critérios de aceite

A Fase 1 só pode ser declarada concluída se **todos** passarem e os resultados reais forem relatados:

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- Teste de integração que executa `rhp run hello.rhp` e compara a saída com `Hello from RHP!`.
- Testes unitários por crate (lexer, parser, geração de bytecode, VM), incluindo ao menos um caso de erro (ex.: string não terminada, `;` ausente).

Registre a implementação inicial em um arquivo próprio dentro de `changes/`.

---

## 6. Marco 2 (planejado, NÃO implementar agora)

Apenas registre no `ROADMAP.md`: variáveis, aritmética, comparações, `if`/`else`, `while`, funções e `echo` de expressões.

---

## 7. Regras de execução (permanentes; replicar no `AGENTS.md`)

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

---

## 8. Relatório final

Informe:

- Arquitetura e estrutura criadas.
- Funcionalidades realmente implementadas.
- Arquivos de governança e especificação criados.
- Registro individual criado em `changes/`.
- Testes executados, comandos e resultados reais.
- Testes não executados e motivos.
- Riscos e limitações.
- Estado do Git.
- Próximo marco recomendado.

Comece inspecionando o ambiente e estabelecendo a fundação do RHP. Não declare o projeto ou a linguagem concluídos; declare somente o que foi efetivamente implementado e validado.
