# NEXT_GENERATION_AUDIT.md — Auditoria de maturidade (CHECKPOINT 1)

- Data: 2026-11-14 · Ramo: `feat/next-generation` (sem push; sem mudanca de codigo)
- Escopo: capacidade do RHP de suportar projetos maiores (missao §2).
  Auditoria anterior (`reports/AUDIT_MASTER.md`, P0/P1 corrigidos) nao e
  repetida aqui — apenas referenciada quando o status mudou.

## 1. Linha de base (fatos verificados hoje)

| Item | Resultado real |
|---|---|
| `cargo fmt --check` / `clippy -D warnings` / `build` | OK |
| `cargo test --workspace` | **120 testes, 0 falhas** (20+10+13+8+15+25+29) |
| Deps externas / `unsafe` / TODOs | zero / nenhum / nenhum |
| `push 20k` (debug) | ~98 s, pico RSS **8 MB**, saida correta |
| Array literal 100k itens (300 KB fonte) | 0,6 s, ok |
| Cadeia de 200 imports | 14 ms totais |
| Redirect HTTP 301 | erro limpo `status 301` (sem seguir) |
| `int("1_0")` | erro (conversao estrita, sem `_`) |
| `rhp --help` | funcional, mas rotulo "Fase 5" desatualizado |

Metodo: leitura integral do workspace + docs normativas + ADRs + `changes/` +
relatorios anteriores; sondas em `/tmp/opencode` (repo intacto).

## 2. Mapa rapido (confirmado no codigo)

Lexer (posicao+escapes incl. `\u{...}`) -> parser (precedencia, limite 128)
-> bytecode (`Value`+`Func`, pools, `CallBuiltin`, `validate()` iterativo) ->
VM (frames, escopos+globais, builtins incl. `http_get`, `VmLimits` opt-in) ->
CLI (`run [--max-steps]`, `fmt`, loader DFS com jaula+teto 8 MiB).

## 3. Achados (P0 = critico … P4 = futuro)

### P1 — Alto

#### NG-101 — Semantica por valor tem custo quadratico sem alternativa
- Arquivos: `crates/rhp-vm/src/lib.rs` (`push`, `store_index_value`,
  `BuildArray` — clones integrais); `crates/rhp-bytecode/src/lib.rs`
  (`Display`/igualdade recursivos).
- Causa: cada `push`/atribuicao clona o composto inteiro (ADR 0002).
- Impacto/risco: `push` em loop 20k = ~10 s (release) / ~98 s (debug);
  projetos maiores com vetores/mapas grandes ficam inviaveis em tempo
  (memoria de pico segue baixa: 8 MB medidos — retencao ok, CPU nao).
- Evidencia: `/usr/bin/time -v` + bench `run push 20k` (ver
  `docs/BENCHMARKS.md`).
- Solucao: NAO mudar a semantica (breaking + RFC). Caminho: medir (feito)
  -> ADR de memoria (`reports/MEMORY_MODEL.md` da missao §4) comparando
  Rc/CoW/refcount/GC -> so entao implementar, com teste de compatibilidade
  que prove copia observavel equivalente.
- Testes necessarios: teste de equivalencia observavel (aliasing aparente
  zero) + benchmarks antes/depois + teste de memoria (pico RSS).
- Dependencias: decisao de memoria (Checkpoint 3 da missao).

#### NG-102 — Pilha de operandos/heap de valores sem teto
- Arquivos: `crates/rhp-vm/src/lib.rs` (`Frame.stack`, `Vec<Value>` sem
  limite); `check_constant_depth` limita *profundidade*, nao *tamanho*.
- Causa: `VmLimits` cobre so instrucoes; literal `[1; 100k]` aloca ate OOM.
- Impacto/risco: DoS por fonte valida (dentro do teto de 8 MiB cabem ~2M
  de itens); validacao aprova, execucao estoura memoria.
- Evidencia: literal 100k ok em 0,6 s (funciona, mas sem teto nada impede
  10x isso); `BuildArray(u32::MAX)` artesanal falha limpo (pilha
  insuficiente) — o buraco e volume *valido*, nao underflow.
- Solucao: estender `VmLimits` (`max_stack_depth`, `max_value_bytes` com
  medicao aproximada) opt-in, padrao ilimitado; documentar em `VM_SPEC.md`.
- Testes: literal gigante com limite => erro limpo; sem limite => ok.
- Dependencias: nenhuma mudanca de bytecode.

### P2 — Medio

#### NG-201 — Diagnosticos sem estrutura central (bloqueia LSP/debugger)
- Arquivos: mensagens espalhadas em `rhp-lexer`, `rhp-parser`,
  `rhp-bytecode`, `rhp-vm` (strings `format!`, sem codigos, sem spans).
- Causa: cada crate formata seu erro para stderr; sem representacao
  estruturada (arquivo, intervalo, severidade, sugestao).
- Impacto/risco: autocomplete/hover/goto e debugger (missao §8) nao tem
  sobre o que se apoiar; mensagens inconsistentes entre crates.
- Solucao: crate `rhp-diagnostic` (`Diagnostic { codigo, severidade,
  arquivo, span, mensagem, ajuda }`) + renderer terminal separado do dado
  (LSP-ready); migrar crates por etapas com testes por categoria.
- Testes: um por categoria (lex/sintaxe/compile/runtime) incl. spans.
- Dependencias: spans na AST/lexer (hoje so posicao inicial por token).

#### NG-202 — Verificador nao analisa fluxo de pilha
- Arquivo: `crates/rhp-bytecode/src/lib.rs` (`validate_shallow`: pools,
  saltos, `Halt`, profundidade — mas nao altura de pilha).
- Causa: underflow (`Print` sem valor, aridade de `Call`) so aparece em
  runtime; saltos com alturas divergentes nao sao detectados.
- Impacto/risco: medio — bytecode artesanal/hostil falha em runtime com
  erro limpo (sem abort), mas a missao §5 exige prova estrutural previa.
- Solucao: analise abstrata de altura por instrucao com merge nos alvos de
  salto (erro estatico em divergencia/underflow); testes com chunks
  truncados/malformados/limite.
- Testes: corpus de bytecode invalido (truncado, salto p/ meio de nada —
  ja coberto bounds —, pilha negativa, aridade).
- Dependencias: `BYTECODE_SPEC.md` (regras de efeito de pilha por op).

#### NG-203 — Modulos v1 sem exports/privacidade/alias/cache
- Arquivos: `crates/rhp-cli/src/main.rs` (loader textual com `once`,
  namespace global unico); `LANGUAGE_SPEC.md`.
- Causa: desenho v1 proposital (ADR 0003); colisoes viram erro alto.
- Impacto/risco: projetos maiores sem encapsulamento; re-parse total a cada
  `run` (cadeia de 200 arquivos = 14 ms — aceitavel hoje, sem cache
  invalida nada ainda).
- Solucao: ADR de semantica (exports, privacidade, alias, identidade
  canonica, cache com chave de conteudo, isolamento) ANTES de implementar;
  sem mudar o `import` atual (compatibilidade).
- Testes: matriz de colisao/privacidade/ciclo/determinismo quando
  implementado.
- Dependencias: especificacao da linguagem (sintaxe nova).

#### NG-204 — HTTP: sem redirect/HTTPS/chunked-robusto/taxa; DNS sem timeout
- Arquivo: `crates/rhp-vm/src/lib.rs` (`http_get_body`); ADR 0004.
- Causa: cliente minimo proposital; sondado: 301 => erro limpo (correto
  e documentado); `Host` sem porta; resposta `chunked` a pedido HTTP/1.0
  seria devolvida com framing (nao testado contra servidor real chunked).
- Impacto/risco: integracoes reais precisam de HTTPS/redirects; sem limite
  de taxa, leitura lenta consome os 5 s e falha (aceitavel, documentado).
- Solucao: escopo em ADR (niveis: strict-atual vs `http_get_opts` futuro);
  nao prometer sandbox de rede que nao existe.
- Testes: redirect=>erro (feito na sonda; formalizar em teste), chunked
  contra servidor real, timeout de leitura lenta.
- Dependencias: nenhuma (comportamento atual ja e erro limpo).

#### NG-205 — Sem suite de CI executavel no repo (so script local)
- Arquivos: `tools/check-changes.sh` (existe, testado em fixture); ausencia
  de `.github/workflows/`.
- Causa: CI nunca foi instalada de fato (a Fase 5 entregou o script).
- Impacto/risco: gates dependem de disciplina local (hook); PRs sem
  verificacao automatica.
- Solucao: workflow com `fetch-depth: 0` rodando fmt/clippy/test/build,
  exemplos, `check-changes.sh base=origin/main`, auditoria de `unsafe`
  (`grep -rn`), validacao de ADRs/`changes/`; sem segredos no repo.
- Testes: o proprio workflow e a verificacao (falha proposital em branch
  de teste e reversao — fora do escopo deste checkpoint).
- Dependencias: permissao do responsavel (CI executa em infra externa).

#### NG-206 — `VmLimits` cobre passos, nao memoria/tempo de parede
- Arquivo: `crates/rhp-vm/src/lib.rs` (`VmLimits`, `Vm::set_limits`).
- Causa: limite de tempo real exigiria watchdog/thread; limite de heap,
  alocador instrumentado.
- Impacto/risco: programa lento-nao-infinito consome CPU ate o teto de
  passos (que e alto por necessidade); OOM segue possivel fora dos tetos
  pontuais (corpo HTTP, fonte, profundidade).
- Solucao: documentar fronteira em `SECURITY.md` + `VM_SPEC.md` (feito
  parcialmente: DNS); avaliar watchdog opt-in em ADR propria (riscos de
  condicao de corrida no `Write` compartilhado).
- Testes: teste de timeout quando implementado.
- Dependencias: decisao de runtime (threads) — ver NG-301.

### P3 — Baixo

- NG-301: `rhp --help` rotula "Fase 5" (desatualizado desde CP seguinte);
  sem `rhp doctor`/`--version` detalhado (commit/toolchain). Corrigir textos
  + especificar comandos futuros sem implementar stubs (missao §9 proibe
  comando sem funcionalidade real).
- NG-302: `intern_name` O(n) + sem dedup de constantes (5000 stmts
  identicos = 15000 entradas no pool). Medido baseline em
  `docs/BENCHMARKS.md`; otimizar (HashMap/dedup) so com antes/depois.
- NG-303: `Display`/`fmt` de `f64` grandes dependem de expansao decimal
  exata (correto, mas 1e300 vira 301 digitos). Aceitavel e testado; notar.
- NG-304: erro de alvo de atribuicao aponta posicao apos `=` (falta span
  de origem — depende de NG-201).
- NG-305: `fmt` nao preserva linhas em branco (canonico agressivo,
  documentado pelo comportamento + teste de idempotencia; ok).
- NG-306: convertibilidade `int(" 42 ")` com trim (documentar) — verificado
  estrito para `_` (`int("1_0")` = erro).
- NG-307: `check-changes.sh` exige git + base existente; sem `--help`
  completo. Melhorias pontuais ja aplicadas (renames, fetch-depth doc).

### P4 — Futuro (requer RFC/aprovacao; nao implementar)

- NG-401: PostgreSQL/Redis (servidores reais), async/concorrencia (runtime),
  classes/generics/interfaces, metodos, `break`/`continue`, slices,
  indexacao de strings.
- NG-402: exports/privacidade/alias/cache, gerenciador de pacotes.
- NG-403: HTTPS/redirects/headers/streaming (decisao de escopo via ADR).
- NG-404: GC vs RC vs CoW vs hibrido (`reports/MEMORY_MODEL.md` da missao
  §4 — produzir no Checkpoint 3).
- NG-405: LSP (`rhp-lsp`), debugger (breakpoints/step/inspecao),
  syntax highlighting (dependem de NG-201 + hooks de execucao).
- NG-406: `rhp check/test/build/inspect/doctor` (especificar antes de criar).
- NG-407: versionamento de bytecode persistido (hoje nao serializado —
  documentar encerra o tema, cf. governanca §10).

## 4. Conformidade com a governanca (este checkpoint)

- Zero crates externas (verificado `cargo tree`); sem `unsafe`; sem
  `unwrap`/`expect` em producao; sem breaking silencioso (mudancas desta
  missao ainda: nenhuma — so auditoria).
- Ramo `feat/next-generation` (convencao `feat/...`); sem push; sem
  alteracao de governanca; `changes/` sera atualizado a cada entrega.

## 5. Priorizacao recomendada (proximos checkpoints)

1. **CP2 Diagnosticos** (NG-201): crate central + spans minimos + testes por
   categoria (desbloqueia LSP/debugger e melhora CLI). Dependencia: spans.
2. **CP3 Memoria** (NG-101 + missao §4): medir (parcialmente feito) ->
   ADR comparativo -> implementar vencedor com teste de equivalencia.
3. **CP4 Bytecode+VM** (NG-202 + limites NG-102): analise de pilha +
   `VmLimits` estendido, com benchmarks.
4. **CP5 Modulos** (NG-203): ADR de semantica primeiro; sem codigo antes.
5. **CP6 DevTools** (NG-301/405/406): highlighting + arquitetura LSP/debugger.
6. **CP7+**: limites restantes, performance com protocolo, docs, relatorio.

## 6. Anexos — evidencias

```bash
$ cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings  # ok
$ cargo test --workspace   # 120 passed, 0 failed (20+10+13+8+15+25+29)
$ /usr/bin/time -v rhp run push20k.rhp   # ~98 s debug, pico RSS 8 MB, ok
$ rhp run bigrarr.rhp      # 100k itens, 0,6 s, ok
$ cadeia de 200 imports    # 14 ms totais
$ http 301 local           # erro limpo `status 301`, exit 1
$ int("1_0")               # erro (estrito)
$ cargo tree --prefix none # so workspace (0 externas)
$ grep -rn unsafe crates/*/src/   # vazio
```
