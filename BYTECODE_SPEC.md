# BYTECODE_SPEC.md — Formato e semantica do bytecode (Fases 3–6 implementadas)

## 1. Modelo

```rust
pub enum Value { Null, Bool(bool), Int(i64), Float(f64), Str(String),
                 Array(Vec<Value>), Map(HashMap<String, Value>), Func(Rc<FuncDef>) }
pub struct FuncDef { pub name: String, pub params: Vec<String>, pub body: Chunk }
pub struct Chunk {
    pub constants: Vec<Value>, // pool indexado por u32
    pub names: Vec<String>,    // pool de nomes de variaveis/funcoes
    pub funcs: Vec<FuncDef>,   // tabela de funcoes definidas no chunk
    pub code: Vec<Op>,
}
pub enum Builtin { Len, Push, Pop, Keys, Str, Int, Float, Type, Assert, HttpGet } // ids 0..=9
```

Limites: `u32::MAX` entradas por pool/tabela (erro de compilacao acima).

## 2. Instrucoes

| Op | Efeito |
|---|---|
| `Constant(i)` | `push(constants[i])`. Erro se fora do pool. |
| `Print` | `pop v; output(v + "\n")`. Erro se pilha vazia/IO. |
| `Halt` | Encerra com sucesso (so valido no programa principal). |
| `Add Sub Mul Div Mod` | Binarias numericas com promocao (`Int`+`Float`->`Float`); `+` concatena `Str`; `%` so `Int`; checadas (estouro/div-zero/resto-zero = erro). |
| `Neg` / `Not` / `BitNot` | Negacao numerica / logica / bit a bit (`Int`; erro caso contrario). |
| `Eq Ne` | Igualdade geral (empilha `Bool`). |
| `Lt Le Gt Ge` | Comparacao numero-numero ou `Str`-`Str` (empilha `Bool`). |
| `BitAnd BitOr BitXor` | Operadores bitwise sobre `Int` (erro caso contrario). |
| `Shl Shr` | Deslocamento de `Int` por `Int`; deslocamento invalido = erro. |
| `Dup` | Duplica o topo da pilha. |
| `JumpIfFalse(t)` / `JumpIfNotNull(t)` | `pop`; se falso / se nao-`null`, `ip = t`. `JumpIfFalse` exige `Bool`. |
| `Jump(t)` | `ip = t`. |
| `LoadName(n)` | Empilha valor visivel de `names[n]` (locais -> globais). Erro se indefinido. |
| `StoreName(n)` | `pop` para variavel existente `names[n]`. Erro se indefinida. |
| `DeclareName(n)` | `pop` e declara `names[n]` no escopo atual (globais se no topo do principal). Erro se ja declarado no mesmo escopo. |
| `EnterScope` / `ExitScope` | Abre/fecha escopo de bloco (`ExitScope` sem par = erro). |
| `DefineFunc(f)` | Cria `Func(funcs[f])` e declara o nome no escopo atual. |
| `Call(a)` | `pop` callee + `a` args; cria frame; empilha retorno ao fim. Erros: callee nao-funcao, aridade, profundidade >= 512. |
| `Return` | Retorna `pop` ao chamador. Erro no programa principal. |
| `Pop` | Descarta topo (instrucao por expressao). |
| `BuildArray(n)` | `pop` n valores; empilha `Array` (ordem preservada). |
| `BuildMap(n)` | `pop` n pares chave(`Str`)/valor; empilha `Map`. |
| `Index` | `pop` indice e base; empilha `base[indice]` (regras §3). |
| `StoreIndex(n)` | `pop` valor e indice; grava em `names[n][indice]`. |
| `IterList` | `pop` iteravel; empilha lista de iteracao (`Array` = copia; `Mapa` = chaves `Str` ordenadas; resto = erro). |
| `TryBegin(t)` | Registra handler (`t` = `catch` ou `finally`+`Rethrow`). |
| `TryEnd` | Remove o handler (caminho normal). Sem handler = erro. |
| `Throw` | Desempilha valor e inicia unwinding (frame atual para a base). |
| `Rethrow` | Relanca o erro em propagacao (fim de `finally` sem `catch`). |
| `CallBuiltin(id)` | Executa builtin de id 0–39 com aridade fixa (tabela §4). |

## 3. Compilacao (Marco 2 + Fases 3–4)

- Expressoes: pos-fixa direta; `Call` = `LoadName(f)` + args + `Call(n)`;
  builtins viram `CallBuiltin` direto (aridade checada aqui).
- Literais compostos: itens/valores + `BuildArray(n)`/`BuildMap(n)`;
  `a[i]` = base + indice + `Index`; `nome[i] = v` = indice + valor +
  `StoreIndex(nome)`.
- `let` = valor/`Null` + `DeclareName`; `const` = igual + registro interno
  (atribuicao posterior = erro de compilacao); atribuicao = valor + `StoreName`.
- `if` = cond + `JumpIfFalse->else`; blocos com `EnterScope`/`ExitScope`
  (omitidos no ramo que sempre diverge); `else` opcional via `Jump->fim`.
  `while` = rotulo + cond + `JumpIfFalse->fim` + corpo + `Jump->rotulo`.
  O escopo do corpo fecha em toda saida: a condicao-falsa pula o
  `EnterScope`; `break`/`continue` desenrolam ate a base do laco.
- `for x in e { ... }` = escopo do laco + `e` + `IterList` + nomes ocultos
  (lista, indice) + rotulo + cond (`indice < len(lista)`) +
  `JumpIfFalse->fim` + escopo do corpo + `x = lista[indice]` + corpo +
  incremento + `Jump->rotulo` + saidas. `break` fecha corpo + laco e salta
  a saida (que fecha o escopo do laco); `continue` salta ao incremento.
  Nomes ocultos contem NUL (irrepresentaveis no fonte; sem colisao).
- `try B (catch V? C)? (finally F)?` = `TryBegin->alvo` + escopo + `B` +
  `ExitScope` + `TryEnd` + `F` (normal) + `Jump->fim`; alvo: `catch`
  (escopo + vinculo de `V`/descarte + `C` + `F`) ou, sem `catch`,
  `F` + `Rethrow`. `finally` duplicado por caminho (neutro na pilha).
- `break`/`continue` = desenrolha escopos ate a base do laco + `Jump`
  (saida/inicio ou incremento); fora de laco = erro de compilacao. `return` fora de funcao = erro.
  Cruzando `try` com `finally`, `break`/`continue`/`return` emitem antes
  `TryEnd` + copia do `finally` (do mais interno para fora).
- Ternario = cond + `JumpIfFalse->else` + ramo + `Jump->fim` + ramo-senão.
- `a ?? b` = `a` + `Dup` + `JumpIfNotNull->fim` + `Pop` + `b` (avalia uma vez).
- `fn` = corpo compilado em `Compiler` aninhado + epilogo
  `Constant(Null); Return; Halt` + `DefineFunc`. `return e;` = valor + `Return`
  (`return` no topo = erro de compilacao).
- `Stmt::Import` e consumido pelo carregador de modulos; a compilacao direta
  o ignora (programa deve chegar resolvido).

## 4. Builtins (ids estaveis)

`0 Len 1 Push 2 Pop 3 Keys 4 Str 5 Int 6 Float 7 Type 8 Assert
9 HttpGet 10 Contains 11 StartsWith 12 EndsWith 13 Split 14 Join
15 Trim 16 Upper 17 Lower 18 Replace 19 Abs 20 Min 21 Max 22 Sqrt
23 Pow 24 Floor 25 Ceil 26 Values 27 Entries 28 Merge 29 Has
30 FsRead 31 FsWrite 32 FsAppend 33 FsExists 34 Argv 35 Env
36 Sort 37 Exit 38 JsonParse 39 JsonStringify`
(aridades na tabela §6 de `LANGUAGE_SPEC.md`). Ids desconhecidos invalidam
o chunk; `BuiltinId` tem teste anti-deriva.

## 5. Validacao (obrigatoria, recursiva + fluxo de pilha)

`Chunk::validate()`: codigo nao vazio, ultimo op `Halt`, `Constant`/
`LoadName`/`StoreName`/`DeclareName`/`StoreIndex` dentro dos pools,
`DefineFunc` existente, `Jump`/`JumpIfFalse`/`JumpIfNotNull`/`TryBegin`
com alvo `< code.len()`, `CallBuiltin` com id conhecido, constantes dentro da profundidade maxima e,
por fim, **analise estatica de altura de pilha**: cada posicao alcancavel
tem altura deterministica; underflow estatico e juncao com alturas
divergentes sao erro. Alvo de `TryBegin` entra com altura do ponto + 1
(so o valor do erro); `Throw`/`Rethrow` terminam o caminho. Codigo
inalancavel apos `Return`/`Jump`/`Throw`/`Rethrow` e ignorado
pelo fluxo (mas segue sujeito as checagens estruturais). Recursao nos corpos
de `fn` (contexto nomeado na mensagem). A VM nunca executa chunk invalido,
nem parcialmente.

## 6. Nao especificado ainda

Codificacao binaria em disco, versao de formato, modulos pre-compilados,
internacao/otimizacoes, limites de pilha de operandos/heap, compatibilidade
entre versoes (nenhuma prometida).
