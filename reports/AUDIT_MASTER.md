# AUDIT_MASTER.md — Auditoria integral do RHP (CHECKPOINT 1)

- Data: 2026-11-14
- Ramo: `feat/engineering-audit` (sem push; sem alteracao de codigo nesta fase)
- Missao: secao 3 (Fase Zero). Nenhuma refatoracao executada — somente leitura,
  execucao de testes e sondas em `/tmp` (repositorio intacto).

## 1. Linha de base (fatos verificados)

| Item | Resultado real |
|---|---|
| Toolchain | rustc 1.100.0 / cargo 1.100.0, edicao 2021 |
| `cargo fmt --check` | OK |
| `cargo clippy --workspace --all-targets -- -D warnings` | OK (zero warnings) |
| `cargo build --workspace` | OK |
| `cargo test --workspace` | **93 testes, 0 falhas** (bytecode 17, CLI unit 6, integracao 10, fmt 6, lexer 12, parser 20, VM 22) |
| `rhp run hello.rhp` | `Hello from RHP!` byte-exato |
| `rhp run examples/fib.rhp` | `55` |
| `rhp fmt --check` nos `.rhp` do repo | OK |
| Dependencias externas | **zero** (`cargo tree` = so crates do workspace) |
| `unsafe` no codigo | **nenhum** (`grep` limpo) |
| `unwrap`/`expect` fora de testes | **nenhum** (todos apos `mod tests`) |
| TODO/FIXME/`todo!` | nenhum |
| Tamanho do codigo | 4.405 linhas (`src` + 1 teste de integracao + script CI) |
| Git | limpo, em dia com `origin/main`; branch de trabalho `feat/engineering-audit` |
| Arquivo grande (4 MB, 500k `echo 1;`) | executa correto em ~7,7 s (medido 1x, sem baseline anterior) |

Metodologia: leitura integral dos 6 crates + CLI + script CI; sondas de
comportamento com arquivos em `/tmp/opencode/audit` (nenhum arquivo do repo
foi tocado); confronto implementacao x especificacoes normativas x ADRs.

## 2. Mapa do sistema (confirmado no codigo)

`rhp-lexer` (tokens+posicao) -> `rhp-parser` (AST+precedencia) ->
`rhp-bytecode` (`Value`/`Op`/`Chunk`/`compile`/`validate` recursivo) ->
`rhp-vm` (frames explicitos ate 512, escopos locais + globais, builtins incl.
`http_get` std-only) -> `rhp-cli` (`run` com loader DFS de `import`,
`fmt [--check]` via `rhp-fmt`). `tools/check-changes.sh` audita `changes/`.

Pontos fortes estruturais (preservar): erros proprios por crate com
linha/coluna; validacao de bytecode antes de executar; aritmetica inteira
checada; `Rc<FuncDef>` sem `unsafe`; `Display` deterministico (mapas/chaves
ordenadas); `validate()` recursivo; pool de nomes com `u32`.

## 3. Achados classificados

Legenda de severidade: P0 critico · P1 alto · P2 medio · P3 baixo ·
P4 melhoria futura. "Evidencia" = comando executado + saida real (sondas em
`/tmp/opencode/audit`, binario de `target/debug` compilado do HEAD).

### P0 — Critico

#### AUD-001 — Stack overflow (abort) com entrada profundamente aninhada
- Arquivos: `crates/rhp-parser/src/lib.rs` (cadeia `parse_*`, ~L435–656),
  `crates/rhp-bytecode/src/lib.rs` (`compile_expr`), `crates/rhp-vm/src/lib.rs`
  (`Display` de `Value`, `validate_inner`), `crates/rhp-fmt/src/lib.rs`.
- Descricao: programa com ~50k parenteses aninhados derruba o processo
  (`thread 'main' has overflowed its stack`, `exit=134`, SIGABRT).
- Causa: recursao sem limite de profundidade em parser, compilador, Display e
  formatador. Threads de teste tem pilha ainda menor (2 MiB): um teste de
  regressao ingenuo tambem abortaria o harness.
- Impacto/risco: negacao de servico com arquivo `.rhp` artesanal; impossivel
  testar o limite sem matar o processo.
- Evidencia: `python3 -c "print('echo ' + '('*50000 + '1' + ')'*50000 + ';')"`
  + `timeout 20 rhp run deep.rhp` => abort, `exit=134`.
- Solucao recomendada: orcamento de profundidade (ex.: 256 niveis) aplicado
  na entrada do parser (erro `ParseError`, nao abort); checagem de
  profundidade de constantes aninhadas em `validate()`; testes de borda
  (limite-1 ok, limite+1 erro limpo).
- Dependencias: atualizacao de `LANGUAGE_SPEC.md` (documentar o limite).
- Testes necessarios: regressao com profundidade limite+1 (erro, sem abort),
  profundidade limite-1 (ok), chunk artesanal aninhado (validate rejeita).

### P1 — Alto

#### AUD-101 — Formatador emite float em notacao cientifica irreparavel
- Arquivo: `crates/rhp-fmt/src/lib.rs` (`fmt_expr`, `Float` via `{:?}`).
- Descricao: `echo 0.000001;` formata para `echo 1e-6;`, que o proprio lexer
  nao aceita (`1` + identificador `e-6` => erro `esperado ';'`).
- Causa: `{:?}` de `f64` usa expoente; lexer nao tem literais com expoente.
- Impacto: `rhp fmt` corrompe programas validos (quebra o contrato de
  idempotencia/canonicidade).
- Evidencia: `rhp fmt sci_fmt.rhp` + `rhp run` => `erro sintatico em 1:7`.
- Solucao: formatar finitos com `Display` + sufixo `.0` quando sem ponto
  (sempre reparavel; `Display` nunca usa expoente); teste de roundtrip para
  `0.000001`, `1e300`, `2.0`.
- Dependencias: nenhuma mudanca de spec (formato canonico ja promete
  reparse).
- Testes: roundtrip `fmt(parse(x))` reparseavel para bateria de floats.

#### AUD-102 — Formatador emite chave de mapa sem aspas quando necessario
- Arquivo: `crates/rhp-fmt/src/lib.rs` (`Map`: `format!("{key}: ...")`).
- Descricao: `echo {"a b": 1};` formata para `echo {a b: 1};` (erro
  `esperado ':' apos chave do mapa`).
- Causa: chave sempre emitida nua; so identificadores sao validos nus.
- Evidencia: `rhp fmt mapkey_fmt.rhp` + `rhp run` => erro acima.
- Solucao: emitir nua apenas se `[_a-zA-Z][_a-zA-Z0-9]*`, senao `{:?}`.
- Testes: roundtrip de mapas com chaves arbitrarias.

#### AUD-103 — Strings com caracteres de controle nao fazem roundtrip
- Arquivos: `crates/rhp-fmt/src/lib.rs` (usa `{:?}` de Rust);
  `crates/rhp-lexer/src/lib.rs` (`lex_string`, sem `\u{...}`).
- Descricao: string contendo `\x01` formata para `"\u{1}"`, que o lexer
  rejeita (`escape invalido: \u`).
- Evidencia: `rhp fmt ctrl_fmt.rhp` + `rhp run` => erro acima.
- Solucao recomendada: adicionar escape `\u{HEX}` (+`\0`?) ao lexer
  (adicao compativel: antes era erro) + spec + testes de roundtrip.
- Dependencias: `LANGUAGE_SPEC.md` (lista de escapes), `changes/`.
- Testes: lex+fmt+run de todos os escapes; rejeicao de `\u` malformado.

#### AUD-104 — `http_get` le corpo sem limite (exaustao de memoria)
- Arquivo: `crates/rhp-vm/src/lib.rs` (`http_get_body`, `read_to_end`).
- Descricao: resposta gigante e acumulada integralmente antes de qualquer
  checagem; sem `Content-Length` previo confiavel (HTTP/1.0 + close).
- Impacto/risco: OOM via servidor malicioso/lento; sem este limite a
  "Fase 5 — limites" da missao fica incompleta na rede.
- Solucao: constante `MAX_HTTP_BODY` (propor: 1 MiB) com erro `VmError`
  ao exceder (leitura em blocos + contador); teste com servidor local
  servindo corpo acima do limite.
- Dependencias: `LANGUAGE_SPEC.md`/`VM_SPEC.md` (documentar o teto).
- Testes: corpo 2x limite => erro limpo; corpo no limite => ok.

#### AUD-105 — Resolucao DNS do `http_get` sem timeout
- Arquivo: `crates/rhp-vm/src/lib.rs` (`to_socket_addrs`).
- Descricao: `connect_timeout`/leitura tem 5 s, mas `getaddrinfo` pode
  bloquear alem disso em DNS quebrado (ADR 0004 promete "timeouts de 5s").
- Solucao minima compativel: documentar a excecao em spec/ADR + teste de
  host invalido (falha rapida e limpa — ja coberta por
  `reports_http_errors`); solucao completa (thread de resolucao) fica como
  P4 pela complexidade sem executor.
- Testes: host inexistente => erro (sem hang) — ja existe cobertura parcial.

### P2 — Medio

#### AUD-201 — Chaves duplicadas em mapa: vence a PRIMEIRA, sem documentar
- Arquivos: `crates/rhp-vm/src/lib.rs` (`BuildMap` insere em ordem reversa
  de `pop`); `LANGUAGE_SPEC.md` (omissa).
- Descricao: `echo {a: 1, a: 2};` imprime `{a: 1}` (a maioria das linguagens
  faz a ultima vencer; aqui vence a primeira — surpreendente e silencioso).
- Evidencia: sonda `dupkey.rhp` => `{a: 1}`.
- Solucao: documentar o comportamento + teste que o fixa (compativel e
  barato); alternativa (erro em duplicada) seria incompatibilidade e exige
  aprovacao — nao recomendada agora.
- Testes: pino de comportamento em VM.

#### AUD-202 — Parametros duplicados em `fn` aceitos silenciosamente
- Arquivo: `crates/rhp-bytecode/src/lib.rs` (`FnDef`; montagem do escopo em
  `call` sobrescreve sem avisar).
- Descricao: `fn f(a, a) { return a; } echo f(1, 2);` => `2`, sem erro.
- Evidencia: sonda `dupparam.rhp` => `2`.
- Solucao: erro de compilacao (`parametro duplicado: a`) + teste. Compativel
  (programas afetados hoje sao quase certamente bugs).
- Dependencias: `LANGUAGE_SPEC.md`.

#### AUD-203 — Semantica de `NaN`/`inf` nao especificada
- Arquivos: `crates/rhp-vm/src/lib.rs` (`values_equal`, `compare_values`,
  `float()` que aceita `"nan"`/`"inf"`); specs omissas.
- Descricao: `float("nan") == float("nan")` => `false`; `<` com NaN => erro
  com mensagem de *tipos* (`incompativeis`), enganosa; `int()` em faixa
  limite satura em vez de errar (ver AUD-204).
- Evidencia: sonda `nan.rhp` => `false` + `NaN`.
- Solucao: secao de NaN/inf na spec (igualdade IEEE, comparacao ordenada =
  erro dedicado ou `false`? — decidir por ADR mini) + testes.
- Testes: pinos de `nan`/`inf` em `==`, `<`, `Display`, conversoes.

#### AUD-204 — `int()` em float no limite de `i64` satura em vez de errar
- Arquivo: `crates/rhp-vm/src/lib.rs` (`Builtin::Int`, checagem
  `*v > i64::MAX as f64` — `MAX as f64` arredonda para cima).
- Descricao: `int(9223372036854775808.0)` retorna `9223372036854775807`.
- Evidencia: sonda `intedge.rhp` => `9223372036854775807`.
- Solucao: checar `*v >= 2f64.powi(63)` (ou `>= 9223372036854775808.0`) +
  teste de borda.
- Dependencias: nenhuma de spec (comportamento ja promete erro).

#### AUD-205 — Sem orcamento de instrucoes/memoria (loops e crescimento)
- Arquivos: `crates/rhp-vm/src/lib.rs` (`run`/`step`); CLI (`read_to_string`).
- Descricao: `while true {}` nunca termina; `push` em loop cresce ate OOM;
  pilha de operandos/escopos sem teto; arquivo-fonte sem teto de tamanho.
  So existem: `MAX_CALL_DEPTH=512` e timeout do HTTP.
- Risco: DoS local; mission Fase 5 exige limites configuraveis.
- Solucao recomendada (compativel): `VmConfig` opt-in
  (`max_steps`, `max_stack`, `max_value_bytes`?) com padrao = ilimitado
  (preserva comportamento); teto de tamanho de fonte no loader
  (ex.: 8 MiB, erro limpo); ADR + testes de cada teto.
- Dependencias: ADR novo; `VM_SPEC.md`; sem mudanca de bytecode.
- Testes: loop com `max_steps` => erro; array gigante => erro; fonte acima
  do teto => erro do loader.

#### AUD-206 — `import` sem confinamento de diretorio
- Arquivo: `crates/rhp-cli/src/main.rs` (`load_file`, `canonicalize`).
- Descricao: `import "/etc/hostname"` e lido (depois falha no lex — sem
  panico nem vazamento alem da mensagem, mas leitura arbitraria).
- Evidencia: sonda `ext.rhp` => erro limpo `exit=1` (sem crash).
- Solucao: confinar imports a subarvore do arquivo de entrada
  (`canonical.starts_with(base)`, erro dedicado) + teste.
- Dependencias: `LANGUAGE_SPEC.md` (modulos).

#### AUD-207 — `import` aninhado em `fn`/bloco e icado silenciosamente
- Arquivo: `crates/rhp-cli/src/main.rs` (`collect_imports_into` desce em
  `FnDef`/blocos; modulos executam no topo, antes do arquivo).
- Descricao: `fn f() { import "x.rhp"; }` carrega `x` no inicio do programa,
  nao quando `f` e chamada — semantica surpreendente e nao documentada.
- Solucao: aceitar `import` apenas no nivel superior (erro de parse ou do
  loader nos demais) + documentar + testes.
- Dependencias: `LANGUAGE_SPEC.md`.

#### AUD-208 — `let` sombreando parametro da funcao e erro nao documentado
- Arquivos: `crates/rhp-vm/src/lib.rs` (`declare`); spec omissa.
- Descricao: `fn f(a) { let a = 1; }` => `variavel ja declarada: a`
  (corpo da funcao nao abre escopo proprio para o nivel superior).
- Evidencia: sonda `shadow.rhp` => erro acima.
- Solucao: documentar (comportamento defensavel) + teste pino.
- Testes: ja existe cobertura parcial em VM (`handles_variables_and_scopes`
  nao cobre este caso — adicionar).

#### AUD-209 — Precedencia de builtins sobre variaveis nao documentada
- Arquivos: `crates/rhp-bytecode/src/lib.rs` (`Call` => `CallBuiltin` se o
  nome e builtin, ignorando variavel homonima).
- Descricao: `let len = 5; echo len(1);` chama o builtin, nao a variavel;
  `echo len;` le a variavel. Previsivel apos conhecido, mas nao especificado.
- Solucao: documentar em `LANGUAGE_SPEC.md` (builtins reservados em posicao
  de chamada; evitar esses nomes) + teste pino.

#### AUD-210 — Ausencia de suite de benchmarks
- Descricao: missao Fase 7 exige metodologia reproduzivel; hoje so existe
  uma medicao ad-hoc (4 MB/500k `echo` em ~7,7 s).
- Solucao: harness std-only (sem crates externas — proibidas sem
  autorizacao) com programas fixos (fib, aritmetica, arrays, mapas,
  importacao), separando lex/parse/compile/run, + doc de metodologia e
  maquina; registrar baseline, sem otimizar antes.
- Dependencias: nenhuma mudanca de produto.

#### AUD-211 — Ausencia de fuzz/property tests
- Descricao: entradas malformadas so cobertas por casos escritos a mao;
  `cargo-fuzz`/proptest sao crates externas (proibidas sem autorizacao).
- Solucao: corpus de entradas malformadas em teste unitario (lex/parse
  nunca podem dar panic — so `Err`), + property simples com PRNG proprio
  e seed fixa (ex.: roundtrip `fmt`, `a == a`, `len(push(a,x)) == len(a)+1`).
- Dependencias: nenhuma.

#### AUD-212 — Verificacao de exemplos nao automatizada em CI
- Descricao: `hello.rhp`/`fib.rhp` tem cobertura em testes de integracao,
  mas nao ha etapa nomeada "exemplos" (missao Fase 10) nem checagem de
  `fmt --check` nos exemplos fora do teste do proprio fmt.
- Solucao: estender integracao (ou `tools/check-examples.sh`) que executa
  `examples/*.rhp` com saidas esperadas + `fmt --check`; referenciar no
  relatorio/CI.
- Dependencias: nenhuma mudanca de produto.

#### AUD-213 — `check-changes.sh`: casos-limite
- Arquivo: `tools/check-changes.sh`.
- Descricao: (a) CI raso (`fetch-depth: 1`) pode quebrar `base...HEAD`
  (fallback atual compara base x worktree — impreciso); (b) `git mv` de
  registro aparece como rename (filtro `-A` nao casa) exigindo bypass.
- Solucao: documentar `fetch-depth: 0` no futuro workflow + aceitar
  renames (`--diff-filter=AR`) ou orientar `--bypass` justificado; teste
  manual ja cobre 4 cenarios, adicionar os 2 novos.
- Dependencias: doc de CI.

### P3 — Baixo

- AUD-301: docstrings com fase desatualizada (`rhp-lexer`/`rhp-vm` "Fase 2",
  `rhp-bytecode` "Fase 3", etc.) — trocar por texto atemporal.
- AUD-302: typo "O alvao valido" (`rhp-parser`, doc de `parse_assign_or_expr_stmt`).
- AUD-303: `+5` (mais unario) rejeitado (`esperado expressao`) — adicao
  compativel trivial + teste; ou documentar a ausencia.
- AUD-304: `intern_name` com busca linear O(n) + sem deduplicacao de
  constantes (cada literal `"a"` = nova entrada) — trocar por
  `HashMap` + dedup de `Value` simples (ganho em arquivos grandes; sem
  mudanca semantica).
- AUD-305: erro de alvo de atribuicao aponta posicao apos `=` em vez do
  inicio do alvo — melhorar apos spans na AST (depende de P4/diagnosticos).
- AUD-306: `Host:` sem porta no `http_get`; resposta `chunked` a pedido
  HTTP/1.0 nao tratada — documentar limitacoes.
- AUD-307: `Display` de `f64` grandes via `{}` nunca usa expoente (ok), mas
  precisao em promocao `Int as f64` — nota documental.
- AUD-308: docstring da VM ("nao esperada no Marco 2") desatualizada.
- AUD-309: trailing comma rejeitada (`[1, 2,]`) — documentar ou aceitar
  (decisao trivial; manter rejeicao documentada e o mais barato).
- AUD-310: `MOD`/`DIV` por `-0.0` ja tratados (`== 0.0` pega `-0.0`) — sem
  acao; registrar como verificado (feito aqui).

### P4 — Melhoria futura (nao implementar sem RFC/aprovacao)

- AUD-401: PostgreSQL/Redis/async/classes/generics (ja em `ROADMAP.md`).
- AUD-402: `export`/privacidade, gerenciador de pacotes, isolamento e cache
  de modulos.
- AUD-403: HTTPS, redirects, headers, `http_post`, streaming.
- AUD-404: closures sobre locais, metodos, `break`/`continue`, indexacao de
  strings, slices, `else if` dedicado.
- AUD-405: otimizacoes (tail-call, arena/internacao, dedup) so apos
  benchmarks (Fase 7 da missao); decisao de memoria (`MEMORY_MODEL.md`).
- AUD-406: esquemas de diagnosticos `E0001` com spans/snippet (Fase 4 da
  missao — escopo proprio com ADR).
- AUD-407: versionamento/compatibilidade formal de bytecode persistido
  (hoje bytecode nao e serializado — documentar isso encerra o tema por ora).

## 4. Conformidade com a governanca (verificado)

- `changes/` possui 1 registro por fase + README modelo; `CHANGELOG.md`
  consolidado em `unreleased`; sem versoes/metricas inventadas.
- Specs descrevem o implementado; `MEMORY_MODEL.md`/`PHP_DIFFERENCES.md`
  honestos ("A definir"); ADRs 0001–0004 presentes.
- Proibicoes respeitadas no codigo: sem `unsafe`, sem `unwrap`/`expect` em
  producao, sem breaking silencioso conhecido (mapa duplicado e NaN sao
  lacunas de *especificacao*, nao regressoes — Fase 1 intacta e reverificada).
- Pendencia de processo: `feat/engineering-audit` criado e nao publicado
  (correto — sem push sem autorizacao).

## 5. Priorizacao recomendada para os proximos checkpoints

1. **Checkpoint 3 (nucleo)**: AUD-001 (limite de profundidade), AUD-101/102
   (fmt float e chaves), AUD-103 (escape `\u`), AUD-202 (params duplicados),
   AUD-204 (`int` limite), AUD-303 (mais unario) — todos com teste
   falha-antes/passa-depois + specs + `changes/` + ADRs pontuais.
2. **Checkpoint 4 (seguranca)**: AUD-104 (teto HTTP), AUD-105 (doc DNS),
   AUD-205 (`VmConfig` opt-in + teto de fonte), AUD-206 (jaula de import),
   AUD-207 (import so no topo), docs AUD-201/203/208/209 + testes.
3. **Checkpoint 2 (docs) pode rodar em paralelo de conteudo**: README
   reestruturado (missao §4.1), `docs/` (getting-started, reference
   consolidando specs sem duplicar, CLI, diagnostics-atual, security com
   garantias reais, FAQ), `CONTRIBUTING.md`+`SECURITY.md` enxutos.
4. **Checkpoint 5 (performance)**: AUD-210 (harness+baseline) antes de
   qualquer otimizacao; AUD-304 so com numeros.
5. **Nao fazer agora**: P4, reescritas amplas, `#[allow]` novos, crates
   externas, mudanca de semantica sem ADR.

## 6. Anexos — comandos e saidas

```bash
$ cargo fmt --check            # BASE_FMT_OK (silencioso = ok)
$ cargo clippy --workspace --all-targets -- -D warnings   # Finished, 0 warnings
$ cargo test --workspace       # 93 passed, 0 failed (17+6+10+6+12+20+22)
$ ./target/debug/rhp run hello.rhp      # Hello from RHP!
$ ./target/debug/rhp run examples/fib.rhp  # 55
$ ./target/debug/rhp fmt --check hello.rhp examples/fib.rhp  # ok
$ printf 'echo 0.000001;\n' > sci.rhp && rhp fmt sci_fmt.rhp
  # => echo 1e-6;  +  run => erro sintatico em 1:7 (AUD-101)
$ printf 'echo {"a b": 1};\n' ...  # => echo {a b: 1}; + run => erro (AUD-102)
$ printf 'echo "a\x01b";\n' ...   # => echo "a\u{1}b"; + run => escape invalido (AUD-103)
$ python3 -c "print('echo ' + '('*50000 + '1' + ')'*50000 + ';')" > deep.rhp
$ timeout 20 rhp run deep.rhp  # thread 'main' has overflowed its stack ... exit=134 (AUD-001)
$ echo 'fn f(a, a) { return a; } echo f(1, 2);'  # => 2 (AUD-202)
$ echo 'echo {a: 1, a: 2};'                    # => {a: 1} (AUD-201)
$ echo 'echo int(9223372036854775808.0);'      # => 9223372036854775807 (AUD-204)
$ echo 'echo float("nan") == float("nan");'    # => false (AUD-203)
$ echo 'echo +5;'  # => 1:6: esperado expressao, encontrado `+` (AUD-303)
$ import "/etc/hostname"  # => erro limpo, exit=1, sem panico (base p/ AUD-206)
$ 500k echo => 4 MB, ~7,7 s, saida correta (baseline p/ AUD-210)
$ cargo tree --prefix none | sort -u  # so crates do workspace (0 externas)
$ grep -rn unsafe crates/*/src/      # vazio
```
