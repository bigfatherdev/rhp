# RFC 0003 — Closures e funcoes de primeira classe (captura por valor)

- ID: RFC-0003
- Data: 2026-11-15
- Estado: aceita (autorizacao do programa RHP-100; implementada em P1)
- Contexto: programa "RHP 100% PHP" (fatia funcional, nao clonagem);
  closures sao pre-requisito de callbacks, `map`/`filter` com funcao,
  middleware componivel e metodos futuros.

## 1. Problema

Funcoes RHP hoje so existem por nome global (`fn f`) e so sao chamaveis
pelo nome (`Call` por `LoadName`). Nao ha como: passar funcao como
argumento, retornar funcao, guardar funcao em variavel/array/mapa, criar
funcao anonima, nem capturar ambiente lexico (base de `map`/`filter` com
callback, middleware, handlers como valor, `usort`-futuro).

## 2. Objetivo e não-objetivos

Objetivo: `fn (params) { corpo }` como expressao; chamada de qualquer
expressao (`f(x)`, `f()(1)`, `(fn(x){x})(2)`); captura lexica **por valor**
(snapshot na criacao, coerente com a semantica RHP por copia e com o
`use` padrao do PHP); `return` sai da ativacao; custo documentado.

Nao-objetivos: captura por referencia (`use (&$x)` do PHP — conflita
com copia; rejeitado com motivo), funcoes nomeadas como expressao,
metodos/`$this` (fase OO), `is_callable`/reflection de funcoes,
tail-call, `static` em funcao.

## 3. Decisao de desenho (resumo implementavel)

- Sintaxe (sem keyword nova): `Primary` continua; posfixo ganha chamada
  geral: `Postfix := Primary (("[" Expr "]") | ("(" Args ")"))*`.
  `f(x)` com `f` = Ident preserva o caminho atual (`Call` por nome,
  incluindo builtins); demais callees viram `CallValue`.
- AST: `Expr::Closure { params, body }` + `Expr::CallValue { callee, args }`.
- Captura: **cadeia de escopos da ativacao definidora, clonada na criacao**
  (zero analise estatica; shadowing e aninhamento corretos por
  construcao; cada chamada clona o env de novo — sem estado
  compartilhado entre chamadas). Globais NAO sao capturados (lidos ao
  vivo na chamada — coerente: globais sao estado compartilhado por
  desenho; locais sao valores).
- Consequencia documentada: contador via captura NAO acumula
  (`c = c + 1` sempre parte do snapshot); contadores usam `global`
  explicito, parametros ou banco. Sem ciclos possiveis (valores sao
  arvores), sem GC novo.
- Atribuicao a capturada dentro do corpo altera so a copia da ativacao.
- `throw`/`return`/`break` dentro de closure: `return` sai da ativacao;
  `break`/`continue` seguem vinculados ao laco textual (erro fora dele).
- `Call(arity)` existente ja e generico no callee: nenhum op novo de
  chamada. Novo op so para criacao: `MakeFunc(f)` (empilha `Func`
  com env; `DefineFunc` passa a fazer o mesmo + declarar).
- `Func` vira `FuncValue { def, env }`; `Display`/`type()` inalterados
  (`<fn nome>`, anonima = `<fn anonima>`).
- `Vm::run_value` ja existe (C2) e e reutilizado nos testes.

## 4. Alternativas consideradas

- Captura por referencia (`Rc<RefCell>` + cells): aliasing, ciclos e
  pressao no coletor; contradiz copia-por-valor; rejeitada.
- Analise estatica de variaveis livres + lista de captura: otimizacao
  valida futura (equivalencia exige prova com shadowing); rejeitada para
  o v0 em favor da correcao simples (cadeia inteira).
- `use (...)` explicito estilo PHP: preciso mas verboso; rejeitado no
  v0 (automatico e suficiente com snapshot; `use` pode voltar como
  restricao opt-in futura — anotado).
- `arrow fn => expr`: acucar; adiado (nao e arquitetura).

## 5. Compatibilidade

Aditiva. Nenhum programa valido muda de comportamento:
`f(x)` por `Ident` compila exatamente como antes (incluindo precedencia
de builtins sobre `fn` homonima — mantida). `fn` vira reservada em
posicao de expressao? Nao: `fn nome` continua statement; `fn (` vira
expressao pelo lookahead — zero quebra lexica nova (a keyword `fn` ja
era reservada).

## 6. Segurança

Sem `unsafe`, sem Send manual (env clonado dentro da thread; `Rc`
nunca cruza threads — closures nao sao `Send`, como `Func` nunca foi).
Custo O(escopo) por criacao/chamada (declarado; sem enumeracao surpresa:
pilha de operandos e `max_collection_len` continuam valendo).

## 7. Performance

Baseline nos testes (criacao/chamada em laco) + nota de custo na spec.
Sem otimizacao (lista de captura futura exige prova de equivalencia).

## 8. Plano de implementacao (P1)

1. Lexer: nada (sem token novo).
2. Parser: `Expr::Closure` + `Expr::CallValue`, posfixo geral,
   `fn (` => anonima em expressao; testes (falha-antes).
3. Bytecode: `MakeFunc`, `FuncValue{def,env}`, `Call` generico ja serve;
   `DefineFunc` captura igual; validador (MakeFunc sem efeito de pilha).
4. VM: env clonado na chamada (params sombreiam); testes (captura,
   snapshot, aninhadas, loop-var por iteracao, retorno/first-class,
   erro de callee).
5. `rhp-fmt` (anonima + chamada geral, idempotente), CLI (nada),
   gramatica (nada: sem token novo).
6. Specs (`LANGUAGE_SPEC` §3/§4, `BYTECODE_SPEC` novo op,
   `PHP_DIFFERENCES.md` com divergencias assumidas), exemplo
   `examples/closures.rhp` (roda em CI), `changes/` + push.

## 9. Plano de testes

- Unidade por crate; matriz: criacao/primeira-classe (var/array/mapa/
  retorno/argumento); snapshot (mutacao posterior nao afeta); aninhadas;
  loop-var por iteracao; `return` interno; nao-funcao como callee;
  aridade; IIFE; `use` implicito de global (ao vivo) vs local (snapshot).
- Integracao: `map`-manual via callback (prova do destravamento).
- Regressao: suite intacta (Call por nome identico).

## 10. Migracao e rollout

Nenhuma (aditivo). Rollout via PR da fase P1.

## 11. Criterios de aceitacao

- [ ] Testes da §9 passam; suite sem regressao.
- [ ] `fmt`/`clippy`/`test` + `check-changes.sh` verdes.
- [ ] Specs + exemplo executavel em CI; divergencias em
      `PHP_DIFFERENCES.md`.
- [ ] Esta RFC marcada `aceita` antes do merge.
