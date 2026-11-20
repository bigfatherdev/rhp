# RFC 0025 — Generics por apagamento

- ID: RFC 0025
- Status: aceita e implementada
- Data: 2026-11-19
- Versao: unreleased (0.4.0 planejada)

## 1. Problema

Estruturas reutilizaveis (`Caixa`, `primeiro`) precisam repetir codigo por
tipo ou abrir mao de documentar o parametro.

## 2. Objetivo e nao-objetivos

- Objetivo: parametros `<T>` em `class`/`fn`, argumentos `<...>` em uso,
  `T` aceito em anotacoes — tudo apagado (sem checagem, sem custo).
- Nao-objetivos: monomorfizacao, bounds (`T: X`), variancia, inferencia,
  especializacao, reflexao de tipos.

## 3. Proposta

```rhp
class Caixa<T> {
  fn novo(v) { this["v"] = v; }
  fn obter() { return this["v"]; }
}
let c = new Caixa<int>(100);
fn primeiro<T>(items) { return items[0]; }
echo primeiro<int>([7, 8]);
```

- Parse valida forma (idents, `,`/`>`, duplicatas); bytecode/VM ignoram
  (apagamento total, sem ops novos).
- `T` em anotacao (`v: T`) = sem checagem (como sem anotacao).
- `<` binario preservado: `< T > (` seguido de `(` = generics; senao =
  comparacao (lookahead `looks_like_type_args_call`).

## 4. Alternativas consideradas

- Checagem em runtime com tags: rejeitada (custo por chamada, complexidade
  sem ganho para linguagem dinamica).
- Monomorfizacao: rejeitada (explosao de bytecode, sem necessidade).

## 5. Compatibilidade

Compativel: `<`/`>` seguem comparacao fora do padrao `Nome<T>(`.

## 6. Seguranca

Sem impacto (sem novos valores; sem recursion extra alem do parse
limitado por `MAX_PARSE_DEPTH`).

## 7. Performance

Zero custo em runtime (apagado no parse).

## 8. Plano de implementacao

Lexer (nada) -> parser (generics + desambiguacao `<`) -> compilador
(ignora) -> testes.

## 9. Plano de testes

Funcoes/classes genericas executam; `<` comparacao intacta (`for` com
`<`); `Expr` nao cresce (teste de profundidade passa).

## 10. Migracao e rollout

Aditivo e opcional.

## 11. Criterios de aceitacao

Sintaxe aceita e apagada; sem regressoes de precedencia.
