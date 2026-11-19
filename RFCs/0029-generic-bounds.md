# RFC 0029 (proposta) — Restricoes de generics (`bounds`)

- ID: RFC 0029
- Status: proposta (nao implementada)
- Data: 2026-11-19
- Versao: —
- Depende de: RFC 0025 (generics por apagamento)

## 1. Problema

Generics apagados (RFC 0025) nao expressam requisitos (`T` com metodo
`id()`, `T` instancia de `Base`), entao erros so aparecem no uso, dentro
do corpo generico, com mensagem do ponto de falha e nao do contrato.

## 2. Objetivo e nao-objetivos

- Objetivo: `bounds` minimos e apagaveis — `<T: Classe>` (subclasse) e
  `<T: interface>` (contrato) — checados NA INSTANCIACAO (`new`/`call`),
  sem mudar o runtime (apagamento mantido).
- Nao-objetivos: inferencia, variancia declarada, bounds multiplos
  complexos (`T: A + B` fica para depois), especializacao, reflexao.

## 3. Proposta

```rhp
class Repo<T: Entidade> {
  fn salvar(v: T) { echo v.id(); }
}
interface Entidade { fn id(); }
class Usuario implements Entidade {
  fn novo() { this["id"] = 1; }
  fn id() { return this["id"]; }
}
let r = new Repo<Usuario>();
```

- Sintaxe: `T: Tipo` em `<...>` de `class`/`fn`/`interface` (um bound por
  parametro em v1).
- Checagem em compilacao QUANDO o argumento de tipo e conhecido
  (`new Repo<Usuario>` vs bound `Entidade`: precisa da hierarquia/contrato
  — tabelas ja existem no compilador). Sem argumento de tipo (`new Repo()`)
  = sem checagem (legado apagado).
- Erro: `` `Usuario` nao satisfaz `T: Entidade` em `Repo<...>` ``.

## 4. Alternativas consideradas

- Checagem em runtime com tags: rejeitada (custo por chamada; contradiz o
  apagamento aprovado na RFC 0025).
- `where` clauses: adiado (sintaxe extra sem caso de uso fechado).

## 5. Compatibilidade

Aditiva: codigo sem bounds comporta-se igual; `:` em `<T: X>` e novo nesse
contexto (fora dele, nada muda).

## 6. Seguranca

So compilacao (sem valores novos, sem recursion extra); limite de cadeia
`parent` ja existe (256).

## 7. Performance

Zero custo em runtime (checagem em compilacao, apagamento mantido).

## 8. Plano de implementacao

Parser (`T: Tipo` + qualificacao) -> tabelas do compilador (bound por
parametro; checagem em `New`/`Call` com type args) -> fmt/lsp -> testes.

## 9. Plano de testes

Bound satisfeito passa; violado falha com mensagem; sem type args passa;
bound para interface exige o metodo com aridade.

## 10. Migracao e rollout

Opcional; bounds podem ser adicionados depois sem quebrar usos existentes
(que passam a ser checados — documentar como `Changed` menor).

## 11. Criterios de aceitacao

Violacao detectada em compilacao com mensagem; apagamento preservado.
