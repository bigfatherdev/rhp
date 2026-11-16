# [Added] — Builtins `rand`/`rand_range`/`fs_list` (RFC 0012, Fase 4)

- ID: 2026-11-16-feat-rand-fslist
- Data: 2026-11-16
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (3 builtins novos, ids 53–55)

## Resumo
`rand()` (`Int` em `[0, i64::MAX]`), `rand_range(min, max)` (inclusive) e
`fs_list(dir)` (nomes ordenados). xorshift64* std-only, NAO criptografico.

## Motivação
Stdlib pratica (matematica/arquivos). Branch: `feat/php-100-fase4-chamadas`.

## O que mudou
- Bytecode: `Builtin::{Rand, RandRange, FsList}` (ids 53/54/55), nomes,
  aridades (0/2/1), anti-deriva.
- VM: `rng: u64` (seed relogio+pid em `new`; mantido no `reset`),
  parametro em `call_builtin`, `next_rand` (xorshift64*), 3 bracos
  (modulo simples; conta em 128 bits contra estouro; `read_dir` + sort),
  testes (100x faixa/tipo, limites, erros, dir temporario).
- Specs: tabelas + `PHP_DIFFERENCES.md` (split `rand`/`rand_range`, nao-cripto,
  raso/ordenado). CLI: teste `fs_list` (ordenado, vazio, inexistente).

## Detalhes técnicos
- Alternativas na RFC 0012: seed fixa (surpreende em producao), so `rand()`
  (modulo pelo usuario e erro-prone), `srand` (adiado).
- `>> 1` limita a `[0, i64::MAX]` (`as i64` puro negativaria metade).
- Arquivos: bytecode/vm + `RFCs/0012-builtins-rand-fslist.md`.

## Compatibilidade e migração
Aditivo. Nenhuma.

## Segurança e performance
- xorshift NAO criptografico (documentado em spec + ajuda? — sem ajuda
  dedicada; mensagem de erro nao vaza nada).
- `fs_list`: sem canonicalize novo alem do vigente; O(entradas).

## Testes e validação
- Ver consolidado da Fase 4 (suite final verde).

## Limitações e riscos conhecidos
- Sem `random_bytes`/cripto (vedado) e sem `srand` (futuro).
- `fs_list` raso, inclui tudo que `read_dir` devolve.

## Referências
- `RFCs/0012-builtins-rand-fslist.md` (aceita)
- Branch `feat/php-100-fase4-chamadas`
