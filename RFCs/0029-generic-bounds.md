# RFC 0029 (proposta) — Restricoes de generics (`bounds`)

- ID: RFC 0029
- Status: aceita e implementada
- Data: 2026-11-19
- Versao: unreleased (0.5.0 planejada)
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
  parametro em v1). Type args preservados no AST (`CallData`/`NewData`);
  `Expr::{Call,New}` encaixotados (reduz `sizeof(Expr)`, preserva o guarda
  de profundidade).
- Checagem em compilacao QUANDO ha argumentos de tipo (`new Repo<Usuario>`
  vs bound `Entidade`: hierarquia/contrato das tabelas do compilador).
  Sem argumentos de tipo (`new Repo()`, `f(x)`) = apagado, sem checagem
  (legado). Aridade e nao-generica-com-args tambem validadas.
- Parametro de tipo visivel no escopo adia a checagem (`new Caixa<T>` dentro
  de `fn f<T>` passa; sera verificado na instanciacao concreta).
  Desconhecido fora de escopo = erro (inclui primitivos como `int`).
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
bound para interface exige o metodo com aridade; aridade e nao-generica com
args falham; parametro visivel adia; builtin com type args falha.
Implementado: teste de unidade `checks_generic_bounds_at_compile_time`,
integracao `nextgen_generic_bounds`, idempotencia do fmt com bounds.

## 10. Migracao e rollout

Opcional; bounds podem ser adicionados depois. Novo erro para: aridade
divergente, nao-generica com args, builtin com args, bound violado ou
desconhecido, tipo desconhecido no uso (documentar como `Changed` menor —
so atinge codigo sem sentido ou com nomes errados).

## 11. Criterios de aceitacao

Violacao detectada em compilacao com mensagem; apagamento preservado.
