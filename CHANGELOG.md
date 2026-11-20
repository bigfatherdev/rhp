# CHANGELOG.md — Resumo consolidado por release

Formato inspirado em Keep a Changelog. Detalhes auditaveis ficam em `changes/`.

## [0.6.1] — 2026-11-20 (Auditoria full-system: crash do parser, MySQL, diagnósticos)

### Fixed

- Estouro de pilha nativa (SIGABRT) com cadeias planas longas
  (`1+1+...`, `!!!x`, `a?b:c?d...`, `a??b??...`, `a.b.c`, `a[0][1]`,
  `f()()()`): teto uniforme de 64 níveis + `nest` nas recursões; tudo
  vira E1000 limpo com posição. Registro:
  `changes/2026-11-20-fix-parser-depth-chains.md`.
- MySQL 100% inoperante: `seq` de pacote não zerava por comando (erro
  1156 já no primeiro comando). Uma linha em `command()`; validado ao
  vivo em MariaDB 11 e MySQL 8.0 (`mysql_native_password`).
  Registro: `changes/2026-11-20-fix-mysql-seq-reset.md`.
- 23 mensagens de erro de tipo 2-arg citavam só o 1º tipo
  (`split("a,b", 1)` dizia "encontrado string"). Agora ambos
  (`(string, int)`). Registro:
  `changes/2026-11-20-improved-builtin-arg-types.md`.

### Changed

- Teto sintático 128→64 níveis (medição: ~120 já estoura threads de 2 MiB
  em debug; 64 ≈ 2× margem). Cadeias 65–128 agora E1000 (patológicas;
  código real não chega perto). Spec e docs atualizados.
- `docs/REFERENCIA.md` (`db_query`): `true` gravado volta `1` (`Int`) no
  SQLite/MySQL (sem bool nativo); só PG round-tripa `Bool`.

### Known limitations

- Sem `&&`/`||`, sem pertinência em array, MySQL `caching_sha2_password`
  (exigem RFC — planejado 0.7.0). Detalhes: `audit/full-system/`.

## [0.6.0] — 2026-11-20 (Correcao, robustez do runtime e evolucao do HTTP)

### Fixed

- `return e;` (e `yield e;`) dentro de `try`: o operando agora e avaliado
  ANTES de fechar o handler/`finally` — erros da avaliacao (`return
  json_parse(s);`, `return unknown_fn();`) sao "erro em B" e alcançam o
  `catch` ativo em vez de escapar para o topo. Causa raiz: ordem de
  emissao no compilador (`TryEnd` antes do operando). Inclui `finally`
  rodando depois, com o valor ja calculado (semantica padrao).
  Registros: `changes/2026-11-20-fix-return-try-operando.md`.
- `http_serve` com bind fixo em `127.0.0.1`: nova opt `host` (IP literal
  v4/v6; `"0.0.0.0"` = todas IPv4; default segue local). Erro claro em
  bind falho (porta em uso, interface inexistente). Outros nomes sao rejeitados.
  Registro: `changes/2026-11-20-feat-http-host-bind.md`.
- Requests malformados agora devolvem 400 (`requisicao invalida`) em vez
  de 500: sem status-line, `Content-Length` invalido, corpo incompleto,
  cabecalhos acima de 64 KiB, `Transfer-Encoding` desconhecido.
  `reason()` cobria 400/429 como "Desconhecido" — corrigido (+ 408/413/
  422/502/504).
  Registro: `changes/2026-11-20-fix-http-protocolo.md`.
- Falha de `accept` nao encerra mais o servidor silenciosamente (log +
  nova tentativa com recuo).
  Registro: `changes/2026-11-20-fix-http-protocolo.md`.
- Percent-decode operava em `char` (UTF-8 virava mojibake latin1):
  agora decodifica em bytes (`%C3%A9` => "é").
  Registro: `changes/2026-11-20-fix-http-protocolo.md`.

### Added

- Handler como valor: `http_serve(8080, handler, {})` (resolve `fn` do
  topo; equivale a string). Closures/aninhadas: erro claro em vez de
  comportamento indefinido. Divergencia doc-vs-runtime reconciliada
  (docs atualizadas; sintaxe inline `fn req => ...` continua nao
  suportada — limitacao registrada).
  Registro: `changes/2026-11-20-feat-http-handler-valor.md`.
- Opts `max_passos` (default 10M instrucoes/request; 0 = sem limite) e
  `max_heap_bytes` (default 0 = sem limite): handler em loop eterno vira
  500 e o servidor segue (antes: worker presa para sempre).
- Corpo `Transfer-Encoding: chunked` suportado (RFC 9112 §7.1; precede
  `Content-Length`; malformado = 400). `"localhost"` como `host` vira
  `127.0.0.1`. Registro: `changes/2026-11-20-feat-http-chunked-localhost.md`.
  Registro: `changes/2026-11-20-feat-http-limites-handler.md`.
- Guarda anti-reentrada: `http_serve` dentro do handler vira erro claro
  (500) em vez de hang. `http_serve` indireto via topo replay tambem
  coberto. Registro: `changes/2026-11-20-feat-http-limites-handler.md`.
- Prazo total de leitura por request (cabecalhos + corpo, 30 s):
  gotejamento lento nao prende conexao indefinidamente.
  Registro: `changes/2026-11-20-fix-http-protocolo.md`.
- `docs/WEB.md`: secao `http_serve(porta, handler, opts)` completa +
  aviso de seguranca do `0.0.0.0`; `docs/DOCKER.md`: bind em container +
  topologia validada; `docs/REFERENCIA.md`: linha do `http_serve` e
  semantica de `return`-em-`try`; `LANGUAGE_SPEC.md`/`BYTECODE_SPEC.md`:
  ordem de avaliacao do operando normatizada.

### Improved

- Diagnosticos: erros novos do `http_serve` mapeiam para E3000 generico
  (sem ajuda enganosa de E3001); budget de instrucoes segue E3010.
- Suite HTTP: 9 testes novos de integracao (bind, host invalido, porta
  ocupada, handler-valor, 400s, 64 KiB, UTF-8, loop limitado, aninhado,
  concorrencia 8x) + 1 teste de regressao da VM com os 8 casos do
  `return`-em-`try` + 2 unitarios (`host`, percent-decode).
- Benchmarks: baseline 0.6.0 registrado (harness `bench_rhp`, release);
  caminhos medidos nao foram tocados — variacao observada e ruido de
  CPU compartilhada (ex.: `fib(20)` 96→78 ms entre runs).

### Security

- Default de bind segue `127.0.0.1` (decisao consciente e documentada);
  `0.0.0.0` e opt-in com aviso (firewall, nao expor sem autenticacao).
- Limites por request: corpo 1 MiB, cabecalhos 64 KiB, prazo total 30 s,
  orcamento de instrucoes do handler — entradas maliciosas nao prendem
  worker nem estouram memoria sem teto.
- Auditoria `unwrap`/`expect` em producao: invariantes checadas
  (`http_fetch` redirect com `location` verificada); sem panics novos.
- Prova Docker real: servidor 0.0.0.0 em container alcancado por IP
  interno de outro container + nginx reverso separado + port forwarding;
  controle negativo (bind local = connection refused entre containers).

### Compatibility

- Compativel com 0.5.0: programas validos seguem validos. Unica mudanca
  observavel: (1) `return e;` com `finally` agora avalia `e` antes do
  `finally` (semantica padrao; antes avaliava depois); (2) handlers em
  loop eterno passam a falhar com 500 apos `max_passos` (antes: hang);
  (3) requests malformados passam a 400 (antes: 500); (4) `Accept` com
  erro transiente nao para o servidor. Nenhuma sintaxe removida; nenhum
  codigo de erro existente alterado; `host` default preserva o bind.

### Known limitations

- Handler inline/closure nao suportado (use nome string ou `fn` do topo
  como valor); sem keep-alive; resposta `corpo` so `Str` (sem binario);
  trailers de `chunked` descartados; sem HTTPS no `serve`
  (terminar TLS no proxy); worker recompila o topo por request
  (idempotencia exigida; efeitos nao-idempotentes no topo desaconselhados).

## [0.5.0] — 2026-11-19 (Limitações cuidadas: bounds, metodos async, RFC 0030, trie do router)

### Added

- Bounds de generics (RFC 0029): `<T: Classe|Interface>` com checagem em
  compilacao no uso (`new`/`call` com type args); aridade e nao-generica
  com args validadas; apagamento mantido (zero custo em runtime).
  Registros: `changes/2026-11-19-feat-generics-bounds.md`.
- Metodos `async` com snapshot: `async fn` em classe/trait; `obj.m()`,
  destacada e `super.m()` geram tarefa sobre copia do `this` (sem
  write-back). Instancias cruzam a fronteira com a classe (envelope
  `__rhp_instancia__`); mapas com a chave reservada sao rejeitados.
  Registro: `changes/2026-11-19-feat-async-metodos.md`.
- Combinadores e controle async (RFC 0030, nucleo): `await_timeout(h, s)`,
  `async_cancel(h)`, `async_cancelled()`, `await_all([...])`,
  `await_race([...])` (builtins 75–79). Tarefas comunicam por canal +
  flag atomica; thread destacada. Pool segue adiado (documentado).
  Registro: `changes/2026-11-19-feat-async-combinadores.md`.
- Router do RHP Web com trie ordenada (primeiro match global vence;
  `Allow` identico, provado por teste diferencial): ~7x no pior caso
  medido (100 rotas, build dev). Registro:
  `changes/2026-11-19-perf-web-trie.md`.
- Guia `docs/ASYNC.md` (modelo, fronteira, padroes); referencia com 80
  builtins. Registro: `changes/2026-11-19-docs-async-arquitetura.md`.

### Changed

- `Expr::{Call,New}` encaixotados (`CallData`/`NewData` com `type_args`);
  `sizeof(Expr)` reduzido (guarda de profundidade preservado).
- Worker async executa definicoes de topo (ja em 0.4.0); joins agora via
  canal (mesmas mensagens de erro); retorno instancia decodifica (antes
  degradava em mapa no `join`).
- Conflito de nomes de closure resolvido (`{closure:N}` deterministicos).

## [0.4.0] — 2026-11-19 (Next Generation: heranca, namespaces, async, tipadas, generics, extensoes, web + itens 1–4)

### Added

- Tarefas async com captura e codigo compartilhado (item 1): worker executa
  definicoes de topo (funcoes, classes, `import native`); captura transporta
  valores planos; closures com nomes unicos (`{closure:N}`); `async` aninhado;
  instancia como argumento agora e erro claro (antes degradava em mapa).
  Registros: `changes/2026-11-19-feat-async-captura-estado.md`.
- RHP Web via `rhp pkg` (item 2): `rhp pkg add rhp_web <repo>/framework`
  vendoriza em `pacotes/rhp_web/` com `rhp.lock`; guia em `docs/WEB.md`.
  Registro: `changes/2026-11-19-feat-pkg-framework-web.md`.
- RFCs proposta (item 3): 0028 (`[autoload]` + `use function`), 0029
  (bounds `<T: X>`), 0030 (cancel/timeout/`all`/`race`). Registro:
  `changes/2026-11-19-docs-rfcs-fase3.md`.

- Heranca simples (`extends`, `super` com write-back), interfaces
  (`interface`, `implements` com verificacao de aridade) e traits (`trait`,
  `use`, precedencia propria > trait > pai, conflito sem override = erro).
  RFC 0021. Registro: `changes/2026-11-19-feat-next-generation.md`.
- Namespaces (`namespace`, `A\B`) e autoload (`import A\B [as C]`, PSR-4
  `A/B.rhp` confinado a arvore). RFC 0022.
- `async fn` + `await` sobre tarefas isoladas (args posicionais, erros
  propagados; funcoes autocontidas). RFC 0023.
- Excecoes tipadas (`catch Tipo [var]`, hierarquia por `parent`, ordem
  primeiro-match, `finally`). RFC 0024.
- Generics por apagamento (`class C<T>`, `fn f<T>`, `new C<int>`, `T` sem
  checagem; `<` binario preservado). RFC 0025.
- Extensoes nativas estaticas (`import native "demo"`, ABI 1, `demo_eco`/
  `demo_soma`/`demo_len`). RFC 0026.
- RHP Web (`framework/web.rhp`: rotas, `:params`, middlewares, `web_json`/
  `web_text`/`web_redirect`, 404/405, seguranca, templates; exemplo
  `framework/exemplo_api.rhp`). RFC 0027.
- Testes de integracao `nextgen_*` (10 casos) + gramatica VSCode com novas
  palavras-chave.

### Changed

- Worker `http_serve`: executa todo o topo por request exceto a chamada
  `http_serve` (antes: so `fn`s). Topo deve ser idempotente; permite rotas
  e estado do framework. Programas so com `fn`s + `http_serve` comportam-se
  igual.
- `FuncDef` com `is_async`/`native_id`; `ClassDef` com `parent`; novos ops
  (`BuildClassEx`, `IsInstanceOf`, `Await`, `CallSuper`, `LoadNative`).

### Fixed

- Desambiguacao de `<` binario vs generics (`i < n` nao vira tipo).
- Linker: nomes qualificados globais (sem rename) + alias `as` para autoload.
- Framework `web_casar`: deteccao de `:param` via `starts_with` (indexacao
  de string nao suportada).

## [0.3.0] — 2026-11-18 (Segurança: auditoria 20 CVEs + base Docker `trixie`)

### Changed

- Base Docker `bookworm` → `trixie` (`rust:1.100-trixie`,
  `debian:trixie-slim`; backports de segurança mais rápidos; sem mudança
  de arquitetura, usuário ou pacotes). Registro:
  `changes/2026-11-18-tooling-docker-trixie.md`.

### Security

- Auditoria das 20 CVEs do escopo: 18 NÃO AFETADAS (evidências por
  componente em `SECURITY_REMEDIATION.md`), 2 MITIGADAS (SQLite FTS5 via
  `libsqlite3` do sistema; eliminação total aguarda backport Debian
  ≥ 3.54.0). `Dockerfile` (builder + runtime) passa a aplicar
  `apt-get upgrade` no build. Registro:
  `changes/2026-11-18-security-remediacao-20-cves.md`.

## [0.2.0] — 2026-11-18 (Etapas 3-5 + DevOps: conformidade, heap, fuzz, fmt, DevX, tipos, OO, geradores, async, conectores, Docker)

### Added

- Referência completa da linguagem (`docs/REFERENCIA.md`): sintaxe,
  74 builtins, tipos, erros E0xxx–E3xxx, CLI, exemplos verificados.
  Registro: `changes/2026-11-18-docs-referencia.md`.
- Distribuição oficial via Docker Hub (`bigfatherdev/rhp`, Etapa DevOps):
  `Dockerfile` multi-stage multi-arch, workflow `docker-publish`
  (gates + build amd64/arm64 + push condicional + cache + tags semver
  reais), `docs/DOCKER.md`, ADR 0015. Registro:
  `changes/2026-11-17-tooling-docker-publish.md`.
- `fmt` preserva comentários e linhas em branco (Etapa 4, DevX):
  trivia `//` no lexer, `StmtNode` com decoracao, reemissao idempotente.
  Registro: `changes/2026-11-16-feat-fmt-comentarios.md`.
- Stack traces em erros de runtime (Etapa 4): `VmError.trace` + `  em f`
  na CLI (message intacta p/ `try`). Registro:
  `changes/2026-11-16-feat-stack-traces.md`.
- REPL com estado persistente (Etapa 4): `rhp repl` + `Vm::run_continue`.
  Registro: `changes/2026-11-16-feat-repl.md`.
- Trace de execucao + LSP minimo (Etapa 4): `rhp run --trace`,
  `rhp lsp` (JSON-RPC stdio, diagnostics). Registro:
  `changes/2026-11-16-feat-trace-lsp.md`.
- Conectores TCP/Redis/MySQL (RFC 0020, Etapa 5): `tcp_*`, `redis_*`
  (RESP2), `db_open("mysql:...")` reaproveitando `db_*`. Testes com
  servidores in-process. Registro:
  `changes/2026-11-16-feat-conectores-tcp-redis-mysql.md`.
- Tarefas assíncronas (RFC 0019, Etapa 5): `async_spawn("f")` (thread
  isolada, recompila o programa) + `async_join(h)` (bloqueia, devolve
  o retorno desserializado). Registro:
  `changes/2026-11-16-feat-async-spawn-join.md`.
- Geradores lazy (RFC 0018, Etapa 5): `yield`, `next`/`exhausted`,
  `for x in gen` (duas formas), `type()` = gerador, `==` por identidade.
  Registro: `changes/2026-11-16-feat-geradores.md`.
- Classes mínimas (RFC 0017, Etapa 5): `class`, `new` (construtor `novo`),
  `obj.metodo()` com `this` e write-back, `type()` classe/instância.
  Por valor, sem herança. Registro:
  `changes/2026-11-16-feat-classes-minimas.md`.
- Anotações de tipo com checagem em runtime (RFC 0016, Etapa 5):
  `let x: int`, `fn f(a: int): int`, 8 tipos, erro `tipos incompativeis`
  (E3001). Opt-in, sem coercao. Registro:
  `changes/2026-11-16-feat-tipos-anotacoes.md`.
- Teto de heap opt-in (RFC 0015, Etapa 3): `VmLimits.max_heap_bytes` +
  `rhp run --max-heap-bytes N`, estimador do modelo com enforcement O(1)
  amortizado, erro `heap excedeu o limite` (E3012). Padrão ilimitado.
  Registro: `changes/2026-11-16-feat-heap-limit.md`.
- Fuzz determinístico std-only em CI (Etapa 3): lexer (5000 entradas),
  parser (5000 sequências), VM/validador (8000 chunks hostis + 600
  mutações de programas válidos). Sem novas dependências. Registro:
  `changes/2026-11-16-fix-fuzz-vm-panic.md`.
- Suite de conformidade `tests/conformance/` (Etapa 3, Tooling): 18
  programas válidos com stdout pinada + 6 inválidos com stderr pinada,
  via `rhp run` (`crates/rhp-cli/tests/conformance.rs`); property tests
  std-only ampliados (`-`, `<`/`==`, `len()` de concatenação; 1800
  programas gerados). Registro:
  `changes/2026-11-16-test-conformidade-etapa3.md`.
- Closures de primeira classe (RFC 0003, PHP-100 P1): `fn (params) { }`
  como expressão com captura por valor, `MakeFunc`, chamadas de callee
  arbitrário (`CallValue`), `import` em closure rejeitado no loader.
  Registro: `changes/2026-11-16-feat-closures-primeira-classe.md`.
- `switch` com fallthrough (RFC 0004, Fase 2): `case`/`default` (único,
  posição livre), `break`/`continue`, escopo por ramo, comparação `==`.
  `switch`/`case`/`default` viram keywords.
  Registro: `changes/2026-11-16-feat-switch.md`.
- HOFs com callback (RFC 0005, Fase 2): `map`/`filter`/`usort` via
  protocolo de chamada pendente na VM (`usort` estável).
  Registro: `changes/2026-11-16-feat-hofs-callback.md`.
- Parâmetros default e variádicos (RFC 0006, Fase 2): `b = expr-const`
  dobrado em compilação, `...rest` coletando excedentes.
  Registro: `changes/2026-11-16-feat-args-default-variadicos.md`.
- Destructuring raso (RFC 0007, Fase 2): `let [a, b]` / `let {x}` /
  `let {k: v}` (+ `const`), erros herdados, sem `null` silencioso.
  Registro: `changes/2026-11-16-feat-destructuring.md`.
- Lote nucleo-2 (RFC 0008, Fase 3): `match` estrito, Elvis `?:`, arrow
  `=>`, `for(;;)`, `break`/`continue N` (`match` vira keyword).
  Registro: `changes/2026-11-16-feat-lote-nucleo-2.md`.
- Chamadas mistas (RFC 0009, Fase 4): nomeados + spread via `CallMixed`.
  Registro: `changes/2026-11-16-feat-chamadas-mistas.md`.
- `static` em funcao (RFC 0010, Fase 4): estado por definicao.
  Registro: `changes/2026-11-16-feat-static-funcao.md`.
- Re-export seletivo (RFC 0011, Fase 4): transitivo, sem `export *`.
  Registro: `changes/2026-11-16-feat-reexport.md`.
- Builtins `rand`/`rand_range`/`fs_list` (RFC 0012, Fase 4).
  Registro: `changes/2026-11-16-feat-rand-fslist.md`.
- Hashing de senhas (RFC 0013, Fase 5): `hash_password` (Argon2id) e
  `verify_password` (`$argon2id$` + bcrypt `$2*$`, migração de bancos PHP).
  Registro: `changes/2026-11-16-feat-password-hashing.md`.
- Regex, hashing e HMAC (RFC 0014, Fase 6): `regex_match`/`regex_replace`/
  `regex_split` (sem lookaround/backrefs), `sha256` + `hmac_sha256`/`hmac_verify`
  (RustCrypto puro, teto anti-DoS).
  Registro: `changes/2026-11-16-feat-regex-hmac.md`.

### Removed

- `RHP_GOVERNANCE.md` (duplicata byte-idêntica de `GOVERNANCE.md`).
  Registro: `changes/2026-11-16-docs-remover-governanca-duplicada.md`.

### Fixed

- `async_spawn` sem programa-fonte no REPL, no `rhp test` e nos handlers
  `http_serve` (so `rhp run` definia; worker precisa recompilar). Achado
  na verificação pós-merge (auditoria de todos os caminhos + servidor
  real). Registro: `changes/2026-11-17-fix-async-program-context.md`.
- `==` sempre falso para classes, instancias e geradores (braços
  ausentes em `values_equal`; achado ao escrever exemplos). Registro:
  `changes/2026-11-18-fix-eq-novos-tipos.md`.
- Panic com `max_stack_depth` + `return` (Etapa 3, achado do fuzz):
  `Return` remove o frame e a checagem final de profundidade usava
  índice obsoleto; agora tolera frame removido (topo corrente medido
  no passo seguinte). Regressão pinada. Registro:
  `changes/2026-11-16-fix-fuzz-vm-panic.md`.

## [0.1.0] — 2026-11-15 (primeira release: linguagem + runtime web + persistencia)

### Added

- Primeira fatia vertical do RHP: `hello.rhp`, lexer, parser/AST, geracao de
  bytecode, VM em pilha e CLI `rhp run hello.rhp` (saida `Hello from RHP!`).
  Registro: `changes/2026-11-14-feat-fase-1-fatia-vertical.md`.
- Marco 2 (nucleo imperativo, compativel): expressoes (int/float/bool/null,
  aritmetica checada, comparacoes), `let`/atribuicao, blocos/escopos,
  `if`/`else`, `while`, `fn`/`return`/chamadas com recursao limitada (512),
  `echo` de qualquer valor, comentarios `//` e exemplo `examples/fib.rhp`.
  Registro: `changes/2026-11-14-feat-marco-2-nucleo-imperativo.md`.
- Fase 3 (stdlib nucleo): arrays/mapas por valor, indexacao e `a[i] = v`,
  builtins `len push pop keys str int float type assert` (ADR 0002).
  Registro: `changes/2026-11-14-feat-fase-3-stdlib-nucleo.md`.
- Fase 4 (modulos): `import "x.rhp"` com DFS/dedup/ciclo como erro (ADR 0003).
  Registro: `changes/2026-11-14-feat-fase-4-modulos.md`.
- Fase 5 (ferramentas): crate `rhp-fmt` + `rhp fmt [--check]` idempotente e
  `tools/check-changes.sh` de CI (ADR 0003).
  Registro: `changes/2026-11-14-feat-fase-5-ferramentas.md`.
- Escape `\u{HEX}` em strings.
  Registro: `changes/2026-11-14-feat-escape-unicode.md`.
- Limites de execução opt-in (`VmLimits`, `rhp run --max-steps N`).
  Registro: `changes/2026-11-14-feat-limites-execucao.md`.
- Robustez de testes: corpus malformado, property test determinístico,
  verificação de exemplos, `check-changes.sh` com renames.
  Registro: `changes/2026-11-14-test-robustez.md`.
- Benchmarks std-only + baseline documentado (sem otimizações).
  Registro: `changes/2026-11-14-bench-baseline.md`.
- Documentação: README reestruturado, `docs/`, `CONTRIBUTING.md`, `SECURITY.md`.
  Registro: `changes/2026-11-14-docs-checkpoint2.md`.
- Stdlib lote 2: 19 builtins (strings, matemática, mapas; ids 10–29).
  Registro: `changes/2026-11-15-feat-stdlib-lote2.md`.
- I/O e ambiente: `fs_read/fs_write/fs_append/fs_exists`, `argv()`, `env()`;
  `rhp run` repassa args extras.
  Registro: `changes/2026-11-15-feat-io-env.md`.
- `rhp test` (descoberta `*_test.rhp`) e `rhp doctor` (versão + autoteste).
  Registro: `changes/2026-11-15-feat-cli-test-doctor.md`.
- Sintaxe lote 2: bitwise, ternário, `??`, `break`/`continue`, `const`.
  Registro: `changes/2026-11-15-feat-sintaxe-lote2.md`.
- Laco `for x in ...` (array = elementos; mapa = chaves ordenadas),
  builtins `sort(array)` (copia ordenada estavel) e `exit(codigo)`
  (0-255; vira exit code no `rhp run`).
  Registro: `changes/2026-11-15-feat-laco-for-sort-exit.md`.
- Builtins `json_parse(texto)` / `json_stringify(valor)` (parser e
  serializador proprios, sem dependencias; teto de 128 niveis; erros
  com posicao).
  Registro: `changes/2026-11-15-feat-json.md`.
- Exceções: `try`/`catch`/`finally` + `throw` (RFC 0001), com propagação
  entre funções, `finally` em todos os caminhos (`exit()` não aciona) e
  erro como valor (`{tipo, mensagem}`).
  Registros: `RFCs/0001-excecoes.md`, `changes/2026-11-15-feat-excecoes.md`.
- Módulos v2 (ADR 0009): `export fn/let/const` opt-in e
  `import { a, b as c } from` seletivo (privacidade real, alias, colisão
  só entre visíveis; bare preserva o legado).
  Registro: `changes/2026-11-15-feat-modulos-v2.md`.
- Datas/horas std-only: `now()` (epoch), `sleep(s)` (teto 1h) e
  `date_str(ts)` (ISO UTC).
  Registro: `changes/2026-11-15-feat-datas-horas.md`.
- Rede v2: `http_fetch(url, opts)` (redirects, headers, timeout;
  `http_get` preservado). Fase A concluída.
  Registro: `changes/2026-11-15-feat-rede-v2.md`.
- SQLite via FFI mínimo ao `libsqlite3` (sem crate externa):
  `db_open/exec/query/close` (ADR 0011).
  Registro: `changes/2026-11-15-feat-sqlite.md`.
- PostgreSQL via protocolo próprio (mesmos `db_*`, prefixo `pg:`),
  testado contra servidor real. Fase B concluída.
  Registro: `changes/2026-11-15-feat-postgres.md`.
- Servidor HTTP: `http_serve(porta, handler, opts)` (RFC 0002; thread
  por conexão, 500 sem vazar, `exit` termina o servidor).
  Registro: `changes/2026-11-15-feat-http-serve.md`.
- Sessões e templates: cookies em `req`, `template()` com escape HTML,
  padrão sessão-via-banco documentado.
  Registro: `changes/2026-11-15-feat-sessao-template.md`.
- HTTPS: `http_fetch` com TLS verificado (rustls + raízes embutidas +
  `RHP_CA_BUNDLE`; ADR 0012). Fase D iniciada.
  Registro: `changes/2026-11-15-feat-https.md`.
- Pacotes v0: `rhp init`, `rhp pkg add/install [--check]`, `pacotes/` +
  `rhp.lock` com hash por arquivo.
  Registro: `changes/2026-11-15-feat-pacotes.md`.
- `rhp build` (bundle autocontido para deploy) + `limite_taxa` no
  servidor (429 com `Retry-After`).
  Registro: `changes/2026-11-15-feat-build-ratelimit.md`.
- Fase 6 (rede minima): builtin `http_get` std-only (so `http://`, 5s, 2xx)
  verificado contra servidor local; fronteira honesta para PostgreSQL/Redis,
  async, classes/generics (ADR 0004, planejados e NAO implementados).
  Registro: `changes/2026-11-14-feat-fase-6-rede-minima.md`.
- Diagnosticos estruturados: crate `rhp-diagnostic` (codigos E0xxx–E3xxx,
  `render` com snippet e `help`) + CLI com `Error[CODIGO]` (ADR 0007).
  Registro: `changes/2026-11-14-feat-diagnosticos.md`.
- Verificador de fluxo de pilha no bytecode + `VmLimits` estendido
  (`max_stack_depth`, `max_collection_len`) + runtime separado em
  `rhp-vm/src/builtins.rs` (ADR 0008).
  Registro: `changes/2026-11-14-feat-verificador-pilha.md`.
- Dedup de constantes e interning O(1) de nomes (sem mudanca semantica;
  tradeoff documentado) + `reports/MEMORY_MODEL.md`.
  Registro: `changes/2026-11-14-feat-memoria-cp3.md`.
- `rhp check [--json]` (lex+imports+compile sem executar), gramatica VSCode
  versionada com teste de consistencia, desenho de debugger
  (`docs/DEBUGGER_DESIGN.md`) — sem debugger improvisado (ADR 0010).
  Registro: `changes/2026-11-14-feat-devtools-check.md`.
- Modulos v2: apenas semantica proposta em ADR (sem codigo).
  Registro: `changes/2026-11-14-docs-modulos-v2.md`.
- Auditoria de maturidade Next Generation.
  Registro: `changes/2026-11-14-docs-nextgen-audit.md`.

### Fixed

- Teto de profundidade sintatica (128 niveis) e de constantes: entradas
  profundamente aninhadas geram erro em vez de abortar o processo.
  Registro: `changes/2026-11-14-fix-limite-profundidade.md`.
- `rhp fmt` com roundtrip garantido: floats sem notacao cientifica e chaves
  de mapa citadas quando necessario.
  Registro: `changes/2026-11-14-fix-fmt-roundtrip.md`.
- `http_get` com teto de corpo de 1 MiB e DNS documentado.
  Registro: `changes/2026-11-14-fix-http-limites.md`.
- Parametros duplicados rejeitados; `int()` estrito no limite de `i64`;
  `+x` como identidade; `import` confinado a arvore, teto de 8 MiB e
  apenas no nivel superior; `VmLimits`/`--max-steps` opt-in.
  Registros: `changes/2026-11-14-fix-p2-nucleo.md`,
  `changes/2026-11-14-feat-limites-execucao.md`.
- Escopo de `while`/`for` nao vaza em `break`/`continue` (`let` do corpo
  invisivel fora do laco).
  Registro: `changes/2026-11-15-fix-laco-escopo-break.md`.
