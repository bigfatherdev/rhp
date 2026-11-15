# RHP

**RHP** e uma linguagem de programacao experimental com compilador, bytecode,
maquina virtual e runtime proprios em **Rust stable** — uma evolucao
independente do PHP, sem depender do PHP. Zero crates de terceiros, sem
`unsafe`.

## Caracteristicas principais

- Sintaxe familiar ao PHP (`echo`, `fn`, `if`/`else`, `while`), semantica propria
- Tipagem dinamica com erros explicitos em runtime (sem coercao silenciosa)
- Funcoes com recursao, arrays e mapas por valor, builtins (`len`, `push`, `http_get`…)
- Modulos com `import` (DFS, sem ciclos), formatador canonico (`rhp fmt`)
- VM em pilha com validacao de bytecode, limites opt-in e aritmetica checada

## Estado real de desenvolvimento

Marco inicial e Fases 2–6 implementadas e verificadas: nucleo imperativo,
stdlib (`Array`/`Map`/builtins), modulos, ferramentas (`fmt`, CI de
`changes/`), rede minima (`http_get` std-only). Auditoria integral em
`reports/AUDIT_MASTER.md` (P0/P1 corrigidos no Checkpoint 3).

**Planejado e NAO implementado**: PostgreSQL/Redis, async/concorrencia,
classes/generics, exports/privacidade, HTTPS. Ver `ROADMAP.md`.

## Requisitos

- Rust stable recente (verificado com 1.100.0), edicao 2021
- Nenhuma dependencia externa (só workspace + std)

## Instalacao e compilacao

```bash
git clone https://github.com/bigfatherdev/rhp.git
cd rhp
cargo build                    # binario em ./target/debug/rhp
```

## Primeiro programa

```bash
echo 'echo "Hello from RHP!";' > hello.rhp
./target/debug/rhp run hello.rhp   # Hello from RHP!
```

## Sintaxe (resumo)

```rhp
import "mat.rhp";

let a = [1, 2];                // arrays heterogeneos por valor
let m = {nome: "rhp"};         // mapas (chaves ordenadas no Display)
echo a[0] + len(a);            // 1 + 2

fn fib(n) {                    // funcoes, return, recursao (limite 512)
  if n < 2 { return n; }
  return fib(n - 1) + fib(n - 2);
}
echo fib(10);                  // 55
```

Detalhes normativos: `LANGUAGE_SPEC.md`. Referencia rapida: `docs/LANGUAGE_REFERENCE.md`.

## Recursos implementados

Expressoes (`+ - * / %`, comparacoes, `!`/`-` unarios, `+x`), literais
(`int`/`float`/`string` com `\u{...}`/`bool`/`null`), `let`/atribuicao
(inclusive `a[i] = v`), blocos/escopos, `if`/`else`, `while`, `fn`/`return`/
chamadas, `echo` de qualquer valor, comentarios `//`, builtins
(`len push pop keys str int float type assert http_get`), `import` com
jaula na arvore do programa, `rhp fmt`, limites (`--max-steps`, teto HTTP
1 MiB, profundidade sintatica 128).

## Arquitetura resumida

```text
fonte --loader--> tokens --parser--> AST --compile--> Chunk --vm--> saida
rhp-lexer -> rhp-parser -> rhp-bytecode -> rhp-vm -> rhp-cli (+rhp-fmt)
```

Detalhes: `ARCHITECTURE.md` e ADRs em `ADRs/`.

## Estrutura do projeto

| Caminho | Papel |
|---|---|
| `crates/rhp-*` | Lexer, parser/AST, bytecode, VM, formatador, CLI |
| `examples/` | Programas `.rhp` versionados (todos executados em teste) |
| `tools/` | `check-changes.sh` (CI de auditoria) |
| `reports/` | `AUDIT_MASTER.md`, `FINAL_ENGINEERING_REPORT.md` |
| `docs/` | Guias (getting started, CLI, diagnosticos, benchmarks, FAQ) |
| `changes/` | Historico auditavel (um arquivo por mudanca) |

## Testes e validacoes

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace        # 120+ testes, 0 falhas (estado verificado)
./target/debug/rhp run examples/fib.rhp   # 55
tools/check-changes.sh        # exige registro em changes/
```

Cobertura: lexer/parser/bytecode/VM/fmt/CLI/rede (servidor local),
propriedade aritmetica (gerador deterministico), corpus malformado
(sem panico), exemplos, limites e validacao de bytecode.

## Seguranca e limitacoes conhecidas

Garantido e testado: sem `unsafe`; bytecode validado antes de executar;
aritmetica inteira checada; profundidade sintatica 128; recursao 512;
HTTP com timeout 5s e teto 1 MiB; imports confinados + teto de fonte 8 MiB;
DNS sem timeout proprio (documentado). **Nao** ha sandbox/OS-level:
programas RHP rodam com seus privilegios; sem HTTPS/redirect; `VmLimits`
e opt-in. Detalhes: `SECURITY.md`.

## Roadmap

Fases concluidas em `ROADMAP.md`/`CHANGELOG.md`. Proximo (requer aprovacao):
P2s restantes de baixo risco, tipagem/RFC, modulos com exports, PG/Redis com
servidores reais, async (runtime), classes/generics, LSP.

## Governanca e contribuicao

Autoridade maxima: `GOVERNANCE.md`. Leia tambem `AGENTS.md` antes de
qualquer tarefa. Fluxo completo: `CONTRIBUTING.md`. Mudancas sem `changes/`
sao rejeitadas na CI (`tools/check-changes.sh`).

## Licenciamento

`MIT OR Apache-2.0` — ver `LICENSE-MIT` e `LICENSE-APACHE`.
