# PHP_PARITY_AUDIT.md — Auditoria integral + matriz de paridade (FASE 0)

- Data: 2026-11-14 · Ramo: `feat/php-parity-audit` (sem push ate decisao final)
- Referencia funcional verificada hoje: **PHP 8.5.12 estavel** (GA 2025-11-20;
  suporte ativo ate 31/12/2027; fontes: php.net/downloads, php.watch,
  php.net/archive/2026). PHP 8.6 em RC (GA previsto 19/11/2026) — fora do
  escopo da referencia.
- Metodo: leitura integral do workspace (7 crates, ~5,3k linhas com testes),
  specs normativas, ADRs 0001–0010, `changes/`, relatorios anteriores;
  sondas de execucao em `/tmp` (repo intacto); **nenhuma alteracao de codigo
  nesta fase**.

Legenda de estado: IMPLEMENTADO · PARCIAL · AUSENTE · PLANEJADO (registrado,
nao implementado) · NAO_APLICAVEL.

## 1. Estado real do RHP (verificado, nao presumido)

| Item | Estado real |
|---|---|
| Toolchain | rustc/cargo 1.100.0, edicao 2021 |
| Gates | `fmt --check` ok, `clippy -D warnings` ok, `cargo test` 133 passed, 0 failed |
| Deps externas / `unsafe` | zero / nenhum (`cargo tree` = so workspace + std) |
| Lexer | Manual; keywords: `echo let if else while fn return import true false null`; sem `& | ^ << >>`, sem `$`, sem `?->`, sem interpolacao |
| Parser/AST | Precedencia completa p/ operadores existentes; profundidade maxima 128; `+x` identidade; sem `match`/`switch`/`break`/`continue`, sem ternario/`??`, sem closures, sem namespaces |
| Tipos | Dinamicos apenas: `Null Bool Int(i64) Float(f64) Str Array Map Func`; sem estaticos, sem uniao/nullable |
| Funcoes | Nomeadas, params posicionais, `return`, recursao (limite 512 frames), sem padroes, sem variadicos, sem closures/captura, sem primeira-classe (`f` como valor nao existe — `len` e builtin, nao valor) |
| OO | **Ausente por completo**: sem classes/objetos/heranca/interfaces/traits/enums/atributos |
| Erros | `VmError`/`ParseError`/etc. como valores; **sem excecoes** (`try/catch/finally` inexistentes); diagnosticos E0xxx–E3xxx com snippet+help |
| Colecoes | `Array`/`Map` por valor (copia O(n)); `len push pop keys`; sem `sort/filter/map/reduce`, sem conjuntos/filas |
| Strings | Literais + `+` (concat) + escapes (`\u{...}`); sem busca/fatia/divisao/juncao/trim/case/format |
| Builtins (10) | `len push pop keys str int float type assert http_get` |
| Numeros | `+ - * / %`, checados; sem bitwise, sem `sqrt/pow/abs/min/max`, sem random, sem decimal/bigint |
| Modulos | `import` DFS + dedup + ciclo/ausente/jaula/teto 8 MiB + topo-nivel; sem exports/privacidade/alias/namespaces/versoes/pacotes |
| Stdlib arquivos/datas/JSON/regex/cripto/processos | **Tudo ausente** |
| Rede | `http_get` std-only (`http://`, 5 s, so 2xx, corpo <= 1 MiB, sem redirect/HTTPS/chunked dedicado); sem server/sockets/TCP/UDP/DNS API |
| Banco de dados | **Ausente** (nenhum driver, protocolo ou teste contra servidor) |
| Concorrencia | **Ausente** (single-thread; sem async/fibers/threads no nivel da linguagem) |
| CLI | `run [--max-steps]`, `fmt [--check]`, `check [--json]`, `--help/--version`; sem `test/build/inspect/doctor/bench/repl/debug/package` |
| Formatter | Canonico idempotente; **descarta comentarios e linhas em branco** (limitacao conhecida) |
| Diagnosticos | `rhp-diagnostic` (dado separado de apresentacao); spans aproximados; LSP apenas projetado |
| Debugger/REPL/LSP/extensao | Gramatica VS Code versionada; debugger so desenhado; resto ausente |
| Verificador | Estrutural + fluxo de pilha + tetos (profundidade 128, builtins, saltos); sem analise de tipos |
| Memoria | Semantica por valor, sem coleta; dedup de constantes + interning; sem teto geral de heap; `VmLimits` opt-in (passos/pilha/colecao) |
| Docs/governanca | Specs normativas, 10 ADRs, 27 `changes/`, CHANGELOG/ROADMAP consistentes |

## 2. Matriz de paridade funcional (RHP x PHP 8.5)

### 2.1 Nucleo da linguagem

| Recurso (PHP) | RHP | Notas |
|---|---|---|
| Variaveis dinamicas | IMPLEMENTADO (`let`, atribuicao) | |
| Constantes (`const`/`define`) | AUSENTE | Proposta: imediato (P) |
| Tipagem dinamica | IMPLEMENTADO | Sem coercao silenciosa (decisao propria) |
| Tipagem estatica opcional/inferencia | AUSENTE | Exige RFC; ver §7 |
| Tipos primitivos + null | IMPLEMENTADO (7 tipos) | Sem uniao/intersecao/literal |
| Tipos de retorno/parametros | AUSENTE | Sem anotacoes; depende de RFC de tipos |
| Argumentos nomeados/opcionais/variadicos/default | AUSENTE | Proposta: imediato-medio (P/M; sem sintaxe nova alem de `=` e `...`) |
| Desempacotamento de argumentos | AUSENTE | Depende de variadicos |
| Aritmeticos/comparacao/logicos | IMPLEMENTADO | Sem bitwise |
| Bitwise (`& \| ^ ~ << >>`) | AUSENTE | Proposta: imediato (P; lexer+parser+VM) |
| `??`, `?->`, Elvis/ternario | AUSENTE | Proposta: imediato (P; `??`/`?:` sem novo runtime) |
| `match`/`switch` | AUSENTE | Proposta: medio (M; desugar para `if`/saltos ou op proprio) |
| Loops (`for`, `foreach`) | PARCIAL (`while` apenas) | Proposta: `for` como aÇucar/desugar (P) |
| `break`/`continue` | AUSENTE | Proposta: imediato-medio (P/M; salto com contagem de escopos) |
| Destructuring | AUSENTE | Proposta: medio (M; apos padroes de atribuicao) |
| Closures/arrow functions/anonimas | AUSENTE | Exige captura (arquitetura; ver §7) |
| Recursao (+indireta) | IMPLEMENTADO | Limite 512 frames, testado |
| Escopo lexico/global | IMPLEMENTADO (locais+globais) | Sem captura de funcoes internas |
| Referencias (`&`) | NAO_APLICAVEL | Semantica por valor e o contrato (ADR 0002) |

### 2.2 Orientacao a objetos

| Recurso (PHP) | RHP | Notas |
|---|---|---|
| Classes/objetos/construtores | AUSENTE | Exige modelo de objeto + despacho + memoria; ver §7 |
| Propriedades/metodos/visibilidade | AUSENTE | Idem |
| Heranca/abstratas/interfaces/traits | AUSENTE | Idem |
| Estaticos/constantes/finais | AUSENTE | Idem |
| Metodos magicos/sobrescrita/LSP-substituicao | AUSENTE | Idem |
| Reflection/attributes/enums/readonly/hooks/promocao | AUSENTE | Idem, em camadas |
| Autoloading | AUSENTE | Depende de pacotes (ADR 0009) |

Veredito OO: **nao iniciar sem ADR de modelo de objetos + RFC de tipos**.
Risco de "atalhos sintaticos" e alto; custo estimado: G (grande).

### 2.3 Funcoes

| Recurso | RHP | Notas |
|---|---|---|
| Nomeadas + recursao | IMPLEMENTADO | |
| Primeira-classe/alta-ordem/callbacks | AUSENTE | Exige valores `Func` chamaveis por variavel (hoje so `Call` por nome) — medio (M) com desenho |
| Variadicas/nomeados/default | AUSENTE | Ver 2.1 |
| Invocacao dinamica (`$f()`) | AUSENTE | Depende de primeira-classe |
| Geradoras (`yield`) | AUSENTE | Exige suspensao/resume (arquitetura; ver §7) |

### 2.4 Excecoes e erros

| Recurso | RHP | Notas |
|---|---|---|
| Erros como valores + exit codes | IMPLEMENTADO | |
| `try/catch/finally`, hierarquia, `throw` | AUSENTE | Exige unwinding de frames+escopos (arquitetura; ver §7) |
| Stack traces/file:line:col | PARCIAL (diagnosticos com posicao; sem trace de runtime) | Trace exige mapa op→fonte (depende de debugger-design) |
| Tipos de erro (I/O, rede, lib) | PARCIAL (mensagens; sem taxonomia) | Taxonomia E3xxx existe; hierarquia, nao |

## 3. Geradores, iteradores e colecoes (§4 da missao)

Tudo AUSENTE como recurso dedicado. Base existente: arrays/mapas por valor +
`while` + `len`/`keys`. Proposta faseada (sem quebrar semantica de copia):
(1) `for x in iteravel` (acucar sobre indice/len — P/M, sem novo runtime);
(2) builtins funcionais `map/filter` que devolvem novos arrays (compativel
com copia; custo O(n) documentado); (3) `sort/search` (M);
(4) geradores/`yield` (G, arquitetura de suspensao — so apos decisao).
Sets/filas/imutaveis: NAO_APLICAVEL como tipos nativos nesta fase (usar
arrays/mapas). Benchmarks antes de qualquer estrategia de memoria nova.

## 4. Modelo de memoria (§5 da missao)

Estado: verificado em `reports/MEMORY_MODEL.md` (valor por copia, sem
coleta; dedup de constantes + interning; sem teto geral de heap).
Decisao vigente: **manter**, com comparacao CoW/RC/GC/hibrido registrada e
descartada por falta de problema correspondente. Para paridade PHP (que usa
refcount+GC de ciclos + CoW em arrays): RHP nao precisa de GC hoje (sem
ciclos possiveis, sem referencias). Gatilhos que reabrem a decisao:
objetos com identidade/compartilhamento, closures com captura, ou
benchmark que prove retencao patologica. Proposta:Instrumentar medicao
(RSS/pico por cenario) antes de qualquer mudanca; sem `unsafe` em qualquer
alternativa. Complexidade: M para instrumentacao; G para CoW/RC (exige ADR
+ testes de equivalencia observavel).

## 5. Verificador avancado de bytecode (estado + delta)

Existe: pools, saltos, `Halt`, profundidade de constantes, **fluxo de pilha**
(underflow/juncao). Falta (proposto, sem pressa): tipos estruturais de
operandos por instrucao (ex.: `Add` numerico) e tabela de efeitos
documentada em `BYTECODE_SPEC.md`. Sem serializacao de bytecode (nao criar
complexidade de compatibilidade sem uso — cf. governanca §10); se um dia
persistido, exigir versao + politica (PLANEJADO, bloqueado ate la).

## 6. VM e runtime (estado + delta)

Existe: despacho por `match`, frames explicitos, `VmLimits` opt-in,
builtins isolados. Propostos condicionais (so com benchmark provando):
internacao de nomes em runtime (ja feita em compilacao), cache de
`LoadName` (slot), reducao de clones no caminho quente. **Sem JIT**
(explicitamente futuro e condicional). Risco de otimizacao prematura: alto;
regra mantida.

## 7. Modulos de segunda geracao (§6 da missao)

Semantica definida e aprovada em `ADRs/0009` (exports opt-in, privacidade,
alias, identidade canonica, sem mudanca no `import` atual). Falta (apos
aprovacao desta auditoria): RFC com sintaxe final, `BYTECODE_SPEC`
(tabelas), `VM_SPEC` (resolucao), suite de compatibilidade, `rhp test`
basico como primeiro consumidor. Dependencias: diagnósticos (ok),
`rhp check` (ok). Complexidade: M (sem gerenciador ainda).

## 8. Developer experience (§11 da missao)

- VS Code: gramatica versionada + teste de consistencia (IMPLEMENTADO);
  snippets/LSP/debugger: projetados, nao implementados.
- `rhp check [--json]` IMPLEMENTADO (ponte LSP/CI).
- `rhp test/build/inspect/doctor/bench/repl/debug/package`: NAO implementar
  sem funcionalidade real (regra vigente). Proposta: `rhp test` primeiro
  (convenção de descoberta + runner mínimo, M), `rhp doctor` (env/versões/gates, P),
  demais apos dependencias (LSP, debugger, pacotes).
- Debugger: `docs/DEBUGGER_DESIGN.md` vigente (mapa op→span opt-in,
  `Vm::step()`, DAP futuro); sem codigo ate ADR propria.

## 9. CLI e ferramentas (estado)

Existe: `run [--max-steps]`, `fmt [--check]`, `check [--json]`, help/version.
`rhp test` e o proximo candidato real (depende so de convencao + runner;
sem dependencias novas). Demais comandos: PLANEJADOS, bloqueados ate terem
funcionalidade (regra anti-stub mantida).

## 10. Seguranca e execucao controlada (§14)

Existe e testado: validacao (inclui fluxo), sem `unsafe`, aritmetica
checada, tetos (profundidade 128, chamadas 512, HTTP 1 MiB, fonte 8 MiB,
`VmLimits`), jaula de imports, DNS documentado. Falta (proporcional):
limite de taxa HTTP (com servidor real), timeout de parede opt-in
(threads — avaliar), teto geral de heap (instrumentacao), fuzz continuo.
Nunca alegar sandbox: sem isolamento de SO (documentado).

## 11. Biblioteca padrao (§7 da missao) — matriz

Legenda adicional: IMEDIATO = sem mudanca arquitetural (sintaxe/VM ja
suportam; so builtins+testes+docs).

| Area PHP | RHP hoje | Proposta |
|---|---|---|
| Strings (busca, split/join, trim, case, replace, format) | Literais+concat apenas | IMEDIATO: builtins `contains/split/join/trim/upper/lower/replace` (+ testes) |
| Unicode/UTF-8 | `\u{...}`, `len` em escalares | IMEDIATO: `chars()`/iteracao basica; normalizacao = PLANEJADO |
| Arrays (push/pop/sort/filter/map) | `len/push/pop`, index, `keys` p/ mapas | IMEDIATO: `sort/filter/map` funcionais (copia; O(n) declarado) |
| Mapas | CRUD basico + `keys` | IMEDIATO: `values/entries/merge/has` |
| Matematica (`abs/min/max/sqrt/pow`, random) | `+ - * / %` | IMEDIATO: builtins matematicos (+ `rand` nao-cripto via xorshift seed fixa? decidir: sem cripto) |
| Bitwise | AUSENTE | IMEDIATO (P): lexer+parser+VM+testes |
| Datas/horas/timezone | AUSENTE | Medio (M): relogio do SO via std (`SystemTime`), sem cronologia completa inicialmente |
| Arquivos/diretorios | AUSENTE | Medio (M): `fs_read/fs_write/fs_append/fs_exists` com jaula + teto de tamanho; erro dedicado |
| JSON | AUSENTE | Medio (M): parser/serializador proprios, sem deps, com teto de profundidade/tamanho e erros E3xxx |
| Regex | AUSENTE | Grande/bloqueado sem decisao: motor proprio = risco ReDoS; crate externa = quebra regra zero-dep (exige ADR+aprovacao). Recomendo adiar |
| Rede cliente | `http_get` minimo | Medio (M): redirects/headers/timeouts por requisicao; HTTPS = PLANEJADO (exige TLS = ADR) |
| Servidor HTTP/roteador | AUSENTE | Grande (G): event loop/threads; so apos stdlib+testes maduros |
| Sockets/TCP/UDP/DNS API | AUSENTE | Grande (G); DNS API apos resolver timeout de resolucao |
| PostgreSQL/SQLite/MySQL | AUSENTE | Grande+bloqueado ate servidores de teste + RFC de API comum; SQLite exigiria motor ou binding (binding = deps/FFI: ADR) |
| Cripto (hash/HMAC/senha/rand seguro) | AUSENTE | **Nao implementar algoritmo proprio**; exige crates auditadas = conflito com zero-dep => ADR para aprovacao. Sem isso: NAO fazer |
| Processos/env/argv/exit | AUSENTE | Medio (M): `argv/env/exit` triviais; `exec` = risco command injection => desenho restrito + ADR de seguranca |

## 12. Web, concorrencia, reflection, interop (§§8–9, 12)

- Web (router/request/response/middleware/sessoes/templates/JSON APIs):
  PLANEJADO, apos stdlib+server. Nao criar framework antes das primitivas.
- Concorrencia/async (tasks, channels, event loop, Fibers como referencia):
  PLANEJADO; exige decisao de runtime (threads vs executor) + regras de
  compartilhamento (hoje: single-thread, sem `Send` nos valores).
  Recomendacao: RFC dedicada; nada de `async` sintatico sem runtime.
- Reflection/atributos/metaprogramacao: PLANEJADO; depende de objetos
  (quando existirem) + limites anti-abuso.
- Interop (HTTP/JSON/protocolos): PARCIAL via `http_get`+JSON futuro;
  FFI/C ABI/WASM: NAO_APLICAVEL por ora (risco >> beneficio sem caso de uso).

## 13. Desempenho e otimizacao (§13)

Baseline em `docs/BENCHMARKS.md` (harness std-only). Regra vigente: sem
otimizacao sem prova antes/depois. Candidatos reais (quando aprovados):
dedup ja feito; interning de nomes em runtime; cache de `LoadName`;
clones no caminho quente. **Sem JIT** (condicional e futuro).

## 14. Testes e conformidade (§15)

Existe: 133 testes (unit/integracao/parser/bytecode/VM/CLI/fmt/diagnosticos/
rede local/corpus/property/exemplos/limites). Falta: suite de conformidade
normativa por secao da spec (mapear regra→teste), testes de estresse,
fuzz continuo, matriz IMPLEMENTED/PARTIAL/PLANNED/BLOCKED/NOT_APPLICABLE —
proposta abaixo (§16) como artefato vivo.

## 15. Versoes e compatibilidade (§14 da missao + governanca)

`CHANGELOG.md` (unreleased), semver planejado para releases, sem breaking
sem RFC/aprovacao (governanca §3–§4, §6). Bytecode nao persistido =>
sem politica de compatibilidade de bytecode (documentado; nao criar
complexidade antecipada).

## 16. Matriz de estado (vivo — atualizar por entrega)

| Item | Estado | Evidencia |
|---|---|---|
| Nucleo imperativo, stdlib nucleo, modulos v1, fmt, check, diagnosticos, http_get, verificador, limites, benchmarks | IMPLEMENTADO | 133 testes verdes; gates |
| Strings/regex/arquivos/JSON/datas/matematica avancada | AUSENTE | §11 |
| Pacotes, LSP completo, debugger, REPL, test/build/inspect/doctor | AUSENTE/PROJETADO | ADR 0009, DEBUGGER_DESIGN |
| Classes/OO, exceptions, generators, closures, tipos estaticos, async | PLANEJADO (exige RFC/ADR) | §7 + Roadmap |
| PG/SQLite/MySQL, HTTPS, server HTTP, sockets API, cripto | BLOQUEADO (servidores/dependencias/decisao) | §11 + ADR futura |
| Compatibilidade magica com PHP (rodar `.php`, copiar stdlib 1:1) | NOT_APLICAVEL | Decisao: referencia, nao alvo de clonagem |

## 17. Riscos

1. **Escopo**: tentar fases 2–10 de uma vez (mitigado: fases independentes
   nesta proposta; nada obrigatorio em sequencia rigida).
2. **Quebra silenciosa**: mitigado por pins de compatibilidade + suite;
   exigir teste falha-antes/passa-depois por mudanca (regra vigente).
3. **Dependencias externas**: stdlib ambiciosa (TLS, regex, cripto, DB)
   conflita com zero-dep; cada caso exige ADR propria (nao decisao global).
4. **Complexidade arquitetural**: OO/async/tipos sem RFC viram atalhos;
   ADRs 0007–0010 mostram o padrao a seguir.
5. **Falsa paridade**: alegar equivalencia sem testes contra servidores
   reais (proibido pela missao §17.10 e pela governanca).

## 18. O que pode ser implementado imediatamente (sem preparacao)

Sem sintaxe nova ou com sintaxe trivial, sem mudar runtime/VM em
profundidade, so builtins+testes+docs (cada item = 1 `changes/` + testes):

1. Strings: `contains/split/join/trim/upper/lower/replace/starts_with/ends_with`.
2. Matematica: `abs/min/max/sqrt/pow` (+ bitwise `band/bor/bxor/bnot/shl/shr` ou operadores `& | ^ ~ << >>` — decidir 1 forma).
3. `+` ja existe; `??`/ternario/`break`/`continue` (sintaxe pequena, sem runtime novo).
4. `for x in iteravel` (acucar documentado) — opcional se `while` bastar.
5. `const NOME = expr;` (tempo de compilacao; valor imutavel).
6. `keys/values/entries/merge/has` p/ mapas; `sort/filter/map` funcionais p/ arrays.
7. `argv/env/exit` (+ `rhp test` basico como primeiro consumidor real de CLI).
8. `fs_read/fs_write[_append]/fs_exists` com jaula + tetos (mesmo modelo dos imports).
9. `rhp doctor` (env/versoes/gates) — funcional e util ja.

## 19. O que exige preparacao arquitetural (ADR/RFC antes de codigo)

Excecoes (`try/catch`, unwinding), closures/captura, generators (`yield`),
namespaces/exports/privacidade (ADR 0009 existe), classes/OO completo,
tipos estaticos, async/runtime, http server, TLS, DBs, regex, cripto,
pacotes, LSP/debugger completos, JIT. Nada disso recebe codigo nesta fase.

## 20. O que NAO deve ser implementado

- Clones da stdlib PHP 1:1 sem adaptacao a semantica RHP (valor, erros
  explicitos, limites).
- Algoritmos criptograficos proprios; regex proprio inseguro; execucao de
  scripts de terceiros sem controle; downloads automaticos.
- Comandos-stub (`test/build/inspect/doctor/repl/debug/package` sem funcao
  real — ir contra a regra vigente).
- Serializacao/versionamento de bytecode antecipados; GC sem problema
  correspondente; `unsafe` em qualquer hipotese sem ADR previa.

## 21. Roadmap recomendado (ordem tecnica, fases independentes)

1. **Stdlib imediata** (§18 itens 1–8): so builtins+sintaxe trivial; cada
   item testado; sem dependencias entre si (paralelizavel em lotes).
2. **`rhp test` + `rhp doctor`**: fecha o ciclo dev (usa a propria suite).
3. **Excecoes** (RFC+ADR): unwinding, hierarquia, `try/catch/finally`.
4. **Closures + primeira-classe real** (captura por valor — coerente).
5. **Namespaces/exports** (implementar ADR 0009) + JSON (dependencia de
   muita coisa: pacotes, APIs, testes HTTP).
6. **Pacotes v0** (init/add/install com lockfile local, sem rede? decidir).
7. **Rede v2** (redirects/headers/timeouts; HTTPS = ADR de TLS).
8. **Tipos estaticos opt-in** (RFC grande; so apos 1–5 maduros).
9. **OO** (modelo proprio + testes por regra; apos tipos basicos).
10. **Async/runtime, DBs com servidores, server HTTP, LSP/debugger** (finais;
    cada um com gate de ambiente real).

## 22. Estimativa de complexidade por modulo

P = dias uteis de 1 engenheiro focado, com testes+docs (ordens de grandeza,
nao compromissos): stdlib imediata P por lote de ~6 builtins; `??`/ternario/
`break`/`continue`/`const` P cada; `for` P; JSON M; arquivos M (jaula+tetos
ja existem como modelo); `rhp test` M; `rhp doctor` P; excecoes G;
closures M-G; namespaces/exports M (ADR pronta); pacotes v0 G; tipos
estaticos G+; OO G+; async G+; HTTP server G; TLS/DBs G+ bloqueados por
ambiente; LSP/debugger M-G (apos spans/hooks); JIT fora de questao.

---

*Auditoria: apenas leitura + sondas em `/tmp`; nenhum codigo alterado.
Proximos passos exigem aprovacao explicita, conforme governanca e missao §22.*
