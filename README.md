# RHP

**Uma linguagem de programação independente — lexer, parser, compilador para bytecode, máquina virtual e runtime próprios, implementados em Rust.**

Sintaxe familiar a quem vem do PHP. Semântica própria, explícita e testada: sem coerção silenciosa, sem `truthiness`, erros claros com código estável.

[![Versão](https://img.shields.io/badge/vers%C3%A3o-0.7.2-blue)](CHANGELOG.md)
[![Testes](https://img.shields.io/badge/testes-395_verdes-success)](#qualidade-garantida-por-gates)
[![Linguagem Rust](https://img.shields.io/badge/linguagem-Rust_2021-orange)](https://www.rust-lang.org)
[![Licença](https://img.shields.io/badge/licen%C3%A7a-MIT_OR_Apache--2.0-blue)](LICENSE-MIT)
[![Docker](https://img.shields.io/badge/docker-bigfatherdev%2Frhp-blue)](https://hub.docker.com/r/bigfatherdev/rhp)

## O que há de novo na 0.7.2

Auditoria completa de engenharia sobre a 0.7.1 — compilador, VM, os 85 builtins um a um, conectores, HTTP, concorrência, segurança, performance e CLI — com cada achado corrigido na causa raiz e travado por teste de regressão:

- **Robustez (5 achados críticos zerados):** `for-in` aninhados compilam em tempo linear (antes travavam em 2^N); `yield` dentro de `try` preserva o handler; valores com 128+ níveis de profundidade viram erro capturável em vez de abortar o processo; respostas Redis/MySQL maliciosas com tamanho absurdo viram erro em vez de derrubar o runtime.
- **Correção de comportamento:** `next` respeita o limite de 512 frames; callbacks de `map`/`filter`/`usort` aceitam só função RHP; `catch` tipado funciona em qualquer profundidade de herança; `echo http_serve(...)` no topo do programa passa a servir de verdade; `?` dentro de string SQL não é mais tratado como placeholder.
- **Performance medida:** `json_parse` de strings longas de O(n²) para O(n) — 400 KB em 0,2 s (antes 8,6 s).
- **Experiência de uso:** REPL com blocos multilinha e imunidade a erro de sintaxe; `rhp test` desce em subdiretórios; `argv()` sem o `--`; mensagens de erro com os tipos reais.
- **Documentação honesta:** dependências, `unsafe` isolado no FFI SQLite e limites reais agora descritos como o código realmente é.

Detalhe item a item em [`CHANGELOG.md`](CHANGELOG.md); histórico auditável em [`changes/`](changes/).

## Comece em 2 minutos

Pré-requisito: [toolchain Rust stable](https://rustup.rs) (verificado com 1.100.0).

```bash
git clone https://github.com/bigfatherdev/rhp.git
cd rhp
cargo build                    # binário em ./target/debug/rhp
./target/debug/rhp run hello.rhp
# Hello from RHP!
```

Ou via Docker (multi-arquitetura `linux/amd64` + `linux/arm64`, usuário não privilegiado):

```bash
docker pull bigfatherdev/rhp:latest
docker run --rm bigfatherdev/rhp:latest --version   # rhp 0.7.2
docker run --rm -v "$PWD:/work" bigfatherdev/rhp:latest run hello.rhp
```

Tags: `latest` (main atual), `0.7.2` / `0.7` / `0` (releases). Detalhes em [`docs/DOCKER.md`](docs/DOCKER.md).

## A linguagem em 60 segundos

```rhp
// Fibonacci iterativo: tipos, variáveis, controle e funções.
fn fib(n) {
  if n < 2 {
    return n;
  }
  let a = 0;
  let b = 1;
  let i = 2;
  while i <= n {
    let c = a + b;
    a = b;
    b = c;
    i = i + 1;
  }
  return b;
}

echo fib(10); // 55
```

E quando precisar ir além do script simples — classes, geradores, async e um servidor HTTP de verdade:

```rhp
class Contador {
  fn novo() { this["n"] = 0; }
  fn inc() { this["n"] = this["n"] + 1; return this["n"]; }
}

async fn buscar(id) {
  let h = db_open("/tmp/app.db");
  let linhas = db_query(h, "SELECT nome FROM usuarios WHERE id = ?", [id]);
  db_close(h);
  return linhas;
}

fn handler(req) {
  return {status: 200, corpo: "olá, " + req["caminho"]};
}
http_serve(8080, "handler", {});
```

## Por que RHP

- **Contratos explícitos** — especificações normativas ([`LANGUAGE_SPEC.md`](LANGUAGE_SPEC.md), [`BYTECODE_SPEC.md`](BYTECODE_SPEC.md), [`VM_SPEC.md`](VM_SPEC.md), [`MEMORY_MODEL.md`](MEMORY_MODEL.md)) com precedência definida em [`GOVERNANCE.md`](GOVERNANCE.md). O código não é a especificação: quando divergem, a divergência é tratada como bug.
- **Sem mágica de tipos** — `if`, `&&` e `filter` exigem `Bool` de verdade; aritmética inteira checada (estouro e `/0` são erro); inteiros e floats não se misturam sem conversão explícita.
- **Valores por valor** — arrays e mapas copiam integralmente; sem aliasing observável; `a[i] = v` é atômico.
- **Erros que ajudam** — todo erro carrega código estável (`E1001`…`E3012`), trecho da fonte e sugestão; erros de runtime são capturáveis por `try`/`catch`/`finally`, inclusive os tipados por hierarquia de classes.
- **Concorrência isolada** — tarefas em threads próprias sem estado compartilhado (snapshot por valor); cancelamento cooperativo, timeout, `await_all`/`await_race`.
- **Baterias reais** — SQLite, PostgreSQL, MySQL (inclui `caching_sha2_password`), Redis e TCP com protocolo próprio; `http_get`/`http_fetch` (HTTPS verificado) e `http_serve` com framework web em [`framework/web.rhp`](framework/web.rhp); JSON, Argon2/bcrypt, SHA-2/HMAC, regex linear; 85 builtins com aridade checada na compilação.
- **Ferramental próprio** — formatador canônico idempotente, `rhp check` (valida sem executar), REPL com estado, LSP, `rhp test`, pacotes locais com lockfile e `rhp build` (bundle autocontido).

## CLI

| Comando | Efeito |
|---|---|
| `rhp run [--max-steps N] [--max-heap-bytes N] <arquivo.rhp> [-- args]` | Executa (resolve `import`s); exit 0 ok / 1 erro / 2 uso inválido |
| `rhp repl` | Interativo com estado, blocos multilinha e imunidade a erro |
| `rhp test [dir\|arquivo]` | Executa `*_test.rhp` (desce subdiretórios); exit 1 se falhar |
| `rhp check [--json] <arquivo.rhp>` | Lex + imports + compilação **sem executar** |
| `rhp fmt [--check] <arquivo.rhp>` | Formata no estilo canônico (idempotente) |
| `rhp build <entrada.rhp> [saida]` | Gera bundle autocontido |
| `rhp pkg <init\|add\|install>` | Pacotes locais com `rhp.lock` |
| `rhp doctor` | Diagnóstico do ambiente |
| `rhp lsp` | Servidor LSP (JSON-RPC sobre stdio) |
| `rhp --help`, `rhp --version` | Ajuda e versão |

Referência completa em [`docs/CLI_REFERENCE.md`](docs/CLI_REFERENCE.md).

## Tour pela sintaxe

```rhp
// Variáveis, tipos e echo de qualquer valor.
let nome = "rhp";
let versao = 0.1;
echo nome + " " + str(versao); // "rhp 0.1"

// Aritmética checada (tipos mistos inválidos são erro, não coerção).
echo 1 + 2 * 3;      // 7
echo 10 % 3 == 1;    // true

// Arrays e mapas por valor (sem aliasing).
let a = [1, 2];
a = push(a, 3);
echo len(a);         // 3
a[0] = 9;
echo a;              // [9, 2, 3]
echo keys({b: 1, a: 2}); // [a, b]

// Controle de fluxo (condição sempre Bool).
let i = 0;
while i < 3 {
  if i == 1 {
    echo "um";
  } else {
    echo i;
  }
  i = i + 1;
}

// Funções, closures por valor e alta ordem.
let dobro = fn (v) { return v * 2; };
echo map([1, 2, 3], dobro); // [2, 4, 6]

// Módulos: cada arquivo executa uma vez, antes do importador.
import "mat.rhp";
echo dobro(21); // 42 (se mat.rhp define fn dobro(x) { return x * 2; })
```

Regras que valem sempre: `;` termina instruções; `//` comenta até o fim da linha; `x = …` exige `let` prévio; `a[i] = v` só em um nível; chaves de mapa duplicadas vencem a **primeira**; `nan == nan` é `false` (IEEE 754); limite de 64 níveis de aninhamento sintático. Detalhes normativos em [`LANGUAGE_SPEC.md`](LANGUAGE_SPEC.md).

## Qualidade garantida por gates

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace          # 395 testes, 0 falhas
./tools/fuzz-smoke.sh 1000      # fuzz mutacional: 0 suspeitos
./target/debug/rhp run examples/fib.rhp          # 55
tools/check-changes.sh          # exige registro em changes/ para diffs relevantes
```

Cobertura: lexer, parser, bytecode (inclui validação de fluxo de pilha), VM, os 85 builtins, módulos, CLI, formatador, diagnósticos, rede contra servidores locais e fakes adversários, corpus malformado, `property test` determinístico, verificação dos 20 exemplos e do framework web ao vivo.

## Arquitetura

```text
fonte --loader--> tokens --parser--> AST --compile--> Chunk --vm.run--> saída
rhp-lexer -> rhp-parser -> rhp-bytecode -> rhp-vm -> rhp-cli (+rhp-fmt, +rhp-diagnostic)
```

`Chunk { constants, names, funcs, code }` validado (pools, saltos, `Halt`, fluxo de pilha) **antes** de executar; frames explícitos com limite de 512; `VmLimits` opt-in (`max_steps`, `max_stack_depth`, `max_collection_len`, `max_heap_bytes`); dependências externas mínimas e auditadas (cripto/TLS/regex + FFI SQLite isolado). Detalhes em [`ARCHITECTURE.md`](ARCHITECTURE.md) e [`ADRs/`](ADRs/).

| Caminho | Papel |
|---|---|
| `crates/rhp-*` | Lexer, parser/AST, bytecode, VM, formatador, diagnósticos, CLI |
| `examples/` (20) | Programas `.rhp` versionados (todos executados em teste) |
| `framework/` | Framework web sobre `http_serve` + exemplo funcional |
| `editors/vscode/` | Gramática TextMate + configuração da linguagem |
| `tools/` | `check-changes.sh`, `fuzz-smoke.sh`, prova Docker do HTTP |
| `reports/` + `audit/` | Auditorias e relatórios de engenharia |
| `docs/` | Guias (getting started, CLI, diagnósticos, benchmarks, FAQ…) |
| `changes/` | Histórico auditável (um arquivo por mudança) |

## Segurança (modelo honesto)

Garantido e testado: `unsafe` isolado no FFI SQLite (invariantes documentadas, sem alcance por entrada RHP); bytecode validado antes de executar; aritmética inteira checada; profundidade sintática limitada (64) e de exibição (128, com erro capturável); imports confinados à árvore do programa (teto de 8 MiB/arquivo); `http_get` só `http://` com teto de 1 MiB; `http_fetch` com HTTPS verificado (raízes embutidas + `RHP_CA_BUNDLE`); `VmLimits` opt-in. Explicitamente **não** garantido: isolamento de SO/sandbox, TLS no servidor, TLS no MySQL (há RFC proposta), heap sem teto medido, DNS com timeout próprio. Detalhes e reporte em [`SECURITY.md`](SECURITY.md).

## Documentação

| Documento | O quê |
|---|---|
| `docs/GETTING_STARTED.md` | RHP em 5 minutos |
| `docs/LANGUAGE_REFERENCE.md` | Guia da linguagem com exemplos |
| `docs/REFERENCIA.md` | Referência completa (builtins, semântica) |
| `docs/ESTADO-RHP.md` | Panorama consolidado: o que faz e o que falta |
| `LANGUAGE_SPEC.md` | Contrato normativo da linguagem |
| `BYTECODE_SPEC.md` / `VM_SPEC.md` | Contrato do bytecode e da VM |
| `docs/CLI_REFERENCE.md` | Referência completa da CLI |
| `docs/WEB.md` | Servidor HTTP e framework web |
| `docs/ASYNC.md` | Modelo de concorrência |
| `docs/DIAGNOSTICS.md` | Catálogo de erros e códigos |
| `docs/BENCHMARKS.md` | Metodologia e baseline de performance |
| `docs/FAQ.md` | Perguntas frequentes |
| `ARCHITECTURE.md` | Arquitetura dos crates e do pipeline |
| `ADRs/` | Decisões arquiteturais registradas |
| `ROADMAP.md` / `CHANGELOG.md` | Planejamento e histórico por release |
| `reports/` | Auditorias e relatórios de engenharia |

## Roadmap

- **0.7.2 (atual):** auditoria de engenharia — robustez do núcleo, HTTP e framework, performance do JSON, REPL e `test` melhores, documentação honesta. Ver [`CHANGELOG.md`](CHANGELOG.md).
- **0.8.0 (planejado):** TLS no MySQL (RFC 0034), builder de strings, `fs_chown`/ACL, prazo por handler HTTP, unificação de `db_exec(SELECT)`, gate de fuzz no CI.
- **Explicitamente fora:** `and`/`or`, operador `in`, truthiness implícita, keep-alive — rejeitados por RFC, com motivo registrado.

Ver [`ROADMAP.md`](ROADMAP.md) (estado) e [`CHANGELOG.md`](CHANGELOG.md) (releases).

## Governança e contribuição

Autoridade máxima: [`GOVERNANCE.md`](GOVERNANCE.md). Antes de qualquer tarefa, leia também [`AGENTS.md`](AGENTS.md). Fluxo completo em [`CONTRIBUTING.md`](CONTRIBUTING.md): branch `feat/...`/`fix/...`, gates (fmt+clippy+test), registro obrigatório em `changes/`, sem `push --force`/`reset --hard`, push/tags/releases só com autorização.

## Licenciamento

`MIT OR Apache-2.0` — ver [`LICENSE-MIT`](LICENSE-MIT) e [`LICENSE-APACHE`](LICENSE-APACHE).
