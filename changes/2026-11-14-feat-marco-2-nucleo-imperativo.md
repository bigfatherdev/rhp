# [Added] — Marco 2: núcleo imperativo do RHP

- ID: 2026-11-14-feat-marco-2-nucleo-imperativo
- Data: 2026-11-14
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (todo programa válido da Fase 1 continua válido
  com o mesmo significado; `hello.rhp` inalterado e reverificado)

## Resumo

O RHP passa a ter um núcleo imperativo: expressões (literais `int`/`float`/
`bool`/`null`, aritmética, comparação, unários), `let`/atribuição, blocos e
escopos, `if`/`else`, `while`, funções (`fn`, `return`, chamadas, recursão
limitada) e `echo` de qualquer valor, mais comentários de linha `//`.
Exemplo novo: `examples/fib.rhp` (`fib(10)` = `55`).

## Motivação

Cumprir o Marco 2 do `ROADMAP.md`, aprovado pelo responsável em 2026-11-14
para a Fase 2. Desenho registrado em `ADRs/0001-marco-2.md`.

## O que mudou

- `rhp-lexer`: novas keywords (`let if else while fn return true false null`),
  `Ident`, `IntLiteral`/`FloatLiteral`, 15 operadores/pontuações, comentários
  `//`; erros com linha:coluna (overflow de int, float inválido, etc.).
- `rhp-parser`: AST estendida (`Let Assign If While FnDef Return Block
  ExprStmt`; `Expr` com `Int Float Bool Null Var Unary Binary Call`);
  precedência `==/!=` < comparação < `+-` < `*/%` < unária.
- `rhp-bytecode`: `Value::Func`, `FuncDef`, pool `names`, tabela `funcs` e
  18 novos `Op` (`Add Sub Mul Div Mod Neg Not Eq Ne Lt Le Gt Ge JumpIfFalse
  Jump LoadName StoreName DeclareName EnterScope ExitScope DefineFunc Call
  Return Pop`); `compile()` com saltos remendados; `validate()` recursivo.
- `rhp-vm`: frames explícitos (`MAX_CALL_DEPTH = 512`), escopos locais +
  globais compartilhadas, aritmética checada, erros em português sem panics.
- `rhp-cli`: sem mudança de interface (pipeline genérico já cobria);
  4 novos testes de integração (expressões, controle, funções/fib, erro).
- Docs: `ADRs/0001-marco-2.md` (+`README`), `LANGUAGE_SPEC.md`,
  `BYTECODE_SPEC.md`, `VM_SPEC.md`, `ARCHITECTURE.md`, `ROADMAP.md` e
  `CHANGELOG.md` atualizados. `rhp-vm` ganhou dev-dependência interna
  `rhp-parser` (só testes, sem impacto em produção).

## Detalhes técnicos

Tipagem dinâmica com erro em runtime (sem truthiness implícita; `if`/`while`
e `!` exigem `Bool`); `Int`+`Float` promove; `+` concatena `Str`;
`%` só `Int`; `==` numérico entre `Int`/`Float`; funções comparam por
identidade. `let` no topo do principal declara global; `fn` idem.
Limitações conhecidas: sem closures sobre locais, sem chamada em cauda,
clonagem de chunk por chamada (correto, não otimizado).

## Compatibilidade e migração

Compatível. `hello.rhp` byte-idêntico na saída. Migração: não necessária.
Programas antes inválidos podem passar a valer (esperado em extensão).

## Segurança e performance

- Segurança: validação recursiva antes de executar; aritmética checada;
  limite de 512 frames contra estouro; sem `unsafe`; sem execução externa.
  Sem fuzz/auditoria (fora do escopo).
- Performance: não medida; sem alegações; clonagem por chamada documentada
  como não-meta.

## Testes e validação

Executados em 2026-11-14, Ubuntu 26.04, Rust stable (toolchain da Fase 1):

- `cargo fmt --check`: passou (`FMT_OK`).
- `cargo clippy --workspace --all-targets -- -D warnings`: passou (3
  correções no caminho: `is_some_and`, `while-let` com `allow` justificado,
  `mut` removido, campo morto removido, `&` em teste).
- `cargo test --workspace`: 66 testes, 0 falhas (lexer 12, parser 15,
  bytecode 12, VM 17, CLI 4 unit + 6 integração).
- Manual: `rhp run hello.rhp` → `Hello from RHP!\n` (byte-exato via `od -c`);
  `rhp run examples/fib.rhp` → `55`. Erros cobertos: overflow, divisão/resto
  por zero, tipos incompatíveis, var indefinida/redeclarada, condição
  não-bool, chamada de não-função, aridade, recursão 600 (> 512), `;`/bloco.

## Limitações e riscos conhecidos

Sem tipagem estática, módulos, stdlib, otimizações, fuzz, Windows ou CI de
`changes/` (planejados). Diretório segue sem repo Git (sem commit/push sem
autorização).

## Referências

- Aprovação: responsável pelo projeto em 2026-11-14 (resposta a "como devo
  continuar" → "Aprovo Fase 2 / Marco 2").
- ADR: `ADRs/0001-marco-2.md`. Fase 1: `changes/2026-11-14-feat-fase-1-fatia-vertical.md`.
