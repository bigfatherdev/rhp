# Diagnostics — catálogo de erros do RHP

Erros seguem o formato de referência (implementado por `rhp-diagnostic`,
independente de apresentação para futuro LSP):

```text
Error[E1004]: 1:13: esperado `;`, encontrado fim de arquivo
  --> app.rhp:1:13
1 | echo "x"
  |             ^
help: toda instrucao termina com `;`
```

- `E0xxx`: léxicos · `E1xxx`: sintáticos · `E2xxx`: compilação ·
  `E3xxx`: execução · `E4xxx`: carregador/módulos.
- Léxico/sintaxe incluem trecho da fonte com `^` (`len 0` = até o fim da
  linha). Compilação/execução não têm posição de fonte (bytecode) e saem
  como `Error[CODIGO]: mensagem` + `help:` quando aplicável.

## Léxicos (`erro lexico em`)

| Entrada | Código | Mensagem |
|---|---|---|
| `echo @;` | E0001 | `caractere inesperado: @` |
| `echo "abc` | E0002 | `string nao terminada` |
| `echo "a\qb";` | E0003 | `escape invalido: \q` (+ `\u` malformado) |
| `echo "a` + quebra | E0002 | `quebra de linha sem escape dentro de string` |
| `999…9` (> i64) | E0004 | `literal inteiro fora do intervalo` |

## Sintáticos (`erro sintatico em`)

| Entrada | Código | Mensagem |
|---|---|---|
| `echo "x"` | E1004 | `esperado ';'` (+ ajuda) |
| `echo ;` | E1001 | `esperado expressao…` |
| `if true { echo 1;` | E1002 | `esperado '}', encontrado fim de arquivo` |
| `f() = 1;` | E1003 | `alvo de atribuicao invalido…` |
| `let echo = 1;` | E1005 | `esperado identificador…` |
| `import 42;` / import aninhado | E1007 | caminho de módulo / nível superior |
| demais `esperado …` | E1006 | (genérico de sintaxe) |

## Compilação (`Error[E2xxx]`)

E2001 `return` fora de função · E2002 aridade de builtin · E2003 limites
(constantes/nomes/funções, profundidade) · E2004 chunk inválido · E2000
genérico.

## Execução (`Error[E3xxx]`)

E3001 tipos (`incompativeis`, `exige`, `nao converte`) · E3002
`nao definida`/`ja declarada` · E3003 condição não-bool · E3004 chamada
(`como funcao`, aridade) · E3005 `return` fora de função (interno) · E3006
limite de chamadas · E3007 índice/chave · E3008 I/O · E3009
modulos (`modulo`, `circular`, `fora da arvore`, `muito grande`, nível
superior) · E3010 limites (`instrucoes`, corpo HTTP) · E3011 `http_get` ·
E3000 genérico.

## Dicas recorrentes

Condicoes de `if`/`while` exigem `Bool` (E3003: use comparação explícita).
Tipos mistos (fora `Str+Str` e número+número) são erro E3001: converta com
`int()`/`float()`/`str()`. `a[i]`: confira com `len()`/`keys()` (E3007).
