# RFC 0012 — Builtins `rand`/`rand_range`/`fs_list`

- ID: RFC-0012
- Data: 2026-11-16
- Estado: aceita (programa RHP-100; implementada na Fase 4)
- Contexto: completar stdlib pratica (matematica/arquivos). Pequenos,
  sem arquitetura.

## 1. Problema

Sem aleatoriedade (jogos, amostragem, IDs temporarios) e sem listar
diretorios (`fs_*` cobre arquivos, nao pastas).

## 2. Objetivo e não-objetivos

Objetivo: `rand()` (`Int` em `[0, i64::MAX]`), `rand_range(min, max)`
(inclusive, `min <= max`), `fs_list(dir)` (nomes ordenados).
Nao-objetivos: cripto/rand seguro (vedado; documentado), `srand` (futuro),
recursao em `fs_list` (raso; futuro com flag), dotfiles filtrados (inclui
tudo que `read_dir` devolve).

## 3. Decisao de desenho

- `rand`/`rand_range` sao builtins separados (aridade fixa do RHP; o PHP
  dobra os dois em `rand()` — divergencia documentada).
- RNG: xorshift64* com estado `u64` por VM (semeado em `Vm::new` com
  relogio+pid; std-only). Sem vies relevante para uso comum (modulo
  simples; nao-cripto declarado).
- `rand_range`: `min`/`max` `Int`, `min > max` = erro; estouro evitado com
  conta em 128 bits.
- `fs_list(dir)`: `read_dir` + nomes (`String`) ordenados; vazio = `[]`;
  inexistente/nao-diretorio = erro. Sem jaula alem da vigente nos `fs_*`
  (relativo resolve no diretorio atual; documentado).
- Erros de tipo seguem o padrao (`exige ...`).

## 4. Alternativas consideradas

- Seed fixa (deterministico/testavel): rejeitada (surpreende em producao).
- Rejeitar `rand_range` e expor so `rand()`: rejeitado (PHP tem min/max;
  modulo pelo usuario e erro-prone com negativos).
- `srand()` agora: adiado (sem caso de uso fechado; anotado).

## 5. Compatibilidade

Aditivo (3 builtins novos). Nenhuma mudanca.

## 6. Seguranca

`rand` NAO e criptografico (documentado; sem `Hmac`/senha nesta leva).
`fs_list` nao segue links? `read_dir` lista o link em si (sem canonicalize
= sem TOCTOU novo alem do ja existente nos `fs_*`).

## 7. Performance

O(1) amortizado por chamada (`fs_list` = O(entradas)).

## 8. Plano de implementacao

1. Bytecode: variantes + nomes + aridades (0/2/1) + ids 53/54/55 + anti-deriva.
2. VM: `rng: u64` (seed em `new`; mantido no `reset`), parametro em
   `call_builtin`, 3 bracos, testes (faixa/tipo, limites, erros, dir
   temporario, ordenacao).
3. Specs (`LANGUAGE_SPEC` tabela, `BYTECODE_SPEC` ids, `PHP_DIFFERENCES`:
   split `rand`/`rand_range`, nao-cripto) + `changes`.

## 9. Plano de testes

- `rand()`: 100 chamadas em `[0, i64::MAX]`, todas `Int`.
- `rand_range(1, 6)`: 200 chamadas dentro; `min == max` devolve ele;
  `min > max` erro; nao-int erro.
- `fs_list`: dir com arquivos fora de ordem => ordenado; vazio => `[]`;
  inexistente => erro; arquivo (nao-dir) => erro.

## 10. Migracao e rollout

Nenhuma (aditivo).

## 11. Criterios de aceitacao

- [ ] Testes da §9 passam; suite sem regressao.
- [ ] `fmt`/`clippy`/`test` + `check-changes.sh` verdes.
- [ ] Specs + divergencias em `PHP_DIFFERENCES.md`.
- [ ] Esta RFC marcada `aceita` antes do merge.
