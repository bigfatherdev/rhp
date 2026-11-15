# RFC 0001 — Exceções: `try`/`catch`/`finally` e `throw`

- ID: RFC-0001
- Data: 2026-11-15
- Estado: proposta
- Autores: agente (sessão Fase A) + responsável pelo projeto (aprovação pendente)
- Escopo: linguagem, compilador, bytecode, VM, CLI, specs, diagnósticos

## 1. Problema

O RHP trata todo erro de runtime como falha fatal do programa (`VmError`
aborta a execução; `rhp test` conta como falha de arquivo). Isso impede:

- Tratar erro esperado perto de onde ele ocorre (ex.: `json_parse` de
  entrada externa, `fs_read` de arquivo opcional, `http_get` com rede
  instável) sem encerrar o programa.
- Escrever servidores HTTP (Fase C): um request com dado inválido não
  pode derrubar o processo; o handler precisa isolar a falha e responder
  erro.
- Garantir limpeza (`finally`): fechar recursos lógicos mesmo com erro.

Hoje a única saída é checagem defensiva prévia, que nem sempre é possível
(ex.: condição de corrida entre `fs_exists` e `fs_read`).

## 2. Objetivo e não-objetivos

Objetivo: `try`/`catch`/`finally` + `throw`, com propagação entre funções,
custo zero no caminho sem erro, sem novas dependências, sem `unsafe`.

Não-objetivos desta RFC:

- Hierarquia de classes de erro (sem OO; erro é valor comum, §5).
- Stack traces de runtime (depende do mapa op→fonte do debugger; futuro).
- `try` como expressão (só instrução).
- Cancelamento assíncrono (sem async na linguagem).
- Mudar erros de compilação (continuam fatais antes da execução).

## 3. Proposta

### 3.1 Sintaxe

```text
TryStmt    := "try" Block ("catch" Ident Block)? ("finally" Block)?
             // ao menos um de `catch`/`finally` é obrigatório
ThrowStmt  := "throw" Expr ";"
```

- `try`, `catch`, `finally`, `throw` viram palavras reservadas (impacto
  avaliado em §5; nenhum uso como identificador no repo ou exemplos).
- `catch` sem variável (`catch { }`) é permitido (erro descartado).
- Erro de parse: `try` sem `catch` nem `finally`; `throw` fora de
  expressão válida segue as regras de `Expr`.

### 3.2 Semântica

- `try { B }`: executa `B`. Sem erro, segue após a instrução.
- Erro de runtime em `B` (incluindo `throw v`, `assert` falho, divisão por
  zero, `index` inválido, builtin com erro): procura o handler `try` mais
  interno **na mesma função**; se não houver, desenrola um frame (executa
  seus `finally`s pendentes, §3.4) e repete na chamadora; sem handler em
  nenhum frame, o programa falha como hoje (mesma mensagem, mesmo
  `Error[CODIGO]`).
- `catch VAR { C }` (ou `catch { C }`): `VAR` é declarada no escopo de `C`
  e recebe o **valor do erro** (§3.3); depois de `C`, a execução segue após
  o `try` (a exceção está consumida).
- `finally { F }`: executa sempre ao sair do `try` — caminho normal,
  caminho de `catch`, erro propagado, `return`, `break`/`continue` que
  cruzem o `try`. Valor de `F` é descartado. `return`/`throw` dentro de
  `F` **substituem** o fluxo em curso (documentado; igual ao PHP).
- `throw EXPR;`: avalia `EXPR` e a lança como valor de erro (qualquer
  valor é lançável; convenção: mapas com `mensagem`, §3.3).
- `exit(codigo)`: **não** aciona `finally` (igual ao PHP com `exit`/`die`);
  é saída abrupta do processo, documentada.
- `break`/`continue`/`return` dentro de `try` executam os `finally`s do
  caminho antes de concluir o desvio.
- Erros de compilação, `import` e `exit` não são capturáveis.

### 3.3 Valor do erro

Erros internos da VM passam a carregar valor estruturado: mapa
`{tipo: "runtime", mensagem: "..."}` (chaves ordenadas, como todo mapa).
`catch VAR` recebe esse mapa para erros internos, ou o valor original de
`throw`. Compatibilidade: mensagens de `VmError` e códigos `E3xxx`
permanecem idênticos (o `Display` do mapa em `rhp run` sem handler é a
mensagem de hoje).

### 3.4 Bytecode e VM (desenho)

- Novos ops: `TryBegin(h)` (empilha handler com alvo `h`, profundidade de
  escopo e de frame), `TryEnd` (desempilha), `Throw` (desempilha valor e
  inicia unwinding).
- Cada frame mantém pilha de handlers. `Throw`/erro: descarta operandos
  do frame, restaura escopos até a profundidade do handler, empilha o
  valor do erro, `ip = alvo`. Sem handler no frame: executa `finally`s?
  Não — `finally` é compilado como código nos caminhos de saída (duplicado
  no `catch`/propagação via sub-rotina de `Jump`? Decisão de
  implementação: `finally` vira bloco executado em todos os caminhos com
  uma flag de "em propagação"; A2 detalha, mantendo o validador de pilha
  consistente em cada caminho).
- Validador de fluxo de pilha: caminhos de exceção entram no handler com
  altura 1 (só o valor do erro); junções seguem a regra vigente.
- `VmLimits.max_steps`: cada frame desenrolado conta como instrução (sem
  loop infinito de unwinding).
- Custo no caminho feliz: um push/pop de handler por `try` (O(1)).

### 3.5 Diagnósticos e CLI

- Parse: `E1xxx` existente para `try` malformado (mensagens novas, mesmo
  mapeamento).
- Runtime com handler: sem saída de erro (consumido). Sem handler: mesma
  saída de hoje, incluindo `exit codes`.
- `rhp check`: valida `try` (sintaxe + fluxo de pilha); `rhp test`:
  `throw` não capturado = falha do arquivo (como erro hoje).

## 4. Alternativas consideradas

1. **Só erros como valores + `pode_falhar()`**: exige disciplina manual em
   todo call-site e não resolve limpeza (`finally`); rejeitada.
2. **`catch` sem `finally`**: metade do problema (limpeza) fica sem
   solução; rejeitada (server precisa de `finally` para consistência).
3. **Exceções só intra-função (sem propagação)**: mais simples, mas força
   `try` em toda função da pilha; rejeitada (ergonomia inviável p/ server).
4. **Result<T, E> tipado**: exige sistema de tipos (RFC futura); rejeitada
   por ora — erro como valor dinâmico é compatível com os tipos atuais.

## 5. Compatibilidade

Aditiva, com duas ressalvas mínimas:

- `try`/`catch`/`finally`/`throw` viram reservadas. Busca no repo
  (2026-11-15): zero ocorrências como identificador em `crates/`,
  `examples/`, `docs/`, `tools/`. Programas externos que usem esses nomes
  como variável quebram no parse (erro claro, migração = renomear).
- `exit()` dentro de `try` não roda `finally` (documentado; igual ao PHP).

Nenhum programa válido existente muda de comportamento (suite de
regressão intacta + pino de compatibilidade para o mapeamento de erros).

## 6. Segurança

- Nenhum I/O novo; `throw` não executa nada (só transporta valor).
- Mensagens de erro podem conter caminhos/URLs (como hoje); `catch` que
  imprimir o erro segue as mesmas regras de log (sem segredos embutidos
  pela VM).
- Profundidade de unwinding limitada por `MAX_CALL_DEPTH` + `max_steps`
  (sem DoS por cadeia de handlers).
- Sem `unsafe`; handlers validados pelo verificador (alvo `< code.len()`).

## 7. Performance

- Caminho feliz: O(1) por `try` (sem alocação além do registro).
- `throw`: O(frames desenrolados); sem otimização prematura; benchmark
  antes/depois no `BENCHMARKS.md` (harness existente) em A2.
- Sem impacto no código sem `try` (nenhum op novo no caminho quente).

## 8. Plano de implementação (A2)

1. Lexer: 4 keywords + testes.
2. Parser: `Stmt::{Try, Throw}` + `parse_try/parse_throw` + testes
   (falha-antes).
3. Bytecode: `TryBegin/TryEnd/Throw`, compilação dos caminhos
   (normal/catch/finally/propagação), validador, anti-deriva, testes.
4. VM: pilha de handlers por frame, unwinding, valor de erro (§3.3),
   `max_steps` por frame, testes (mesma função, entre funções, `finally`
   em todos os caminhos, `exit` sem `finally`, não-capturado = erro).
5. `rhp-fmt`, diagnósticos, CLI (`test` com `throw` = falha).
6. Specs (`LANGUAGE_SPEC` §2/§7, `BYTECODE_SPEC` §2/§3/§5), exemplo
   `examples/http_erro_test.rhp`? Não — exemplo genérico `try_test.rhp`.
7. Gates + `changes/` + push.

## 9. Plano de testes

- Unidade por crate (lexer/parser/bytecode/VM/fmt/CLI).
- Matriz obrigatória: `throw`+captura local; propagação 2+ frames;
  `finally` no normal/catch/propagação/`return`/`break`; `throw` sem
  handler (mensagem idêntica à de hoje); `exit` pula `finally`; erro de
  builtin capturável (`json_parse` inválido); handler dentro de `for`.
- Integração fim a fim via `rhp run` (exit codes preservados).
- Regressão: suite atual intacta (nenhuma mudança de comportamento sem
  `try`).

## 10. Migração e rollout

Sem migração (aditivo). Rollout: merge via PR da Fase A após gates;
`finally`-sem-`catch` e `throw` documentados em `LANGUAGE_REFERENCE.md`
com exemplos executáveis.

## 11. Critérios de aceitação

- [ ] Todos os testes da §9 passam; suite existente sem regressão.
- [ ] `fmt`/`clippy`/`test` + `check-changes.sh` verdes.
- [ ] Specs atualizadas na mesma entrega; exemplo executável em CI.
- [ ] Custo do caminho feliz medido (benchmark) e desprezível.
- [ ] Esta RFC marcada `aceita` (aprovação do responsável) antes do merge
      da implementação.
