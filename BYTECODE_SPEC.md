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
| `Neg` / `Not` | Negacao numerica / logica (tipos estritos, erro caso contrario). |
| `Eq Ne` | Igualdade geral (empilha `Bool`). |
| `Lt Le Gt Ge` | Comparacao numero-numero ou `Str`-`Str` (empilha `Bool`). |
| `JumpIfFalse(t)` | `pop Bool`; se falso, `ip = t`. Nao-bool = erro. |
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
| `StoreIndex(n)` | `pop` valor e indice; grava em `names[n][indice]` existente. |
| `CallBuiltin(id)` | Executa builtin de id 0–9 com aridade fixa (tabela §4). |

## 3. Compilacao (Marco 2 + Fases 3–4)

- Expressoes: pos-fixa direta; `Call` = `LoadName(f)` + args + `Call(n)`;
  builtins viram `CallBuiltin` direto (aridade checada aqui).
- Literais compostos: itens/valores + `BuildArray(n)`/`BuildMap(n)`;
  `a[i]` = base + indice + `Index`; `nome[i] = v` = indice + valor +
  `StoreIndex(nome)`.
- `let` = valor/`Null` + `DeclareName`; atribuicao = valor + `StoreName`.
- `if` = cond + `JumpIfFalse->else`; blocos com `EnterScope`/`ExitScope`;
  `else` opcional via `Jump->fim`. `while` = rotulo + cond +
  `JumpIfFalse->fim` + corpo + `Jump->rotulo`.
- `fn` = corpo compilado em `Compiler` aninhado + epilogo
  `Constant(Null); Return; Halt` + `DefineFunc`. `return e;` = valor + `Return`
  (`return` no topo = erro de compilacao).
- `Stmt::Import` e consumido pelo carregador de modulos; a compilacao direta
  o ignora (programa deve chegar resolvido).

## 4. Builtins (ids estaveis)

`0 Len(1) 1 Push(2) 2 Pop(1) 3 Keys(1) 4 Str(1) 5 Int(1) 6 Float(1)
7 Type(1) 8 Assert(1) 9 HttpGet(1)` — ids desconhecidos invalidam o chunk.

## 4. Validacao (obrigatoria, recursiva)

`Chunk::validate()`: codigo nao vazio, ultimo op `Halt`, `Constant`/
`LoadName`/`StoreName`/`DeclareName`/`DefineFunc` dentro dos pools,
`Jump`/`JumpIfFalse` com alvo `< code.len()`, e recursao nos corpos de
`fn` (contexto nomeado na mensagem). A VM nunca executa chunk invalido,
nem parcialmente.

## 5. Nao especificado ainda

Codificacao binaria em disco, versao de formato, modulos pre-compilados,
internacao/otimizacoes, limites de pilha de operandos/heap, compatibilidade
entre versoes (nenhuma prometida).
