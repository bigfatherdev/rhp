# LANGUAGE_SPEC.md — Especificacao da linguagem (Fases 3–6 implementadas)

> Regra: descreve apenas o que foi implementado e validado. O restante esta
> em "Nao especificado ainda".

## 1. Programa, comentarios e modulos

```text
Program := Stmt*
Block   := "{" Stmt* "}"
```

- Arquivo fonte: texto UTF-8. Comentarios de linha `//` ate o fim da linha.
- Programa vazio e valido (sem saida).
- **Modulos (Fases 4 e A3):** `import "caminho.rhp";` (caminho relativo
  ao arquivo que importa). Resolucao em profundidade: cada modulo executa
  uma vez, antes do importador; mesmo arquivo importado N vezes entra uma
  vez; ciclo (`a -> b -> a`) e arquivo ausente sao erros.
  - `export fn f` / `export let x` / `export const N` (opt-in, nivel
    superior): o nome e publico; sem `export` = privado ao modulo.
  - `import { a, b as c } from "m.rhp";` importa SO nomes exportados
    (outros = erro "`x` nao e exportado"); `b as c` vincula com alias.
    `from`/`as` sao contextuais (identificadores comuns fora do `import`).
    `export { a, b as c } from "m.rhp";` re-exporta (RFC 0011, so nivel
    superior): nomes passam aos exports deste modulo (transitivo, inclusive
    em cadeias; ciclo = "importacao circular").
  - `import "m.rhp"` (bare) mantem a semantica legada: vincula todos os
    nomes de topo (publicos e privados).
  - Colisao: so entre nomes visiveis vinculados (erro alto, como antes).
    Modulos distintos nao colidem entre si (isolamento por renomeacao
    interna; programas legados sem colisao comportam-se igual).

```rhp
import "mat.rhp";
echo dobro(21); // 42
import { dobro as d } from "mat.rhp";
echo d(1); // 2 (privados de mat.rhp seguem invisiveis)
```

Regras de modulos: caminhos relativos ao arquivo que importa, **confinados
a arvore do programa** (fora dela = erro); cada modulo executa uma vez;
ciclos e arquivos ausentes/grandes demais (> 8 MiB por arquivo) sao erros;
`import` vale **apenas no nivel superior** do arquivo (dentro de `fn`/bloco
= erro).

- **Limite de profundidade sintatica: 128 niveis** (expressoes e blocos
  aninhados; cadeias planas como `1+1+...` ou instrucoes em sequencia nao
  contam). Acima do limite, erro `profundidade maxima excedida`
  (protecao contra estouro de pilha — ver ADR 0005). Constantes de bytecode
  artesanais alem de 128 niveis sao rejeitadas na validacao.

## 2. Instrucoes

```text
Stmt       := LetStmt | ConstStmt | DestructureStmt | AssignStmt | EchoStmt
           | IfStmt | WhileStmt | ForStmt | ForCStmt | SwitchStmt | TryStmt
           | ThrowStmt | BreakStmt | ContinueStmt | FnDef | StaticStmt | ReturnStmt
           | Block | ImportStmt | ImportNames | Export | ReExport | ExprStmt
LetStmt    := "let" Ident (":" Tipo)? ("=" Expr)? ";"
ConstStmt  := "const" Ident (":" Tipo)? "=" Expr ";"
Tipo       := "int" | "float" | "string" | "bool" | "array" | "mapa" | "null" | "funcao"
DestructureStmt := ("let" | "const") ("[" Ident ("," Ident)* "]"
           | "{" (Ident | Ident ":" Ident) ("," (Ident | Ident ":" Ident))* "}") "=" Expr ";"
AssignStmt := Target "=" Expr ";"
Target     := Ident | Ident "[" Expr "]"
EchoStmt   := "echo" Expr ";"
IfStmt     := "if" Expr Block ("else" Block)?
WhileStmt  := "while" Expr Block
ForStmt    := "for" Ident "in" Expr Block
ForCStmt   := "for" Stmt? ";" Expr? ";" (Assign | Expr)? Block
           // `init` = `let`/`const`/atribuicao/expressao; `passo` sem `let`;
           // secoes vazias ok (`for (;;)` = infinito)
SwitchStmt := "switch" Expr "{" (CaseClause | DefaultClause)* "}"
CaseClause := "case" Expr Block   // `default` unico, posicao livre
DefaultClause := "default" Block
TryStmt    := "try" Block ("catch" Ident? Block)? ("finally" Block)?
           // ao menos um de `catch`/`finally` e obrigatorio
ThrowStmt  := "throw" Expr ";"
BreakStmt  := "break" Int? ";"     // N >= 1, padrao 1 (sai de N breakables)
ContinueStmt := "continue" Int? ";" // N >= 1, padrao 1 (conta laco e `switch`)
FnDef      := "fn" Ident "(" (Param ("," Param)*)? ("," "..." Ident)? ")" (":" Tipo)? Block
ClassDef   := "class" Ident "{" Metodo* "}"
Metodo     := "fn" Ident "(" (Param ("," Param)*)? ("," "..." Ident)? ")" (":" Tipo)? Block
Param      := Ident (":" Tipo)? ("=" Expr)?   // padrao: so expressao constante (dobra em compilacao)
ReturnStmt := "return" Expr? ";"
ImportStmt := "import" String ";"
StaticStmt := "static" Ident "=" Expr ";"   // so dentro de funcao
ReExportStmt := "export" "{" (Ident ("as" Ident)?) ("," ...)* "}" "from" String ";"
ExprStmt   := Expr ";"
```

- `let x = e;` declara no escopo atual (`let x;` inicializa com `null`).
  Redeclarar no mesmo escopo e erro em runtime.
- Anotacoes de tipo (RFC 0016, Etapa 5): `let x: int = 1;`,
  `const N: string = "a";`, `fn f(a: int, b = 1): int { }` (e closures).
  Tipos = `int | float | string | bool | array | mapa | null | funcao`
  (nomes de `type()`). Checagem em runtime no vinculo (declaracao,
  argumento apos defaults, retorno): mismatch = erro `tipos
  incompatíveis: esperava T, encontrou V` (capturavel por `try`).
  `let x: int;` falha (nulo implicito nao satisfaz `int`). Sem anotacao,
  zero checagem (nenhuma instrucao nova). `static`/destructuring sem
  anotacao (nao-objetivo).
- Classes (RFC 0017, Etapa 5): `class Nome { fn m(params) { } ... }`,
  `new Nome(args)`, `obj.metodo(args)`, `obj.campo`, `obj.campo = v`,
  `this` (vinculado no metodo). Construtor = metodo `novo` (obrigatorio;
  retorno ignorado). Instancias sao POR VALOR (copia, como mapas/arrays;
  sem aliasing): mutacoes via `obj.metodo()` sobre variavel nomeada
  persistem (write-back do `this` final; coerente com `a[i] = v`, que
  tambem exige variavel); metodo destacado (`let f = obj.m; f()`) nao
  persiste; `type()` = `"classe"`/`"instancia"`; `echo` mostra campos.
  Sem heranca, sem `private`, sem estaticos de classe (nao-objetivos).
- `static n = e;` dentro de funcao (RFC 0010): estado por definicao,
  avaliado a cada chamada mas atribuido so na primeira; visivel no corpo
  inteiro (lookup: escopos, statics, globais); fora de funcao = erro.
- `export { a, b as c } from "mod.rhp";` re-exporta (RFC 0011): nomes passam
  aos exports deste modulo (transitivo); `export * from` nao existe.
- `let [a, b] = e;` / `let {x} = e;` / `let {k: v} = e;` (e formas `const`,
  que congelam todos; RFC 0007): avalia `e` uma vez e desempacota por
  posicao (array) ou chave (mapa; `{x}` = chave `"x"`). Indice fora dos
  limites, chave ausente e duplicata no padrao = erro (regras vigentes de
  `Index`/`DeclareName`; sem `null` silencioso). Padroes vazios e aninhados
  = erro de parse.
- `x = e;` exige variavel visivel (escopos internos -> externos -> globais).
- `echo e;` escreve `Display` do valor + `\n` (qualquer tipo).
- `if`/`while` exigem condicao `Bool` (sem truthiness implicita). `while`
  reavalia a condicao a cada iteracao. `for x in e { ... }` avalia `e` uma
  vez: array itera os elementos em ordem; mapa itera as chaves `Str` em
  ordem (mesma ordem de `keys`); gerador consome sob demanda (forma lazy,
  RFC 0018); outro tipo = erro de runtime ("`for` exige array, mapa ou
  gerador"). `x` e declarada a cada iteracao no escopo do corpo (atribuir
  a `x` nao afeta a iteracao; para strings, use
  `for c in split(s, "")`). `break` (dentro de `while`, `for` ou `switch`,
  mesmo aninhados em `if`/blocos) sai do laco/`switch` mais interno e fecha
  tambem o escopo do corpo — `let`s internos nao vazam para fora; fora de
  laco/`switch` e erro de compilacao. `continue`/`continue 1` em `switch`
  equivale a `break` dele (gotcha do PHP); `continue N` conta N breakables
  (laco ou `switch`) para fora. `continue` no `for` pula ao proximo
  elemento. `in` e contextual (identificador comum fora do `for`), mas
  `for` e palavra reservada. Blocos `{}` criam escopo; shadowing
  com `let` interno e permitido.
- Geradores (RFC 0018, Etapa 5): `fn g() { yield 1; yield 2; }` — chamar
  NAO executa (devolve gerador lazy); `next(g)` retoma ate o proximo
  `yield` (valor) ou fim (`Null`, marca esgotado); `exhausted(g)` testa o
  fim sem executar. `next` apos o fim = erro; `yield` fora de funcao =
  erro de compilacao; `return` dentro de gerador descarta o valor.
  Padrao manual: `while !exhausted(g) { let v = next(g); if exhausted(g)
  { break; } ... }` (o `next` final entrega `Null` descartavel).
  `for x in gen` consome (com `break`/`continue`); gerador esgotado em
  novo `for` = zero iteracoes. `this` em metodos-geradores e vinculado
  na criacao (sem write-back). `type()` = `"gerador"`; `==` por
  identidade.
- `switch e { case c { ... } default { ... } }` (RFC 0004): avalia `e` uma
  vez; cada `case` compara com `==` do RHP em ordem ate o primeiro match
  (preguicoso); sem `break`, o controle cai no proximo ramo (fallthrough);
  `default` e unico, em qualquer posicao, e casa quando alcancado sem match
  anterior. `break` sai do `switch` (mais interno); `continue` em `switch`
  equivale a `break` dele (gotcha do PHP; `continue N` conta niveis
  incluindo `switch`, ver RFC 0008).
  Cada ramo tem escopo proprio (`let` repetido em ramos distintos nao
  colide). `case`/`default` fora de `switch` e erro de parse; `switch`
  vazio e permitido. `switch(true)` com cases int nao casa (`==` nao promove
  `Bool`; ver `PHP_DIFFERENCES.md`).
- `match e { c1 => v1, default => v2 }` (RFC 0008, expressao): avalia `e`
  uma vez; bracos em ordem com igualdade estrita, primeiro match vence (sem
  fallthrough); `default` unico, em qualquer posicao, casa quando alcancado;
  sem match e sem `default` = erro de runtime (capturavel por `try`).
- `fn (p) => expr` e acucar para closure com corpo `[return expr]`
  (fmt normaliza para bloco).
- `for init; cond; passo { corpo }` (RFC 0008): `init`/`passo` executados
  como statements (`let`/`const` so no `init`); `cond` ausente = `true`;
  `continue` salta ao passo; `init` com `let` nao vaza (escopo proprio).
  `break N;`/`continue N;` (`N` literal >= 1, padrao 1): contam N breakables
  (laco ou `switch`) para fora; em laco continuam-no, em `switch` equivalem
  a `break` dele; alem do aninhamento = erro de compilacao.
- `try B (catch V? C)? (finally F)?` (ao menos um dos dois): erro em `B`
  (incluindo `throw`, `assert` falho, builtin com erro) procura o `catch`
  mais interno na mesma funcao; sem handler, desenrola chamadas (rodando
  `finally`s) e falha como hoje. `V` recebe o erro: mapa
  `{tipo: "runtime", mensagem: "..."}` para erros internos, ou o valor de
  `throw`. `F` roda sempre (normal, `catch`, erro propagado, `return`/
  `break`/`continue` que cruzem o `try`); `return`/`throw` em `F`
  substituem o fluxo. `exit()` nao aciona `finally`. `try` sem `catch`
  nem `finally` e erro de parse.
- `const NOME = expr;` declara como `let`, mas imutavel: atribuir depois
  (inclusive `NOME[i] = v`) e erro de compilacao.
- `fn nome(params) { ... }` declara funcao no escopo atual. Chamada
  `nome(args)` avalia callee e argumentos da esquerda para a direita;
  aridade: sem defaults/`...`, exata; com defaults, ao menos os
  obrigatorios (iniciais; obrigatorio apos opcional e erro de parse);
  `...rest` (no maximo um, por ultimo) coleta excedentes em `Array`
  (`[]` se nenhum). Ausentes usam o padrao compilado (`null` explicito
  e valor, nao aciona padrao). Nomeados (`f(x: 1)`, qualquer ordem) e
  spread (`f(...arr)`, splice em ordem; RFC 0009) ligam em runtime:
  desconhecido, conflito posicional+nomeado e spread nao-`Array` = erro.
  `return e;` sai da funcao com o valor (`return;``
  retorna `null`); fim do corpo sem `return` tambem retorna `null`.
  `return` fora de funcao e erro de compilacao. Recursao permitida ate
  `MAX_CALL_DEPTH = 512` (erro apos o limite). Funcoes enxergam parametros,
  locais proprios, ambiente capturado (closures) e globais.
- `fn (params) { ... }` (sem nome) e expressao: cria funcao de primeira
  classe capturando o ambiente atual por valor (snapshot dos escopos
  internos para os externos; parametros sombreiam capturadas). Atribuicao
  posterior ao mesmo nome nao afeta a closure ja criada dentro de funcoes;
  no nivel superior, `let` vive em `globals` e segue dinamico (paridade com
  `fn` nomeada). Chamada de callee arbitrario: `(fn (x) { ... })(41)`,
  `f(x)(y)`; `f(x)` direto preserva o caminho classico (precedencia de
  builtins). `import`/`export` dentro de closure e erro do loader
  (so nivel superior). Display: `<fn {closure}>` para anonimas.
- `ExprStmt` (ex.: `f();`) avalia e descarta o valor.

## 3. Expressoes e precedencia (da menor para a maior)

Ternario (`a ? b : c`, associativo a direita) < coalescencia nula
(`a ?? b`, avalia `a` uma vez; `null` usa `b`) < igualdade (`==`, `!=`) <
OU-bit (`|`) < XOR-bit (`^`) < E-bit (`&`) < comparacao (`<`, `<=`, `>`,
`>=`) < deslocamento (`<<`, `>>`) < aditiva (`+`, `-`) < multiplicativa
(`*`, `/`, `%`) < unaria (`!`, `-`, `~` e `+x` como identidade) < primaria.

```text
Primary   := Int | Float | String | "true" | "false" | "null"
           | Array | Map | Ident | Closure | Match | "(" Expr ")"
Closure   := "fn" "(" (Ident ("," Ident)*)? ")" Block
           | "fn" "(" (Ident ("," Ident)*)? ")" "=>" Expr  // arrow: corpo `[return expr]`
Match     := "match" Expr "{" (MatchArm ",")* (MatchArm ","?)? "}"
MatchArm  := (Expr | "default") "=>" Expr
Array     := "[" (Expr ("," Expr)*)? "]"
Map       := "{" ((String | Ident) ":" Expr ("," ...)*)? "}"
Call      := Ident "(" (Expr ("," Expr)*)? ")"
CallValue := Postfix "(" (Expr ("," Expr)*)? ")" // callee arbitrario
CallArg   := Expr | Ident ":" Expr | "..." Expr // posicional, nomeado, spread
// Nomeado/spread compilam para `CallMixed` (descritor); so-posicional usa o
// `Call` classico. Posicional apos nomeado, duplicata e builtin com
// nomeado/spread = erro de compilacao.
// Posfixo: Primary (("[" Expr "]") | ("(" Args ")"))* (indexacao e
// chamada encadeadas; callee `Ident` simples usa `Call` classico)
```

Literais: inteiros `i64` (`42`), floats (`1.5`, ao menos um digito apos o
ponto), strings `"..."` com escapes `\" \\ \n \r \t \u{HEX}` (`\u` exige
chaves com 1–6 digitos hexadecimais formando ponto de codigo Unicode valido,
ex.: `\u{41}` = `A`, `\u{1F600}` = 😀), `true`/`false`/`null`.
Identificadores: `[a-zA-Z_][a-zA-Z0-9_]*` (palavras-chave reservadas).

## 4. Semantica de operadores

- Aritmetica com checagem: estouro de `Int`, divisao e resto por zero
  (int e float) sao erros. `Int`+`Float` promove para `Float`.
  `Str + Str` concatena; demais combinacoes mistas sao erro.
  `%` exige `Int` e `Int`. `-x` exige numero; `!b` exige `Bool`.
- Comparacao: `< <= > >=` aceitam numero-numero (com promocao) e
  `Str`-`Str` (lexicografica); resto e erro. `==`/`!=` aceitam quaisquer
  valores: `Int`/`Float` comparam numericamente, funcoes por identidade de
  definicao + igualdade do ambiente capturado, tipos diferentes sao diferentes.
  `match` usa igualdade **estrita** (mesmo tipo; `Int`/`Float` promovem
  numericamente como no `==`).
- Bitwise (`& | ^`, so `Int`): E/OU/XOR bit a bit; `~x` inverte os bits de
  `Int`; `<<`/`>>` deslocam (deslocamento negativo ou >= 64 = erro).
- Ternario `c ? a : b` avalia so o ramo escolhido (`c` deve ser `Bool`).
  `a ?? b` avalia `a` uma vez: se nao-`null`, usa `a`; senao avalia `b`.
  Elvis `a ?: b` e acucar de parse para `a ?? b` (sem truthiness: aqui so
  `null` aciona `b`, divergencia deliberada do PHP).
- `Display`: `null`, `true`/`false`, inteiro, float, conteudo da string,
  `[a, b]` para arrays, `{k: v, ...}` (chaves ordenadas) para mapas,
  `<fn nome>` para funcoes.
- `NaN`/`inf` (via `float("nan")`/`float("inf")`): `nan == nan` e `false`
  (IEEE 754); comparacoes ordenadas com `NaN` sao erro de tipos; `int()` de
  `NaN`/infinito e erro.
- Nomes de builtins tem precedencia em posicao de chamada: `len(x)` sempre
  chama o builtin mesmo havendo variavel `len` (leitura `len` sem chamada
  acessa a variavel). Evite declarar variaveis com esses nomes.
- `let` nao sombreia parametro da funcao no mesmo nivel: erro
  `variavel ja declarada`.

## 5. Compostos (Fase 3: semantica por valor/copia, sem aliasing)

- Array: `[1, "a", true]` (heterogeneo permitido, vazio `[]`).
  `a[i]` com `i: Int` em `[0, len)`; fora disso ou negativo = erro.
- Mapa: `{nome: "rhp", "v": 3}` (chaves string ou identificador; vazio `{}`).
  `m[k]` exige chave existente (`Str`); ausente = erro. Inserir chave nova
  por atribuicao e permitido. **Chaves duplicadas no literal: vence a
  primeira** (ordem do fonte; pino de compatibilidade).
- Atribuicao: `a[i] = v` (um nivel: `nome[expr] = valor`; leitura-escrita
  atomica na variavel). Alvos maiores (`f() = v`, `a[0][1] = v`) sao erro
  de parse ("alvo de atribuicao invalido").
- `==`/`!=` comparam arrays e mapas por conteudo; ordenacao (`<` etc.)
  com compostos e erro.

## 6. Biblioteca-padrao nucleo (builtins; aridade checada em compilacao)

| Builtin | Efeito |
|---|---|
| `len(x)` | Tamanho: array, mapa ou string (conta caracteres). |
| `push(arr, v)` | Novo array com `v` ao fim (nao muta o original). |
| `pop(arr)` | Ultimo elemento; array vazio = erro. |
| `keys(map)` | Array de chaves ordenado. |
| `str(v)` | Representacao `Display`. |
| `int(v)` | `Int`->igual; `Float` integral->trunca faixa; `Str`->parse; resto erro. |
| `float(v)` | `Float`->igual; `Int`->converte; `Str`->parse; resto erro. |
| `type(v)` | `"null"`, `"bool"`, `"int"`, `"float"`, `"string"`, `"array"`, `"mapa"`, `"funcao"`. |
| `assert(cond)` | `true`->`null`; `false`/nao-bool = erro. |
| `contains(s, parte)` | `true` se `parte` ocorre em `s` (ambos `Str`). |
| `starts_with(s, prefix)` | `true` se `s` comeca com `prefixo`. |
| `ends_with(s, sufixo)` | `true` se `s` termina com `sufixo`. |
| `split(s, sep)` | `Array` de pedacos; separador vazio divide em caracteres. |
| `join(arr, sep)` | Junta `Array` de `Str`; nao-`Str` = erro. |
| `trim(s)` | Remove espacos das pontas (Unicode). |
| `upper(s)` / `lower(s)` | Caixa alta/baixa (Unicode). |
| `replace(s, de, para)` | Substitui todas as ocorrencias. |
| `abs(x)` | Valor absoluto (`Int` com checagem; `Float` direto). |
| `min(a, b)` / `max(a, b)` | Minimo/maximo numericos (misto vira `Float`). |
| `sqrt(x)` | Raiz quadrada (`Float`); negativo = erro. |
| `pow(base, exp)` | Potencia (`Float`; tipos numericos). |
| `floor(x)` / `ceil(x)` | Para baixo/cima (`Int`); fora de `i64` = erro. |
| `values(m)` | `Array` com os valores (ordem das chaves). |
| `entries(m)` | `Array` de pares `[chave, valor]` (ordem das chaves). |
| `merge(a, b)` | Une dois mapas; chaves de `b` vencem. |
| `has(m, chave)` | `true` se a chave existe. |
| `fs_read(caminho)` | Conteudo do arquivo (`Str` UTF-8); erro de I/O ou > 8 MiB = erro. |
| `fs_write(caminho, texto)` / `fs_append(caminho, texto)` | Escreve/anexa; retorna bytes escritos. Caminhos relativos resolvem no diretorio atual; roda com as permissoes do usuario. |
| `fs_exists(caminho)` | `true` se legivel (qualquer falha = `false`). |
| `fs_list(dir)` | (RFC 0012) Nomes das entradas ordenados (`Array` de `Str`; `[]` se vazio); inexistente/nao-diretorio = erro. |
| `rand()` | (RFC 0012) `Int` em `[0, i64::MAX]` (xorshift64*; NAO criptografico). |
| `rand_range(min, max)` | (RFC 0012) `Int` em `[min, max]` (inclusive); `min > max` ou nao-`Int` = erro. |
| `hash_password(senha)` | (RFC 0013) Argon2id (`$argon2id$v=19$...`, defaults OWASP); `Str` requerida. |
| `verify_password(senha, hash)` | (RFC 0013) `Bool`; aceita `$argon2id$` e bcrypt `$2a$`/`$2b$`/`$2x$`/`$2y$` (migracao de bancos PHP); resto = erro; teto anti-DoS (m<=64MiB, t<=16, p<=4). |
| `regex_match(s, padrao)` | (RFC 0014) `Bool`; tempo linear garantido, sem lookaround/backrefs (erro de compilacao do padrao). |
| `regex_replace(s, padrao, subst)` | (RFC 0014) `Str`; suporta `$1`/`$name` na substituicao. |
| `regex_split(s, padrao)` | (RFC 0014) `Array` de `Str` (vazias incluidas; sem limite de partes). |
| `sha256(s)` | (RFC 0014) Hex lowercase de SHA-256; puro Rust. |
| `hmac_sha256(chave, msg)` | (RFC 0014) Tag hex HMAC-SHA256; comparacao em tempo constante via `hmac_verify`. |
| `hmac_verify(chave, msg, tag)` | (RFC 0014) `Bool`; tag fora do formato hex = erro. |
| `argv()` | `Array` de `Str` com os argumentos apos o arquivo (`rhp run prog.rhp a b` => `["a", "b"]`). |
| `env(nome)` | Valor da variavel de ambiente ou `null` se ausente. |
| `sort(arr)` | Copia ordenada (estavel): numeros (`Int`/`Float`, mistos comparam exato quando possivel), `Str` (ordem lexicografica de bytes UTF-8) ou `Bool` (`false` < `true`); vazio => vazio. Mistura com outros tipos ou nao-array = erro. Nao muta o original (`a = sort(a);`). |
| `map(arr, f)` | (RFC 0005) Aplica `f(elem)` a cada elemento e devolve array de resultados (mesma ordem); vazio => `[]` sem chamar. `f` declara exatamente 1 param. |
| `filter(arr, f)` | (RFC 0005) Devolve elementos cujo `f(elem)` retorna `Bool(true)` (ordem preservada); vazio => `[]`. Retorno nao-`Bool` = erro (sem truthiness). `f` declara exatamente 1 param. |
| `usort(arr, cmp)` | (RFC 0005) Copia ordenada por `cmp(a, b): Int` (`< 0`: `a` antes; `> 0`: depois; `0`: empate, estavel — insertion sort). Retorno nao-`Int` = erro. Vazio/unitario nao chama. Nao muta o original. |
| `exit(codigo)` | Encerra o programa com codigo 0-255 (fora disso ou nao-`Int` = erro). Desenrola chamadas; em `rhp test`, conta como falha do arquivo. |
| `now()` | Segundos desde 1970-01-01T00:00:00Z (`Int`; relogio do sistema). |
| `sleep(s)` | Bloqueia `s` segundos (`Int`/`Float`; fracao = sub-segundo). Negativo, nao-finito ou > 3600 = erro. Devolve `null`. Tarefas `async` rodam em paralelo (cada uma na sua thread). |
| `date_str(ts)` | Epoch `Int` => `"AAAA-MM-DDTHH:MM:SSZ"` (UTC, sempre; sem timezones). |
| `json_parse(texto)` | Texto JSON => valor (`null`/`Bool`/`Int` se inteiro em `i64` senao `Float`/`Str`/`Array`/`Mapa`). Gramatica estrita; erro com posicao (byte). Objetos: primeira chave vence (regra dos literais). Teto de 128 niveis. |
| `json_stringify(valor)` | Valor => texto JSON canonico (chaves ordenadas, sem espacos; `2.0` para floats inteiros). `NaN`/infinito e `Func` = erro. Teto de 128 niveis. |
| `http_get(url)` | (Fase 6) GET `http://host[:porta]/caminho` com timeouts de 5s (conexao/leitura; DNS usa o resolvedor do sistema, sem timeout proprio), teto de corpo de 1 MiB, so 2xx e corpo UTF-8; resto = erro. Somente esquema `http://`. Sem redirects (use `http_fetch`). |
| `http_fetch(url, opts)` | (Fase A5) GET com `opts` (tudo opcional): `seguir` (bool, padrao sim), `max_redirecionamentos` (0-10, padrao 5), `timeout` (segundos 0-60, padrao 5), `headers` (mapa `Str`->`Str`), `metodo` (so `GET`). Devolve mapa `{status: Int, cabecalhos: Mapa (chaves minusculas), corpo: Str}` — status final mesmo fora de 2xx. Redirects 301/302/303/307/308 com `Location` (absoluta, raiz ou relativa); `https://` verificado (raizes embutidas + `RHP_CA_BUNDLE` com PEM extra); upgrade `http->https` seguido, downgrade recusado. Erro em transporte, timeout, teto 1 MiB, limite de redirects, opcao invalida. |
| `db_open(caminho)` | (Fase B) Abre/cria SQLite; devolve handle `Int`. Caminhos relativos resolvem no diretorio atual; roda com as permissoes do usuario. `db_open("pg:host=.. port=.. user=.. dbname=..")` conecta no PostgreSQL (auth `trust`; padroes host 127.0.0.1/porta 5432/user `postgres`/dbname = user). |
| `db_exec(h, sql, params)` | Uma instrucao (INSERT/UPDATE/DELETE/DDL...); placeholders `?` (SQLite) ou `$1` (PG), `params` array (`[]` sem params); devolve linhas afetadas (`Int`). Linhas de SELECT sao descartadas (use `db_query`). |
| `db_query(h, sql, params)` | Consulta; devolve array de mapas coluna => valor (`Int`/`Float`/`Bool`/`Str`/`null`; BLOB = erro no SQLite; tipos PG mapeados por OID, resto vira `Str`). |
| `db_close(h)` | Fecha o handle (reuso = erro); devolve `null`. Fim do programa fecha o resto. |
| `http_serve(porta, handler, opts)` | (Fase C/D) Servidor HTTP: `porta` 0–65535 (0 = livre do SO), `handler` = nome de `fn(req)` global, `opts` com `max_conexoes` (padrão 64), `max_requisicoes` (padrão 0 = para sempre) e `limite_taxa` (req/s por IP, padrão 0 = sem limite; excedeu = 429). Por request: `req` = mapa `{metodo, caminho, query, cabecalhos (minúsculas), cookies, corpo}`; retorno `Str` (corpo, 200) ou mapa `{status?, cabecalhos?, corpo?}`. Erro no handler = 500 sem vazar; `exit(codigo)` termina o servidor. Handlers sem estado (sem compartilhar nada entre requests). |
| `template(texto, valores)` | (Fase C) Interpola `{{chave}}` com escape HTML obrigatório (`&<>"'`); chave ausente/`{{` sem fechar/tipo inválido = erro; sem laços/condicionais. |
| `next(gen)` | (RFC 0018, Etapa 5) Retoma o gerador ate o proximo `yield` (devolve o valor); no fim, marca esgotado e devolve `Null`. Gerador esgotado = erro (teste com `exhausted` antes). Nao-gerador = erro. |
| `exhausted(gen)` | (RFC 0018, Etapa 5) `Bool`: `true` se o gerador terminou (ou nunca executou? Nao: so apos o fim; antes de qualquer `next` = `false`). Nao-gerador = erro. |
| `async_spawn("f")` | (RFC 0019, Etapa 5) Roda `f` (só `fn` de nivel superior, sem captura, nao-gerador) em thread propria e devolve handle `Int` imediatamente. Isolada: sem globais compartilhados (o worker recompila o programa). |
| `async_join(h)` | (RFC 0019, Etapa 5) Bloqueia ate a tarefa terminar; devolve o retorno (desserializado de JSON: null/bool/int/float/string/array/mapa; resto = erro). Handle one-shot (reuso = erro). Erro da tarefa = erro no `join` (`tarefa ... falhou: ...`). |

Exemplo:

```rhp
let a = [1, 2];
a = push(a, 3);
echo len(a);      // 3
echo keys({b: 1, a: 2}); // [a, b]
try {
  echo json_parse("{oops}");
} catch e {
  echo e["mensagem"];   // detalhe do erro, sem abortar
} finally {
  echo "fim";
}
```

## 7. Erros (lex com linha:coluna base 1; runtime com mensagem)

Lexicos: caractere inesperado, string nao terminada, escape invalido,
numerico fora do intervalo. Sintaticos: token esperado (`;`, `)`, `}`,
identificador, expressao...). Runtime: tipos incompativeis, variavel
indefinida/redeclarada, condicao nao-bool, chamada de nao-funcao, aridade,
`return` fora de funcao, limite de chamadas, I/O.

Exemplos invalidos:

- `echo "x"` (falta `;`).
- `echo 1 + true;` (runtime: tipos incompativeis).
- `x = 1;` sem `let` previo (runtime: nao definida).
- `if 1 { }` (runtime: condicao deve ser bool).
- `echo [1][5];` (runtime: indice fora do intervalo).
- `echo {}["x"];` (runtime: chave inexistente).
- `try { echo 1; }` sem `catch`/`finally` (parse: "`try` exige ao menos").
- `throw "boom";` sem `try` (runtime: `excecao nao capturada: boom`).
- `import "a.rhp"; import "a.rhp";` com `a.rhp` importando de volta
  (erro: importacao circular).

## 8. Nao especificado ainda

Tipos/inferencia estatica, classes/interfaces/generics, `export`/privacidade
de modulos, metodos, `async`/concorrencia, PostgreSQL/Redis (ver ADR 0004),
formatter alem do canonico atual, LSP, otimizacoes (incluindo chamada em
cauda), semantica alem da descrita aqui.
