# ADR 0002 — Biblioteca-padrão núcleo: compostos e builtins (Fase 3)

- Data: 2026-11-14
- Estado: aceita (autorização "até a última fase" + execução verificada)
- Fase: Fase 3 (arrays, mapas, indexação, builtins)

## Contexto

A linguagem precisava de valores compostos e operações básicas antes de
módulos e I/O. Restrições: std-only, sem `unsafe`, erros explícitos.

## Decisão

1. **Semântica por valor (cópia).** `Array(Vec<Value>)` e `Map(HashMap)` são
   clonados em atribuição/chamada/`push`. Sem aliasing, sem referência
   compartilhada: comportamento previsível e `Send`-amigável para o futuro.
2. **Indexação estrita.** `arr[i]` com `i` fora de `[0, len)` ou negativo é
   erro; `map[k]` com chave ausente é erro; tipos errados são erro.
   Atribuição indexada `nome[i] = v` só em um nível (`StoreIndex`), com
   leitura-escrita atômica na variável (erro preserva o valor antigo).
3. **Builtins como `CallBuiltin(id)`.** Nomes reservados na prática
   (`len push pop keys str int float type assert`); aridade checada em
   compilação. `Display` determinístico (mapas com chaves ordenadas).
4. **Conversões estritas.** `int("abc")`, `int(1.5)`, `pop([])` e `assert(x
   não-bool)` são erros; nada de coerção silenciosa.

## Alternativas consideradas

- Referência compartilhada (`Rc<RefCell>`): rejeitada — aliasing implícito
  contraria a governança (§5).
- `map[k]` ausente => `null`: rejeitado — erro explícito detecta bugs cedo.
- Métodos `arr.push(x)`: rejeitado por agora — sem sintaxe de métodos nesta
  fase; builtins cobrem o núcleo (métodos ficam para RFC futura).

## Consequências

- `Value` ganha `Array`/`Map`; 4 novos ops + `CallBuiltin`; `Display`
  especificado em `BYTECODE_SPEC.md`.
- Custo de cópia documentado como não-meta de performance.
