# RFC 0021 — Heranca, interfaces e traits

- ID: RFC 0021
- Status: aceita e implementada
- Data: 2026-11-19
- Versao: unreleased (0.4.0 planejada)

## 1. Problema

O RHP 0.2.0/0.3.0 possui classes minimas (RFC 0017): construtor `novo`,
metodos, `this` com write-back, sem heranca, sem contratos, sem
reutilizacao. Projetos medios repetem codigo e nao expressam hierarquias
(`Animal`/`Cachorro`) nem contratos (`Logger`).

## 2. Objetivo e nao-objetivos

- Objetivo: heranca simples, interfaces como contratos verificados em
  compilacao, traits como composicao com precedencia definida, `super`.
- Nao-objetivos: heranca multipla de classes, mixins com estado,traits com
  campos, metodos estaticos, visibilidade `private`, sobrecarga.

## 3. Proposta

```rhp
class Animal {
  fn novo(n) { this["nome"] = n; }
  fn falar() { return "Som"; }
}
class Cachorro extends Animal {
  fn falar() { return "Au au"; }
}
interface Logger { fn log(mensagem); }
class ConsoleLogger implements Logger {
  fn novo() { this["x"] = 0; }
  fn log(m) { echo m; }
}
trait Saudacao {
  fn ola() { return "ola"; }
}
class Pessoa {
  use Saudacao;
  fn novo() { this["x"] = 1; }
}
class Filha extends Base {
  fn quem() { return super.quem() + "!"; }
}
```

- `extends`: unico, classe pai definida antes (erro `desconhecida`).
- `implements I1, I2`: verifica em compilacao que todos os metodos existem
  com mesma aridade (params + `...rest`); ausente/incompativel = erro.
- `use T1, T2;` no corpo: composicao; precedencia propria > trait > pai;
  conflito trait-trait sem override = erro.
- `super.metodo(args)`: metodo do pai com o `this` atual; write-back no
  `this` do chamador. So em chamada de metodo dentro de metodo.
- Hierarquia circular e auto-heranca = erro. Metodos achatados na classe
  (copia dos `FuncDef` do pai/traits); `ClassDef.parent` guarda o nome para
  `super` e captura tipada.
- Novos ops: `BuildClassEx`, `CallSuper`. `ClassDef { name, parent, methods }`.

## 4. Alternativas consideradas

- Heranca prototipica/delegacao em runtime: rejeitada (custo por chamada,
  sem verificacao de interfaces em compilacao).
- Heranca multipla: rejeitada (diamante, sem caso em projetos-alvo).
- Traits com campos: rejeitado (estado compartilhado complica write-back).

## 5. Compatibilidade

Compativel: classes sem `extends`/`implements`/`use` compilam para o mesmo
`BuildClass` de antes. `super` e palavra nova (programas com variavel
`super` quebram — intencional, documentado).

## 6. Seguranca

Cadeia de heranca limitada a 256 niveis em `IsInstanceOf`/`super`
(bytecode artesanal nao causa loop). Sem alteracao de jaula de imports.

## 7. Performance

Metodos achatados: chamada O(1) por hash, sem cadeia por chamada. Custo de
compilacao linear nos metodos herdados (copia de indices, nao de corpos).

## 8. Plano de implementacao

Lexer (`extends`, `implements`, `interface`, `trait`, `use`, `super`) ->
parser (AST + validacoes) -> bytecode (tabelas + `BuildClassEx`/`CallSuper`)
-> VM (parent, super com `pending_super`) -> fmt/lsp/cli -> testes.

## 9. Plano de testes

Positivos (heranca, override, multi-interface, trait, super com mutacao) e
negativos (pai desconhecido, ciclo, interface ausente/incompativel, conflito
de traits, `super` fora de metodo). Integracao `nextgen_*`.

## 10. Migracao e rollout

Nenhuma migracao: codigo 0.3.0 segue valido. Adocoes incrementais.

## 11. Criterios de aceitacao

Classes derivadas funcionam; override vence; interfaces verificadas;
traits compostas com precedencia e erros claros; `super` com write-back;
sem regressoes (suite verde).
