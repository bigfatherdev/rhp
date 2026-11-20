# RFC 0024 — Excecoes tipadas por hierarquia

- ID: RFC 0024
- Status: aceita e implementada
- Data: 2026-11-19
- Versao: unreleased (0.4.0 planejada)

## 1. Problema

`try/catch e` captura tudo num unico ramo, sem distinguir
`ValidationException` de `DatabaseException`; selecao manual via
`if type(...)` e fragil e nao respeita heranca.

## 2. Objetivo e nao-objetivos

- Objetivo: multiplos `catch [Tipo] [var]` em ordem, com `Tipo` casando
  instancias da classe ou subclasses; `catch` sem tipo por ultimo (legado).
- Nao-objetivos: `Exception` base obrigatoria, `finally` com valor,
  filtros por predicado, stack traces (documentado como ausente).

## 3. Proposta

```rhp
try {
  throw new ValidationException("email invalido");
} catch ValidationException e {
  echo e.message();
} catch DatabaseException e {
  echo "banco";
} catch e {
  echo "geral";
} finally {
  echo "fim";
}
```

- Heuristica de parse (compativel): `catch { }` (sem nada), `catch e`
  minusculo (var legada), `catch Tipo` maiusculo (tipo sem var),
  `catch Tipo var` (dois idents).
- Compilacao: cadeia com `Dup` + `IsInstanceOf` + `JumpIfFalse`; primeiro
  match consome e executa; sem match no ultimo tipado = `Rethrow`.
  `catch` sem tipo apos outros = erro (inalcancavel).
- `IsInstanceOf`: desempilha valor, empilha `Bool`; instancas via cadeia
  `ClassDef.parent` (limite 256); nao-instancias nunca casam.
- `finally` preservado em todos os caminhos (normal e erro).

## 4. Alternativas consideradas

- Tabela de tipos global com `instanceof` builtin: rejeitada (op dedicado
  e mais barato e preciso no fluxo de pilha).
- Exigir `extends Exception`: rejeitado (base por convencao; qualquer
  classe serve como tipo).

## 5. Compatibilidade

Compativel: `catch e`/`catch` legados viram clausula sem tipo (primeira e
unica). Multiplos `catch` sao sintaxe nova.

## 6. Seguranca

Sem vazamento de detalhes internos: a mensagem de erro segue a mesma;
tipos sao nomes de classe (sem enderecos). `exit()` nunca capturado
(como antes).

## 7. Performance

O(1) por guarda tipada no caminho de erro (fora do caminho feliz);
cadeia linear no numero de catches (tipico < 5).

## 8. Plano de implementacao

Parser (clausulas) -> compilador (`compile_try_multi`, `IsInstanceOf`) ->
VM (cadeia `parent`, checagem) -> fmt/linker -> testes.

## 9. Plano de testes

Captura por tipo, por pai (subclasse casa `catch` do pai), ordem,
generico final, propagacao, `finally` sempre.

## 10. Migracao e rollout

Nenhuma; adotar tipos incrementalmente.

## 11. Criterios de aceitacao

Tipo casa subclasse; generico captura resto; `finally` respeitado;
erros antigos intactos.
