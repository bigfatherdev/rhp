# [Fixed] — Escopo de `while`/`for` nao vaza mais em `break`/`continue`

- ID: 2026-11-15-fix-laco-escopo-break
- Data: 2026-11-15
- Versão: unreleased
- Tipo: Fixed
- Compatibilidade: Compatível (corrige para o comportamento especificado;
  programa que dependia do vazamento — `let` do corpo visivel apos o laco
  — passa a receber "variavel nao definida")

## Resumo

`break`/`continue` agora fecham tambem o escopo do corpo do laco.
Antes, `while true { let vazou = 1; break; } echo vazou;` imprimia `1`;
agora e erro "variavel nao definida", como manda a especificacao
(blocos/escopos nao vazam).

## Motivação

Bug encontrado ao implementar o `for` (mesmo maquinario `LoopCtx`):
reproduzido antes da correcao com
`while true { let vazou = 1; break; } echo vazou;` => imprimiu `1`.
O lote 2 afirmava "desenrolamento correto", mas so o fluxo estava certo;
o escopo vazava (inclusive com `continue`, que acumulava um escopo por
iteracao).

## O que mudou

- `crates/rhp-bytecode/src/lib.rs`: `LoopCtx` agora carrega
  `break_depth`/`continue_depth` (`while`: ambos = base do laco, i.e.
  fecha o escopo do corpo; `for`: `break` = escopo do laco,
  `continue` = escopo do corpo) e `start: Option<u32>` (`while` salta
  direto; `for` remenda para o incremento).
- Rastreamento estatico de escopos com snapshot/restore ao redor do
  corpo: desvios no corpo movem o rastreador, que e restaurado para o
  caminho normal (pools de nomes/constantes e saltos mantidos).
- `Stmt::For` incluído no `check_no_nested_imports` da CLI (`import`
  dentro de `for` segue proibido, como em `while`).

## Detalhes técnicos

Caminhos de runtime do `while` agora concordam todos na base do laco
(condicao-falsa pula o `EnterScope`; saida normal fecha; `break` fecha
tudo e salta a saida; `continue` fecha tudo e volta ao rotulo). No `for`,
condicao e saidas vivem dentro do escopo do laco (lista/indice ocultos),
fechado na saida por ambos os caminhos. Validador de fluxo inalterado.

## Compatibilidade e migração

Programas que liam variaveis declaradas no corpo do laco apos o laco
(comportamento incorreto) passam a falhar com "variavel nao definida".
Todo o resto e identico (suite de regressao intacta + 5 testes novos).

## Segurança e performance

Mesmo numero de instrucoes no caminho quente do `while` (só alvos de
desenrolamento mudaram). `continue` em laco infinito nao acumula mais
escopos (antes, crescimento sem teto por iteracao).

## Testes e validação

- Falha-antes: probe `while true { let vazou = 1; break; } echo vazou;`
  imprimiu `1` no binario anterior; teste de regressao novo falha sem a
  correcao e passa com ela.
- `cargo test --workspace`: 168 passed, 0 failed.
- `cargo fmt --check` ok;
  `cargo clippy --workspace --all-targets -- -D warnings` ok.

## Limitações e riscos conhecidos

Nenhum novo: `return` dentro de laco segue desenrolando o frame inteiro
(sem mudanca).

## Referências

`LANGUAGE_SPEC.md` §2 (escopo de lacos). Sem ADR (correcao dentro do
desenho vigente).
