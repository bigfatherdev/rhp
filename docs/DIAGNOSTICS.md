# Diagnostics — catalogo de erros do RHP

Formato geral: `origem:linha:coluna: mensagem` no stderr. O normativo e
`LANGUAGE_SPEC.md` (§ de erros); aqui vao os casos reais com exemplo.

## Lexicos (`erro lexico em`)

| Entrada | Mensagem |
|---|---|
| `echo @;` | `1:6: caractere inesperado: @` |
| `echo "abc` | `1:6: string nao terminada` |
| `echo "a\qb";` | `escape invalido: \q` |
| `echo "a\uz";` | `escape invalido: \u (use \u{HEX})` |
| `echo "a\u{110000}";` | ponto fora do intervalo / formato invalido |
| `99999…9` (> i64) | `literal inteiro fora do intervalo: …` |

## Sintaticos (`erro sintatico em`)

| Entrada | Mensagem |
|---|---|
| `echo "x"` | `esperado ';'` |
| `echo ;` | `esperado expressao, encontrado ';'` |
| `let echo = 1;` | `esperado identificador, encontrado 'echo'` |
| `f() = 1;` | `alvo de atribuicao invalido (use 'nome' ou 'nome[indice]')` |
| `if true { echo 1;` | `esperado '}', encontrado fim de arquivo` |
| `import 42;` | `esperado caminho de modulo (string)…` |
| aninhamento > 128 niveis | `profundidade maxima excedida em expressao (limite 128)` |
| `fn f(a, a)` | `parametro duplicado: a` |

## Compilacao (`erro de compilacao`)

`return` fora de funcao; aridade de builtin (`len` espera 1…);
chunk invalido (constante/nome/funcao fora do pool, salto fora do codigo,
builtin desconhecido, sem `Halt`, constante profunda demais).

## Execucao (`erro de execucao`)

Tipos incompativeis (`1 + true`), `divisao por zero`, `estouro de inteiro`,
`variavel nao definida: x` / `ja declarada: x`, `condicao deve ser bool`,
chamada de nao-funcao, aridade (`espera 1 argumentos, recebeu 2`),
`` `return` fora de funcao ``, `limite de chamadas excedido`,
`indice 5 fora do intervalo (tamanho 2)`, `chave inexistente no mapa: k`,
falhas de `assert`/`int`/`float`/`pop`, `http_get` (esquema, DNS, conexao,
status nao-2xx, corpo nao UTF-8, `excedeu o limite de corpo`),
`limite de instrucoes excedido`, `importacao circular`, import fora da
arvore, arquivo muito grande, `import` fora do nivel superior.

Dica: condicoes de `if`/`while` exigem `Bool` (sem truthiness); `+` com
tipos mistos (fora `Str+Str` e numero+numero) e sempre erro.
