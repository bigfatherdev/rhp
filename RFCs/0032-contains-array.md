# RFC 0032 (proposta) — `contains` aceita array (pertinencia)

- ID: RFC 0032
- Status: aceita e implementada
- Data: 2026-11-20
- Versao: unreleased (implementada na 0.7.0)
- Depende de: nenhuma

## 1. Problema

Sem teste de pertinencia em array (RHP-AUD-004): `has` e mapa-only,
`contains` e string-only, sem operador `in`. Workaround e loop manual —
ruim para o caso mais comum (`if contains(pedidos, id)`).

## 2. Objetivo e nao-objetivos

- Objetivo: `contains(array, valor)` => `Bool` usando a igualdade `==`
  existente (por valor, mesma de `==`/`!=`).
- Nao-objetivos: operador `in` (sintaxe nova sem necessidade),
  `contains(mapa, chave)` (usar `has`), comparador customizado (usar
  `filter` + `len`), busca profunda customizada.

## 3. Proposta

```rhp
echo contains([1, 2, 3], 2);   // true
echo contains(["a"], "b");     // false
echo contains([], 1);          // false (sem erro)
```

- Sobrecarga compativel: hoje `contains(array, _)` e erro; passa a
  funcionar. Strings e aridade inalterados.
- Igualdade por valor (`==` do RHP: numeros com promocao, `==` estrutural
  de colecoes como ja definido). `NaN` nunca e igual (coerente com `==`).
- Mensagem de tipo passa a citar os tres casos
  (`(string, string)`, `(array, valor)`).

## 4. Alternativas

- Operador `in`: sintaxe nova, precedencia nova, mais superficie — sem
  ganho sobre a chamada.
- Novo builtin (`member`, `inclui`): prolifera API; `contains` ja diz isso.
- Comparador callback: `filter` cobre; manter `contains` simples e rapido.

## 5. Compatibilidade

Programas validos inalterados (array em `contains` era erro). Sem mudanca
de bytecode/ABI (mesmo `Builtin::Contains`).

## 6. Seguranca

O(n) com early-exit; sem alocacao. Nada recursivo (igualdade de colecoes
usa a existente).

## 7. Performance

Linear no tamanho; sem otimizacao especulativa (hashset mudaria semantica
de igualdade — nao fazer).

## 8. Plano de implementacao

1. Braco `(Value::Array(items), v)` em `Builtin::Contains`: iterar com
   `values_equal` existente (`rhp-vm`, mesma de `==`/`Eq`, com promocao
   numerica). Retorna no primeiro match (early-exit).
2. Mensagem de tipo com os casos. 3. Docs + testes.

## 9. Plano de testes

Vazio, 1 elem, N elems, tipos mistos, igualdade por valor (mapas/arrays
aninhados), `NaN`, nao-bool? (sempre Bool), erro em aridade, mensagem 2-arg.

## 10. Migracao e rollout

Nenhuma (erro vira valor). Documentar na linha do `contains`.

## 11. Criterios de aceitacao

Tabela de casos verde; suite verde; docs atualizadas.
