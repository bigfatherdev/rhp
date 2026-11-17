# [Fixed] — Panic com `max_stack_depth` + `return` (achado do fuzz, Etapa 3)

- ID: 2026-11-16-fix-fuzz-vm-panic
- Data: 2026-11-16
- Versão: unreleased
- Tipo: Fixed (correção) + Tooling (corpus de fuzz)
- Compatibilidade: Compatível (correção de panic para comportamento são;
  nenhum programa válido muda de resultado)

## Resumo

O novo fuzz de bytecode hostil encontrou panic real na VM
(`index out of bounds` em `check_stack_depth`): com `max_stack_depth`
configurado, qualquer `Return` de função entrava em panic, porque o
frame é removido e a checagem final usava o índice obsoleto. Sem
limites, o defeito ficava invisível (a checagem nem tocava no frame).
Corrigido com guarda mínima + regressão pinada. Junto: corpus de fuzz
determinístico std-only em CI (lexer, parser, VM/validador).

## Motivação

GOVERNANCE §9: a VM nunca pode entrar em panic com entrada não
confiável; §8.5 exige fuzzing do lexer/parser/leitor de bytecode.
O corpus anterior era finito e artesanal (21 + 27 entradas); o gap de
"fuzz contínuo" estava assumido. Este passo o estreita sem novas
dependências (sem cargo-fuzz/nightly, sem ADR).

## O que mudou

- Correção (`crates/rhp-vm/src/lib.rs`, `check_stack_depth`): tolera
  `idx` além de `frames.len()` (frame removido por `Return`); o topo
  corrente continua medido no passo seguinte (`run` recalcula o idx),
  então a fiscalização não enfraquece.
- Regressão: `respects_stack_depth_across_return` (teto 16 + chamada
  com `return`; antes: panic; depois: `1`).
- Fuzz lexer (`crates/rhp-lexer`, `fuzz_random_bytes_never_panic`):
  5000 entradas de 41 fragmentos (operadores, keywords, unicode,
  escapes, NUL); asserts: só Ok/Err, nunca panic.
- Fuzz parser (`crates/rhp-parser`, `fuzz_random_tokens_never_panic`):
  5000 sequências de 1–20 fragmentos (51 tokens); `lex` com `if let`
  (não usa o helper `expect`, que entraria em panic com erro léxico);
  asserts: nunca panic.
- Fuzz VM/validador (`crates/rhp-vm`, `fuzz_hostile_bytecode_never_panics`):
  8000 chunks aleatórios (pool fixo, operandos às vezes fora da faixa,
  saltos inválidos, `Halt` ausente em metade) + 600 mutações de 6
  programas válidos; tetos estritos (`steps` 500, pilha 16, coleção 32)
  contêm loops/saltos hostis. `run()` já valida antes de executar; o
  teste chama `validate` + `run` e descarta o resultado (só o panic
  reprova).
- Seeds fixas, LCG próprio (mesmo padrão dos property tests), sem deps.

## Detalhes técnicos

- Causa raiz: `step_inner` termina com `check_stack_depth(idx)`; `Return`
  faz `frames.pop()` e cai no trailing check com idx = len (obsoleto).
  Afetava qualquer `return` com `max_stack_depth = Some(_)` — inclusive
  via `VmLimits`, não só via fuzz.
- Não tok place em `Op::Return` de nível superior (já retorna
  "`return` fora de funcao") nem em `run_value` (só indexa após `Done`).
- `..Default::default()` removido onde redundante (clippy);
  seeds hex em grupos regulares (clippy).

## Compatibilidade e migração

Compatível; migração não necessária. Programas que antes entravam em
panic com teto configurado agora executam normalmente (ou recebem
erro de limite legítimo).

## Segurança e performance

Segurança: elimina classe de panic alcançável por bytecode hostil sob
teto configurado. Sem `unsafe`; sem `unwrap` novo. Performance:
mudança só em `#[cfg(test)]` + 1 guarda (`frames.get`) no caminho com
limite (sem limite, 1 branch como antes); sem medição de bench
(mudança fora do caminho quente padrão — sem limite, delta = 1
`Option::is_some` já existente).

## Testes e validação

- Reprodução antes da correção: `respects_stack_depth_across_return`
  e `fuzz_hostile_bytecode_never_panics` com panic em `lib.rs:735`.
- Depois: `cargo fmt --check` limpo,
  `cargo clippy --workspace --all-targets -- -D warnings` 0 warnings
  (3 lints encontrados e corrigidos no próprio diff),
  `cargo test --workspace`: **288 passed, 0 failed** (284 baseline +
  3 fuzz + 1 regressão).
- Tempo dos 3 fuzz juntos: < 0,5 s (não pesa o CI).

## Limitações e riscos conhecidos

- Corpus com seed fixa: mesmo espaço a cada run (não substitui fuzzer
  contínuo com cobertura; gap segue assumido, agora menor).
- `ROADMAP.md` não tocado aqui de propósito: a seção "Etapas 3–5"
  chega pelo PR #11; atualizá-la agora geraria conflito de merge —
  sincronizar após os merges (teto de heap #12, este, otimizações).
- Suite de conformidade (#11) não rodada contra este branch (mudança
  só adiciona testes; sem alteração de comportamento observável).

## Referências

- GOVERNANCE.md §3.1 (bug), §8.5, §9; `SECURITY.md` (sem panic);
- PR #11 (conformidade), PR #12 (heap) — branches irmãos, sem dependência.
