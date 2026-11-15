# Language Reference — guia rapido do RHP

> Guia com exemplos executaveis. O normativo esta em `LANGUAGE_SPEC.md`
> (sintaxe/semantica), `BYTECODE_SPEC.md` e `VM_SPEC.md`. Em divergencia,
> valem as specs.

## Tipos e literais

`null`, `Bool` (`true`/`false`), `Int` i64 (`42`), `Float` (`1.5`),
`Str` (`"a\nb\u{41}"`), `Array` (`[1, "a"]`), `Map` (`{x: 1}`),
`Func` (`<fn f>` no `echo`).

## Variaveis e escopos

```rhp
let x = 1;      // declara (ou `let x;` => null)
x = x + 1;      // atribui (precisa existir)
{ let x = 2; }  // bloco: shadowing permitido
```

Globais: `let`/`fn` no topo do programa principal (visiveis nas funcoes).
Locais: parametros + lets da ativacao. `let` nao sombreia parametro.

## Operadores (precedencia: `==` < comparacao < `+-` < `*/%` < unario)

`+ - * / %` (checados; `+` concatena `Str`; `%` so `Int`),
`== != < <= > >=` (numeros com promocao, `Str` lexicografica,
`==` geral), `!`/`-` unarios (`+x` = identidade). `Display`: `1.5`, `[1, 2]`,
`{a: 1}`, `<fn f>`.

## Controle e funcoes

```rhp
if x > 1 { echo "maior"; } else { echo "menor"; }
while i < 3 { i = i + 1; }
fn soma(a, b) { return a + b; }
echo soma(3, 4);   // 7
```

`if`/`while` exigem `Bool`. `return` (com ou sem valor) so em funcao;
fim do corpo retorna `null`. Recursao ate 512 frames.

## Compostos e builtins

```rhp
let a = [1, 2]; a[0] = 9; a = push(a, 3);
echo len(a); echo pop(a); echo keys({b: 1});
echo str(42); echo int("42"); echo float(2); echo type(a);
assert(a == [9, 2, 3]); echo http_get("http://127.0.0.1:8080/x");
```

Index fora do intervalo, chave ausente, tipos errados e aridade divergente
sao erros. Chaves duplicadas no literal: vence a primeira.

## Modulos

```rhp
import "lib.rhp";   // nivel superior, caminho relativo, dentro da arvore
```

DFS, uma execucao por modulo, ciclo/ausente/grande demais = erro.

## Erros

Mensagens `arquivo:linha:coluna: motivo` (catalogo em
`docs/DIAGNOSTICS.md`). Erros comuns: `;` ausente, `alvo de atribuicao
invalido`, `variavel nao definida`, `condicao deve ser bool`.
