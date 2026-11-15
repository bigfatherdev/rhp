# [Added] — CP4: verificador de fluxo de pilha + limites estendidos

- ID: 2026-11-14-feat-verificador-pilha
- Data: 2026-11-14
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível para programas do compilador (todos passam);
  chunks artesanais invalidos falham mais cedo (antes: runtime; agora:
  validacao) — endurecimento, nao quebra de contrato

## Resumo

Analise estatica de altura de pilha no `validate` (underflow e juncoes
divergentes viram erro); `VmLimits` com `max_stack_depth` e
`max_collection_len`; runtime separado em `rhp-vm/src/builtins.rs`.

## Motivação

Missao Next Generation CP4 (auditoria NG-202 + extensao de NG-105-limites).

## O que mudou

- `rhp-bytecode`: `check_stack_flow` (worklist, alturas deterministas,
  terminais `Halt`/`Return`, merge com igualdade); efeito corrigido de
  `CallBuiltin` (-aridade+1, sem callee).
- `rhp-vm`: `VmLimits::{max_stack_depth, max_collection_len}`,
  `push_checked`/`check_collection`/`check_stack_depth`;
  `builtins.rs` separado (builtins + `http_get` + `MAX_HTTP_BODY`).
- Specs: `BYTECODE_SPEC.md` (§5 fluxo), `VM_SPEC.md` (limites/runtime).

## Arquivos ou áreas afetadas

- `crates/rhp-bytecode/src/lib.rs` (+3 testes), `crates/rhp-vm/{lib,builtins}.rs`
  (+3 testes), `ADRs/0008-verificador-pilha-runtime.md`, specs.

## Compatibilidade e migração

Todo programa gerado pelo compilador continua valido (suite prova).
Migração: não necessária. Chunks artesanais: mesma rejeicao, mais cedo.

## Segurança e performance

Validador iterativo (sem recursao nova); analise O(instrucoes).
`push_checked` adiciona 1 comparacao por empilhamento.

## Testes e validação

- Falha-antes: underflow e juncao divergente passavam no `validate`.
- Ajuste honesto: teste de runtime `Print` isolado agora falha na validacao
  (mensagem ainda menciona `pilha`); e um bug real encontrado no caminho
  (efeito de `CallBuiltin`).
- `cargo test --workspace`: 138 testes, 0 falhas.
- `cargo fmt --check` ok; `cargo clippy ... -D warnings` ok.

## Limitações e riscos conhecidos

Sem analise de tipos de operandos; sem teto de heap medido; pilha de
operandos do programa principal ilimitada por padrao (opt-in).

## Referências

- Auditoria: `reports/NEXT_GENERATION_AUDIT.md` (NG-202, NG-105).
- ADR: `ADRs/0008-verificador-pilha-runtime.md`.
