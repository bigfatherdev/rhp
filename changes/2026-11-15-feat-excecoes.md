# [Added] — Exceções: `try`/`catch`/`finally` e `throw` (RFC 0001)

- ID: 2026-11-15-feat-excecoes
- Data: 2026-11-15
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (aditivo; `try`/`catch`/`finally`/`throw`
  viram reservadas — zero usos como identificador no repo; programa sem
  `try` executa byte-identico)

## Resumo

`try B (catch V? C)? (finally F)?` + `throw EXPR`, com propagação entre
funções e `finally` em todos os caminhos (`return`/`break`/`continue`
que cruzam o `try` o executam; `exit()` não aciona, como no PHP). Erro
capturado vira valor (`{tipo: "runtime", mensagem: "..."}` p/ internos).

## Motivação

Fase A2 do plano de sistemas: sem tratamento local de erro, um request
inválido derrubaria o servidor (Fase C). Implementa a RFC 0001 aceita.

## O que mudou

- Lexer/parser: 4 keywords; `Stmt::Try { body, catch_var, catch_body:
  Option, finally_body: Option }` + `Stmt::Throw`; `try` sem ao menos um
  dos dois é erro de parse.
- Bytecode: ops `TryBegin/TryEnd/Throw/Rethrow`; `compile_try` (handler +
  `finally` duplicado por caminho, neutro na pilha); `break`/`continue`/
  `return` que cruzam `try` emitem `TryEnd` + copias (filtro por laco;
  guarda contra regressao infinita; handler marcado consumido no caminho).
- Validador: alvo de `TryBegin` entra com altura+1; `Throw`/`Rethrow`
  terminam o caminho; `TryBegin` no cheque de saltos.
- VM: pilha de handlers por frame; unwinding com truncamento de frames/
  escopos/pilha (cada frame conta em `max_steps`); `exit` nunca capturado;
  sem handler, erro original intacto (mensagem de interno preservada via
  mapa; `throw` solto = `excecao nao capturada: v`).
- `rhp-fmt`, diagnósticos (mapeamento existente), CLI (`throw` sem
  handler = falha; `exit` em `try` pula `finally`), gramática VSCode.
- Specs (`LANGUAGE_SPEC` §2/§7, `BYTECODE_SPEC` §2/§3/§5, `VM_SPEC` §3),
  exemplo `examples/try.rhp`, `docs/LANGUAGE_REFERENCE.md`.

## Detalhes técnicos

Custo do caminho feliz O(1) por `try` (um push/pop de handler). Cópias
de `finally` em escopo próprio (sem vazamento de `let`/`const`).
`diverges()` conservador para `try` (correto: iteração vazia cai).
`check_no_nested_imports` cobre `try`/`catch`/`finally`.

## Compatibilidade e migração

Aditivo. Migração: renomear eventual variável `try/catch/finally/throw`
(inexistente no repo). `switch`, tipos de erro hierárquicos e stack
traces seguem pendentes (fora do escopo da RFC).

## Segurança e performance

Sem `unsafe`, sem I/O novo, sem deps. Unwinding limitado por
`MAX_CALL_DEPTH` + `max_steps`. Benchmark dedicado adiado para carga
realista de handler (harness existe; sem otimização prematura).

## Testes e validação

- Falha-antes: `try` não parseava; ops desconhecidos na VM.
- Matriz da RFC §9 implementada em `rhp-vm` (mesma função, entre
  funções, `finally` nos 6 caminhos, `exit` sem `finally`, não-capturado
  intacto, erro de builtin capturável, escopos sem vazamento) + `rhp-fmt`
  + integração (`rhp run`, exit codes, exemplo `try.rhp`).
- `cargo test --workspace`: 181 passed, 0 failed (172 antes + 9 novos:
  parser 2, bytecode 1, VM 5, fmt 1, integração CLI 1; exemplo `try.rhp`
  coberto pelo teste de exemplos).
- `cargo fmt --check` ok;
  `cargo clippy --workspace --all-targets -- -D warnings` ok;
  `tools/check-changes.sh origin/main` ok.

## Limitações e riscos conhecidos

`finally` duplicado por caminho (código maior em `try`s com `finally`
grande; tradeoff documentado vs. sub-rotina). Sem `catch` condicional
por tipo (sem hierarquia). `return`/`throw` em `finally` substituem o
fluxo (documentado).

## Referências

RFC 0001 (`RFCs/0001-excecoes.md`, aceita). Sem ADR nova.
