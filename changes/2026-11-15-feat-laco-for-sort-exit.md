# [Added] — Laco `for-in`, `sort()` e `exit()`

- ID: 2026-11-15-feat-laco-for-sort-exit
- Data: 2026-11-15
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (sintaxe e builtins novos; `for` vira palavra
  reservada — unico programa existente afetado seria um que usasse `for`
  como identificador, inexistente no repo e improvavel; `in` segue
  identificador comum)

## Resumo

`for x in iteravel { ... }` (array = elementos em ordem; mapa = chaves
ordenadas), builtin `sort(array)` (copia ordenada, estavel, homogenea) e
builtin `exit(codigo)` (0-255; vira exit code do processo no `rhp run`).

## Motivação

Missão/auditoria de paridade PHP (§18, lote imediato): `for-in` (acucar
sobre indice/`len`, sem runtime novo alem de um op de normalizacao),
`sort` sem callback (callbacks exigem funcoes de primeira classe,
planejado) e `exit` (limite documentado em
`changes/2026-11-15-feat-io-env.md`).

## O que mudou

- Lexer: keyword `for` (`in` e contextual: `Ident("in")` posicional).
- Parser: `Stmt::For { var, iter, body }`; `for x in e { }`; `in` segue
  valido como variavel (`let in = 1;` passa).
- Bytecode: op `IterList` (array => copia; mapa => chaves ordenadas;
  resto = erro "`for` exige array ou mapa"); `for` compilado como
  escopo+laco por indice com nomes ocultos contendo NUL (nunca colidem);
  `break` fecha corpo+laco, `continue` salta ao incremento.
- Builtins 36 `sort` e 37 `exit` (aridades 1; anti-deriva estendido).
- VM: `IterList`, `sort` (numeros mistos exatos quando possivel, `Str`
  por bytes, `Bool`; `NaN` por ultimo; erro em mistura), `exit`
  (`VmError::exit_code()`, desenrola frames).
- CLI: `rhp run` traduz `exit(N)` em exit code N sem mensagem;
  `rhp test` conta `exit` como falha do arquivo (nao mata o corredor).
- `rhp-fmt`: imprime `for x in e { ... }` (idempotente).
- Gramática VSCode: `for` + `const`/`break`/`continue` + builtins ate
  `exit` (antes parava no lote da Fase 6); teste de consistencia estendido
  a todos os keywords/builtins (`in` de proposito fora: e identificador).

## Detalhes técnicos

Iteravel avaliado uma vez (snapshot por valor; mutar a origem no corpo nao
afeta a iteracao). Variavel do laco declarada por iteracao no escopo do
corpo (invisivel fora). `sort` devolve copia (`a = sort(a);`). Iteracao de
string: `for c in split(s, "")` (sem builtin novo). `exit` fora de 0-255
ou nao-`Int` = erro comum.

## Compatibilidade e migração

Compativel. Migracao: nao necessaria. Excecao minima: `for` reservada
(ver Resumo); `in` nao reservada justamente para nao quebrar nada.

## Segurança e performance

Sem `unsafe`; sem I/O novo. `IterList`/`sort` passam por `push_checked`
(teto `max_collection_len` respeitado). `for` sobre array vazio nao aloca
alem da lista vazia. `exit` nao imprime nada; stdout anterior preservado.

## Testes e validação

- Falha-antes: testes novos de parser/bytecode/VM falharam antes da
  implementacao (`for` nao parseava; `IterList`/`sort`/`exit`
  desconhecidos).
- `cargo test --workspace`: 168 passed, 0 failed (154 antes + 14 novos).
- `cargo fmt --check` ok;
  `cargo clippy --workspace --all-targets -- -D warnings` ok.

## Limitações e riscos conhecidos

Sem `sort` com callback (exige primeira-classe, planejado). Sem iteracao
direta de string (use `split(s, "")`). `filter`/`map` funcionais,
`switch`/`for` classico C, JSON, datas seguem planejados (auditoria §21).
Doc `changes/2026-11-15-feat-stdlib-lote2.md` diz "19 builtins, ids
10-28"; o correto e 20 builtins, ids 10-29 (este registro nao reescreve o
anterior, so anota a divergencia).

## Referências

Auditoria `reports/PHP_PARITY_AUDIT.md` (§18 itens 4/6 parcial, §21 passo
1). Sem ADR (extensao dentro da arquitetura vigente).
