# RFC 0017 — Classes mínimas (Etapa 5, OO sem herança)

- ID: RFC-0017
- Data: 2026-11-16
- Estado: aceita (autorização prévia do programa Etapas 3–5) e implementada
- Contexto: Etapa 5, item "OO". Primeira OO do RHP, deliberadamente sem
  herança, sem visibilidade, sem estáticos de classe.

## 1. Problema

Programas maiores precisam agrupar estado + comportamento com
identidade (`Counter`, `Sessao`, handlers). Hoje só há mapas + closures
avulsas, sem construtor, sem `this`, sem tabela de métodos.

## 2. Objetivo e não-objetivos

Objetivo: `class`, `new`, `obj.metodo()`/`obj.campo`/`obj.campo = v`,
`this`; construtor = método `novo`; métodos com `this` vinculado;
erros claros (classe inexistente, método ausente, `new` sem `novo`).

Não-objetivos: herança/`extends`, `private`, métodos estáticos,
interfaces/traits, sobrecarga, destrutores, `instanceof` (use
`type()`), propriedades computadas, classes anonimas.

## 3. Proposta

- Sintaxe: `class Nome { fn m(params) { } ... }`; `new Nome(args)`
  (só posicionais v1); `obj.metodo(args)`, `obj.campo`, `this`
  (identificador comum, vinculado pelo método).
- Runtime: `Value::Class(Rc<ClassDef>)`,
  `Value::Instance { class, fields }`. Métodos = `FuncDef` na classe;
  `Index` em instância devolve campo ou método vinculado
  (`Func` com `this` no env capturado). `StoreIndex` escreve campos.
- `new`: `Op::New` ativa `novo` com `this` = instância vazia; retorno
  ignorado (a instância — mutada ou não — é o resultado); sem `novo`
  = erro; `novo` com aridade errada = erro normal de chamada.
  Implementado via `pending_new` no frame chamador (mesmo protocolo do
  `pending` de HOF, RFC 0005).
- `type()` → `"classe"` / `"instancia"`; `echo` de instância = campos
  como mapa; classe = `<classe Nome>`; `==` estrutural (nome + campos).
- `try`/`catch`, `fmt` (reemite), linker (renomeia corpos) cobrem classes.

## 4. Alternativas

- Instância = mapa com tag `"__classe__"`: polui `keys()` e é
  falsificável pelo usuário — rejeitada (variante própria).
- Construtor = retorno do `novo` como instância: exige interceptar o
  retorno com a mesma maquinaria (`pending_new`) por ganho nulo de
  simplicidade — rejeitada (retorno ignorado e documentado).
- `new` sem `novo` (instância vazia direta): dois caminhos de
  construção (com/sem construtor) — rejeitada (regra única).
- Herança já: adiada para RFC própria (risco de semântica de `this`/
  `super` sem prova de necessidade).

## 5. Compatibilidade

Aditivo: `new`/`class`/`this` viram keywords (precedente: `match`,
`static`; 0.x). Sem anotação de tipo nova (`: classe`? não incluído —
`check_type` rejeita instância/classe; documentado).

## 6. Segurança

Sem `unsafe`; `Rc` só em metadados imutáveis (padrão Etapa 3). Erros
em vez de panic em: classe ausente, método ausente, aridade,
`new` em não-classe, `Index` em tipo errado (existente).

## 7. Performance

`Index` em instância: 1 lookup em campos + (se ausente) 1 em métodos
+ clone de env + insert `this` (só no acesso a método, não na chamada).
`new`: 1 ativação + 1 frame extra (como `Call`). Sem regressão no bench
(sem classes nos fixtures).

## 8. Plano de implementação

1. Lexer: `new`, `class` (keywords), `.` (`Dot`; `..` continua erro).
2. Parser: `Stmt::ClassDef`, `Expr::New/Member`, `this` = `Ident`.
3. Bytecode: `Value::Class/Instance`, `Op::BuildClass/New`, `Index`
   em instância, `check StoreIndex`, validador.
4. VM: `pending_new`, `this`, método vinculado, `Display`/`type`/`==`.
5. Fmt, linker (Renamer), `LANGUAGE_SPEC`, exemplo, `changes/`.

## 9. Plano de testes

- Ciclo: classe, `new`, campos, métodos, `this`, mutação, erro
  (sem `novo`, método ausente, aridade, `new` em não-classe).
- Método destacado (`let f = obj.m; f()`) com `this` correto.
- `try` captura erro de método; `fmt` idempotente; linker renomeia.
- Suite inteira verde (regressão zero).

## 10. Migração e rollout

Nenhuma migração (aditivo; novas keywords reservadas em 0.x).

## 11. Critérios de aceitação

- [x] RFC aceita antes do código (autorização prévia 2026-11-16).
- [ ] Ciclo completo + 6 erros pinados + método destacado.
- [ ] Suite verde; `fmt`/`clippy` limpos; spec + exemplo + `changes/`.
