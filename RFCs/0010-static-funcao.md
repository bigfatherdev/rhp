# RFC 0010 — `static` em função (estado por definição)

- ID: RFC-0010
- Data: 2026-11-16
- Estado: aceita (programa RHP-100; implementada na Fase 4)
- Contexto: contadores, memoização e singletons locais sem globais.

## 1. Problema

Estado entre chamadas hoje exige global (visível em todo o módulo) ou
banco. Falta o `static` do PHP: variável local persistente por definição.

## 2. Objetivo e não-objetivos

Objetivo: `static n = expr;` dentro de função: avaliado a cada chamada,
mas atribuído só na primeira; lido/escrito normalmente depois; por
definição (closures da mesma definição compartilham).
Não-objetivos: `static` no topo (erro de compilação, como `return` fora);
múltiplos nomes por statement; `static` sem init; `global` (desnecessário:
RHP já enxerga globais).

## 3. Decisão de desenho

- Sintaxe: `Stmt::Static { name, init: Expr }` (init obrigatório).
  `static` vira keyword (renomear identificadores; erro claro).
- Runtime: `Vm.statics: Vec<(Rc<FuncDef>, HashMap<String, Value>)>`
  (identidade por `Rc::ptr_eq`; poucas definicoes usam `static`);
  `Frame.func: Option<Rc<FuncDef>>` (`None` no principal).
- **Um unico op novo: `Op::DeclareStatic`** (pop + insere se ausente).
  Leituras/escritas reutilizam `LoadName`/`StoreName` com fallback em
  `lookup`/`assign` (escopos → statics → globais). `LoadStatic`/`StoreStatic`
  dedicados foram descartados na implementacao: seriam codigo morto
  (nunca emitidos) — o fallback cobre tudo, com shadowing natural.
- Sem tracking de compilador: `LoadName`/`StoreName`/`assign`/`lookup`
  ganham fallback para statics (escopos → statics → globais). Shadowing
  natural (escopo interno vence; `let` após `static` no mesmo escopo
  sombreia — documentado, evite).
- `static` em closure: por definição (instâncias compartilham — como PHP).
- Init com efeito colateral roda por chamada, mas só a primeira atribui
  (divergência documentada; PHP avalia uma vez).

## 4. Alternativas consideradas

- Tracking em compilação (reescrever Var/Assign p/ estática): frágil com
  shadowing e blocos; rejeitado (fallback em runtime é exato e menor).
- Env da closure: rejeitado (env é snapshot por valor; estático é
  compartilhado por definição).
- `static` sem init (= null): rejeitado (explícito é melhor; init barato).

## 5. Compatibilidade

Aditivo + keyword nova (`static` como nome quebra com erro claro).
Nenhum programa válido muda.

## 6. Segurança

Sem `unsafe`; mapa chaveado por `Rc` (sem raw ptr); sem `Send` novo;
validador cobre os 3 ops (mesmos efeitos de pilha dos `*Name`).

## 7. Performance

Um `HashMap` por definição com static; lookup extra só após miss em
escopos (caminho quente inalterado quando há local).

## 8. Plano de implementação

1. Lexer: `Static` + keyword + teste.
2. Parser: `Stmt::Static`, `parse_static` (init obrigatorio), dispatch,
   docs, testes.
3. Bytecode: 3 ops + validador + braco `compile` (`static` fora de funcao
   = erro via `in_function`), testes.
4. VM: `Frame.func`, `statics`, fallback em `lookup`/`assign`, 3 bracos,
   testes (contador, instancia compartilhada, init-uma-vez, closure,
   sem-init-acesso, fora-de-funcao).
5. Fmt (`static n = e;`) + CLI (renamer/checkers) + specs + testes.

## 9. Plano de testes

- Contador persiste; recursao compartilha; duas instancias de closure idem.
- Init com contador: roda por chamada, valor mantido da primeira.
- `static` fora de funcao = erro de compilacao.
- Leitura sem declaracao executada (ex.: `if false`) = erro de runtime.
- Shadowing documentado: `let` apos `static` no mesmo escopo sombreia.
- Suite sem regressao.

## 10. Migracao e rollout

Renomear identificadores `static`. Demais: nenhuma.

## 11. Criterios de aceitacao

- [ ] Testes da §9 passam; suite sem regressao.
- [ ] `fmt`/`clippy`/`test` + `check-changes.sh` verdes.
- [ ] Specs + divergencias em `PHP_DIFFERENCES.md`.
- [ ] Esta RFC marcada `aceita` antes do merge.
