# [Added] — Classes mínimas: `class`, `new`, métodos, `this` (RFC 0017, Etapa 5)

- ID: 2026-11-16-feat-classes-minimas
- Data: 2026-11-16
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (aditivo; `class`/`new`/`this` viram
  keywords — precedente `match`/`static` em 0.x)

## Resumo

OO mínima sem herança: `class Nome { fn m() {} }`, `new Nome(args)`
(construtor `novo` obrigatório, retorno ignorado), `obj.metodo(args)`,
`obj.campo`, `obj.campo = v`, `this` vinculado. Instâncias por valor
(sem aliasing, como mapas); mutação via chamada direta em variável
persiste (write-back; coerente com `a[i] = v`).

## Motivação

Etapa 5, item "OO" (RFC 0017, aceita sob autorização prévia do
programa): agrupar estado + comportamento com identidade e
construtor, sem a complexidade de herança.

## O que mudou

- Lexer: keywords `class`/`new`, `.` (`Dot`; `..` continua erro;
  floats intactos).
- Parser: `Stmt::ClassDef`, `Expr::Member/New`, sufixo `.nome`,
  `parse_class_def` (só `fn`, duplicata = erro com posição),
  `parse_type_ann` reaproveitado nos métodos.
- Bytecode: `Value::Class(Rc<ClassDef>)`,
  `Value::Instance{class, fields}`, `ClassDef{name, methods}`,
  `Op::BuildClass/Op::NewMixed/Op::CallMethod`, validadores
  (aritmética de pilha + pools), `Display` (`<classe N>` / campos),
  `const_key` sem dedup para os novos.
- VM: `Index` (campo ou método vinculado com `this`), `StoreIndex`
  em instância (método não sobrescrevível), `BuildClass`,
  `new_mixed` (decorador compartilhado com `CallMixed`),
  `call_method` (só `Var` base tem write-back; resto = Index+chamada),
  `pending_new/pending_method` no frame DO CALLEE (morrem no unwind,
  sem stale — mesma classe de bug do fuzz já eliminada por desenho),
  `Return` entrega instância/`this` final, `type()` =
  `"classe"`/`"instancia"`, heap conta nome+campos, `check_type`
  rejeita ambos, `json_stringify` serializa instância como objeto.
- Fmt: reemite classe/`new`/`.campo` (idempotente); linker renomeia
  corpos de métodos (decoracão preservada); `==` estrutural derivado.
- `LANGUAGE_SPEC.md` (§2 gramática + regra de valor/write-back),
  `examples/oo.rhp` (`1 2 2 3 2`, exit 0, em CI), `CHANGELOG.md`.

## Compatibilidade e migração

Aditivo; sem migração. `this` como `Ident` comum (fora de método =
`nao definida`). Temporários (`get().m()`) sem write-back (documentado).

## Segurança e performance

Sem `unsafe`; sem panic novo (fuzz `fuzz_op` cobre os 3 ops novos).
Performance: chamada de método = bind de `this` (1 clone de env) +
write-back (1 assign); sem métodos no caminho, custo zero. Bench sem
classes: sem regressão (suite 310 verdes inclui conformidade).

## Testes e validação

- `classes_methods_and_this` (VM): ciclo, write-back, destacado,
  6 erros (sem `novo`, membro ausente, `new` em não-classe, aridade,
  `this` solto, `this = 5`), named/spread, retorno anotado, `type()`.
- `preserves_classes_and_members` (fmt): forma canônica + idempotência.
- `cargo test --workspace`: 310 passed, 0 failed; `fmt --check` limpo;
  `clippy -D warnings` 0 warnings.
- `examples/oo.rhp`: `1 2 2 3 2`, exit 0.

## Limitações e riscos conhecidos

- Sem herança/`private`/estáticos/`instanceof` (futuro, com RFC).
- `new` só via nome simples (sem `new expr()`); named/spread ok.
- Método destacado não persiste (documentado no exemplo).
- `a.b.c()` aninhado compila via `Index` (sem write-back no nível externo).
- Comentários dentro de `class`/corpos: preservados via próximo statement
  (limitação geral da Etapa 4).

## Referências

- RFC 0017 (`RFCs/0017-classes-minimas.md`, aceita e implementada);
- `MEMORY_MODEL.md` (por valor, sem aliasing — instâncias seguem a regra).
