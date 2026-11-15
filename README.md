<div align="center">

# RHP

**Uma linguagem de programação independente, com compilador, bytecode, máquina virtual e runtime próprios em Rust.**

Familiar como o PHP na sintaxe. Própria na semântica, no compilador e na máquina virtual.

</div>

[![Linguagem Rust](https://img.shields.io/badge/linguagem-Rust_2021-orange)](https://www.rust-lang.org)
[![Licença](https://img.shields.io/badge/licen%C3%A7a-MIT_OR_Apache--2.0-blue)](LICENSE-MIT)
[![Zero dependências](https://img.shields.io/badge/depend%C3%AAncias-zero-success)](#)
[![Testes](https://img.shields.io/badge/testes-133_verdes-success)](#testes-e-validacoes)

> **Estado:** núcleo imperativo, stdlib, módulos, formatador, diagnósticos e rede mínima implementados e testados. PostgreSQL/Redis, async, classes e generics seguem **planejados** — ver [Roadmap](#roadmap).

## Índice

- [A linguagem em 60 segundos](#a-linguagem-em-60-segundos)
- [Características principais](#características-principais)
- [Documentação](#documentação)
- [Instalação](#instalação)
- [Primeiro programa](#primeiro-programa)
- [Tour pela sintaxe](#tour-pela-sintaxe)
- [CLI](#cli)
- [Testes e validações](#testes-e-validacoes)
- [Arquitetura](#arquitetura)
- [Estrutura do projeto](#estrutura-do-projeto)
- [Segurança](#segurança)
- [Roadmap](#roadmap)
- [Governança e contribuição](#governança-e-contribuição)
- [Licenciamento](#licenciamento)

## A linguagem em 60 segundos

```rhp
// Fibonacci iterativo: tipos, variaveis, controle e funcoes.
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

```bash
cargo build
./target/debug/rhp run examples/fib.rhp
# 55
```

## Características principais

- **Sintaxe familiar, semântica própria** — `echo`, `fn`, `if`/`else`, `while`, com tipagem dinâmica e erros explícitos em runtime (sem coerção silenciosa).
- **Compilador de verdade** — lexer e parser escritos à mão, AST própria, geração de bytecode com pool de constantes e validação estática (incluindo análise de fluxo de pilha).
- **VM em pilha** — frames explícitos, escopos léxicos, recursão limitada, builtins e `http_get` std-only.
- **Biblioteca-padrão nascente** — arrays e mapas por valor, `len push pop keys str int float type assert`, `import` com resolução DFS.
- **Ferramentas próprias** — formatador canônico (`rhp fmt`), diagnósticos com códigos estáveis (`E0001`–`E3011`), verificação sem execução (`rhp check [--json]`), gramática para VSCode.
- **Zero dependências externas, sem `unsafe`** — só workspace + biblioteca padrão do Rust.

## Documentação

| Documento | O quê |
|---|---|
| `docs/GETTING_STARTED.md` | RHP em 5 minutos |
| `docs/LANGUAGE_REFERENCE.md` | Guia da linguagem com exemplos |
| `LANGUAGE_SPEC.md` | Contrato normativo da linguagem |
| `BYTECODE_SPEC.md` / `VM_SPEC.md` | Contrato do bytecode e da VM |
| `docs/CLI_REFERENCE.md` | Referência completa da CLI |
| `docs/DIAGNOSTICS.md` | Catálogo de erros e códigos |
| `docs/BENCHMARKS.md` | Metodologia e baseline de performance |
| `docs/FAQ.md` | Perguntas frequentes |
| `ARCHITECTURE.md` | Arquitetura dos crates e do pipeline |
| `ADRs/` | Decisões arquiteturais registradas |
| `ROADMAP.md` / `CHANGELOG.md` | Planejamento e histórico por release |
| `reports/` | Auditorias e relatórios de engenharia |

## Instalação

Pré-requisitos: [toolchain Rust stable](https://rustup.rs) recente (verificado com 1.100.0). Nenhuma dependência externa.

```bash
git clone https://github.com/bigfatherdev/rhp.git
cd rhp
cargo build                    # binario em ./target/debug/rhp
```

## Primeiro programa

```bash
printf 'echo "Hello from RHP!";\n' > hello.rhp
./target/debug/rhp run hello.rhp
# Hello from RHP!
```

## Tour pela sintaxe

```rhp
// Variaveis, tipos e echo de qualquer valor.
let nome = "rhp";
let versao = 0.1;
echo nome + " " + str(versao); // "rhp 0.1"

// Aritmetica checada e comparacoes (tipos mistos invalidos sao erro).
echo 1 + 2 * 3;      // 7
echo 10 % 3 == 1;    // true

// Arrays e mapas por valor (sem aliasing).
let a = [1, 2];
a = push(a, 3);
echo len(a);         // 3
a[0] = 9;
echo a;              // [9, 2, 3]
echo keys({b: 1, a: 2}); // [a, b]

// Controle de fluxo (condicao sempre Bool, sem truthiness implicita).
let i = 0;
while i < 3 {
  if i == 1 {
    echo "um";
  } else {
    echo i;
  }
  i = i + 1;
}

// Funcoes com retorno e recursao (limite de 512 frames).
fn fat(n) {
  if n <= 1 {
    return 1;
  }
  return n * fat(n - 1);
}
echo fat(5); // 120

// Modulos: cada arquivo executa uma vez, antes do importador.
import "mat.rhp";
echo dobro(21); // 42 (se mat.rhp define fn dobro(x) { return x * 2; })
```

Regras que valem sempre: `;` termina instrucoes; `//` comenta ate o fim da linha; `x = …` exige `let` previo; `a[i] = v` so em um nivel; chaves de mapa duplicadas vencem a **primeira**; `nan == nan` e `false` (IEEE 754); limite de 128 niveis de aninhamento. Detalhes normativos em [`LANGUAGE_SPEC.md`](LANGUAGE_SPEC.md).

## CLI

| Comando | Efeito |
|---|---|
| `rhp run [--max-steps N] <arquivo.rhp>` | Executa (resolve `import`s); exit 0 ok / 1 erro / 2 uso invalido |
| `rhp fmt [--check] <arquivo.rhp>` | Formata no estilo canonico (idempotente); `--check` so verifica |
| `rhp check [--json] <arquivo.rhp>` | Lex + imports + compilacao **sem executar**; `--json` emite relatorio maquina-legivel |
| `rhp --help`, `rhp --version` | Ajuda e versao |

Erros saem como diagnosticos estruturados, com codigo estavel, trecho da fonte e sugestao:

```text
Error[E1004]: 1:13: esperado `;`, encontrado fim de arquivo
  --> app.rhp:1:13
1 | echo "x"
  |             ^
help: toda instrucao termina com `;`
```

## Testes e validações

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace          # 133 testes, 0 falhas
cargo build --workspace
./target/debug/rhp run examples/fib.rhp          # 55
./target/debug/rhp fmt --check hello.rhp examples/*.rhp
tools/check-changes.sh          # exige registro em changes/ para diffs relevantes
cargo run --release -p rhp-cli --example bench_rhp  # benchmarks std-only
```

Cobertura: lexer, parser, bytecode (inclui validação de fluxo de pilha), VM, builtins, módulos, CLI, formatador, diagnósticos, rede contra servidor local, corpus malformado (nunca entra em pânico), property test determinístico e verificação de exemplos.

## Arquitetura

```text
fonte --loader--> tokens --parser--> AST --compile--> Chunk --vm.run--> saida
rhp-lexer -> rhp-parser -> rhp-bytecode -> rhp-vm -> rhp-cli (+rhp-fmt, +rhp-diagnostic)
```

`Chunk { constants, names, funcs, code }` validado (pools, saltos, `Halt`, fluxo de pilha) **antes** de executar; frames explícitos com limite de 512; `VmLimits` opt-in (`max_steps`, `max_stack_depth`, `max_collection_len`); runtime isolado em `rhp-vm/src/builtins.rs`. Detalhes em [`ARCHITECTURE.md`](ARCHITECTURE.md) e `ADRs/`.

## Estrutura do projeto

| Caminho | Papel |
|---|---|
| `crates/rhp-*` | Lexer, parser/AST, bytecode, VM, formatador, diagnósticos, CLI |
| `examples/` | Programas `.rhp` versionados (todos executados em teste) |
| `editors/vscode/` | Gramática TextMate + configuracao da linguagem |
| `tools/` | `check-changes.sh` (CI de auditoria) |
| `reports/` | Auditorias e relatórios de engenharia |
| `docs/` | Guias (getting started, CLI, diagnósticos, benchmarks, FAQ…) |
| `changes/` | Histórico auditável (um arquivo por mudança) |

## Segurança

Garantido e testado: sem `unsafe`; bytecode validado antes de executar; aritmética inteira checada; profundidade sintática limitada (128); imports confinados à árvore do programa com teto de 8 MiB/arquivo; `http_get` só `http://`, timeouts de 5 s, teto de corpo de 1 MiB; `VmLimits` opt-in. Explicitamente **não** garantido: isolamento de SO, HTTPS/redirects, heap sem teto medido, DNS com timeout próprio. Detalhes e reporte em [`SECURITY.md`](SECURITY.md).

## Roadmap

- **Concluído:** fundação, núcleo imperativo, stdlib (arrays/mapas/builtins), módulos, ferramentas (`fmt`, CI), rede mínima, diagnósticos, memória documentada, verificador de fluxo, `rhp check`, gramática VSCode.
- **Planejado (requer aprovação + RFC onde indicado):** tipagem estática, exports/gerenciador de pacotes, PostgreSQL/Redis com servidores reais, async/concorrência, classes/generics, LSP completo, debugger, otimizações com protocolo antes/depois.

Ver [`ROADMAP.md`](ROADMAP.md) (estado) e [`CHANGELOG.md`](CHANGELOG.md) (releases).

## Governança e contribuição

Autoridade máxima: [`GOVERNANCE.md`](GOVERNANCE.md). Antes de qualquer tarefa, leia também [`AGENTS.md`](AGENTS.md). Fluxo completo em [`CONTRIBUTING.md`](CONTRIBUTING.md): branch `feat/...`/`fix/...`, gates (fmt+clippy+test), registro obrigatório em `changes/`, sem `push --force`/`reset --hard`, push/tags/releases só com autorização.

## Licenciamento

`MIT OR Apache-2.0` — ver [`LICENSE-MIT`](LICENSE-MIT) e [`LICENSE-APACHE`](LICENSE-APACHE).
