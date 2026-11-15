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
- **Modulos (Fase 4):** `import "caminho.rhp";` (caminho relativo ao arquivo
  que importa). Resolucao em profundidade: cada modulo executa uma vez, antes
  do importador; mesmo arquivo importado N vezes entra uma vez; ciclo
  (`a -> b -> a`) e arquivo ausente sao erros. Namespace global unico:
  redefinir no mesmo escopo (inclusive entre modulos) e erro em runtime.

```rhp
import "mat.rhp";
echo dobro(21); // 42
```

- **Limite de profundidade sintatica: 128 niveis** (expressoes e blocos
  aninhados; cadeias planas como `1+1+...` ou instrucoes em sequencia nao
  contam). Acima do limite, erro `profundidade maxima excedida`
  (protecao contra estouro de pilha — ver ADR 0005). Constantes de bytecode
  artesanais alem de 128 niveis sao rejeitadas na validacao.

## 2. Instrucoes

```text
Stmt       := LetStmt | AssignStmt | EchoStmt | IfStmt | WhileStmt | FnDef
           | ReturnStmt | Block | ExprStmt
LetStmt    := "let" Ident ("=" Expr)? ";"
AssignStmt := Target "=" Expr ";"
Target     := Ident | Ident "[" Expr "]"
EchoStmt   := "echo" Expr ";"
IfStmt     := "if" Expr Block ("else" Block)?
WhileStmt  := "while" Expr Block
FnDef      := "fn" Ident "(" (Ident ("," Ident)*)? ")" Block
ReturnStmt := "return" Expr? ";"
ImportStmt := "import" String ";"
ExprStmt   := Expr ";"
```

- `let x = e;` declara no escopo atual (`let x;` inicializa com `null`).
  Redeclarar no mesmo escopo e erro em runtime.
- `x = e;` exige variavel visivel (escopos internos -> externos -> globais).
- `echo e;` escreve `Display` do valor + `\n` (qualquer tipo).
- `if`/`while` exigem condicao `Bool` (sem truthiness implicita). `while`
  reavalia a condicao a cada iteracao. Blocos `{}` criam escopo; shadowing
  com `let` interno e permitido.
- `fn nome(params) { ... }` declara funcao no escopo atual. Chamada
  `nome(args)` avalia callee e argumentos da esquerda para a direita;
  aridade deve coincidir. `return e;` sai da funcao com o valor (`return;`
  retorna `null`); fim do corpo sem `return` tambem retorna `null`.
  `return` fora de funcao e erro de compilacao. Recursao permitida ate
  `MAX_CALL_DEPTH = 512` (erro apos o limite). Funcoes enxergam parametros,
  locais proprios e globais (sem closures sobre locais de outras funcoes).
- `ExprStmt` (ex.: `f();`) avalia e descarta o valor.

## 3. Expressoes e precedencia (da menor para a maior)

Igualdade (`==`, `!=`) < comparacao (`<`, `<=`, `>`, `>=`) < aditiva
(`+`, `-`) < multiplicativa (`*`, `/`, `%`) < unaria (`!`, `-`) < primaria.

```text
Primary   := Int | Float | String | "true" | "false" | "null"
           | Array | Map | Ident | Call | "(" Expr ")"
Array     := "[" (Expr ("," Expr)*)? "]"
Map       := "{" ((String | Ident) ":" Expr ("," ...)*)? "}"
Call      := Ident "(" (Expr ("," Expr)*)? ")"
// Posfixo: Primary ("[" Expr "]")*  (indexacao encadeada em leitura)
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
  valores: `Int`/`Float` comparam numericamente, funcoes por identidade,
  tipos diferentes sao diferentes.
- `Display`: `null`, `true`/`false`, inteiro, float, conteudo da string,
  `[a, b]` para arrays, `{k: v, ...}` (chaves ordenadas) para mapas,
  `<fn nome>` para funcoes.

## 5. Compostos (Fase 3: semantica por valor/copia, sem aliasing)

- Array: `[1, "a", true]` (heterogeneo permitido, vazio `[]`).
  `a[i]` com `i: Int` em `[0, len)`; fora disso ou negativo = erro.
- Mapa: `{nome: "rhp", "v": 3}` (chaves string ou identificador; vazio `{}`).
  `m[k]` exige chave existente (`Str`); ausente = erro. Inserir chave nova
  por atribuicao e permitido.
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
| `http_get(url)` | (Fase 6) GET `http://host[:porta]/caminho` com timeouts de 5s (conexao/leitura; DNS usa o resolvedor do sistema, sem timeout proprio), teto de corpo de 1 MiB, so 2xx e corpo UTF-8; resto = erro. Somente esquema `http://`. |

Exemplo:

```rhp
let a = [1, 2];
a = push(a, 3);
echo len(a);      // 3
echo keys({b: 1, a: 2}); // [a, b]
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
- `import "a.rhp"; import "a.rhp";` com `a.rhp` importando de volta
  (erro: importacao circular).

## 8. Nao especificado ainda

Tipos/inferencia estatica, classes/interfaces/generics, `export`/privacidade
de modulos, metodos, `async`/concorrencia, PostgreSQL/Redis (ver ADR 0004),
formatter alem do canonico atual, LSP, otimizacoes (incluindo chamada em
cauda), semantica alem da descrita aqui.
