# VM_SPEC.md — Maquina virtual (Fases 3–6 implementadas)

## 1. Arquitetura

- Pilha de frames explicitos (`MAX_CALL_DEPTH = 512`); cada frame tem codigo,
  constantes, nomes, funcoes (`Rc`), `ip`, pilha de operandos e pilha de
  escopos locais (`scopes[0]` = ativacao: vazio no principal, params na funcao).
- Mapa de globais compartilhado: `let`/`fn` no topo do programa principal
  declaram globais, visiveis a todas as funcoes. Funcoes enxergam params,
  locais proprios e globais.
- Entrada: `&Chunk` validado recursivamente. Saida: `&mut dyn Write`.

## 2. Ciclo de execucao

1. `validate()` recursivo; invalido => `VmError("bytecode invalido: ...")`.
2. Frame principal; loop ate `Halt` (so valido com um frame) ou `Return`
   (desempilha frame, entrega valor ao chamador).
3. `Call(a)`: `pop` callee + `a` args (ordem preservada), checa tipo `Func`,
   aridade e profundidade; novo frame com params vinculados; continua nele.
4. Fim do codigo sem `Halt`/`Return` alcancado => erro (defesa em
   profundidade; chunks validados nao atingem).

## 3. Valores e erros

- `Display`: `null`, `true`/`false`, inteiro, float, string crua,
  `[a, b]` (arrays), `{k: v, ...}` com chaves ordenadas (mapas),
  `<fn nome>`.
- Compostos por valor: atribuicao/chamada/`push` copiam; `push` retorna novo
  array; `pop` retorna o ultimo (vazio = erro); `keys` ordenado.
- `http_get(url)`: somente `http://`, timeouts 5s, 2xx + corpo UTF-8;
  resto = erro.
- `VmError { message }`: sem panics para entradas nao confiaveis; sem
  `unwrap`/`expect` em producao. Mensagens em portugues, ex.:
  `variavel nao definida: x`, `variavel ja declarada: x`,
  `condicao deve ser bool, encontrado int`,
  `tipos incompativeis para \`+\`: int e bool`, `divisao por zero`,
  `estouro de inteiro em \`+\``, `tentativa de chamar int como funcao`,
  `funcao \`f\` espera 1 argumentos, recebeu 2`,
  `limite de chamadas excedido (recursao muito profunda?)`,
  `` `return` fora de funcao ``, `indice 5 fora do intervalo (tamanho 2)`,
  `chave inexistente no mapa: k`, `` `http_get` retornou status 404 ``.

## 4. Nao especificado ainda

Limites de operandos/heap, chamada em cauda, closures, excecoes, GC,
threads/async, debug/introspeccao, formato serializado, compatibilidade
entre versoes.
