# RHP

Linguagem experimental com compilador, bytecode, máquina virtual e runtime
próprios em Rust — uma evolução independente do PHP, sem depender do PHP.
Toolchain 100% em Rust stable, sem dependências externas, sem `unsafe`.

> Estado: Fases 1–6 implementadas e verificadas (núcleo imperativo, stdlib,
> módulos, ferramentas e rede mínima). O restante da visão (PostgreSQL/Redis,
> async, classes/generics) está planejado e **não** implementado — ver
> `ROADMAP.md` e `ADRs/0004-fase-6-rede-e-fronteira.md`.

## Requisitos

- Rust stable recente (verificado com 1.100.0), edição 2021
- Zero crates de terceiros (só workspace + std)

## Como compilar, testar e rodar

```bash
cargo build                    # binario em ./target/debug/rhp
cargo test --workspace         # 93 testes, 0 falhas (estado verificado)
cargo fmt --check              # formato
cargo clippy --workspace --all-targets -- -D warnings   # lint

./target/debug/rhp run hello.rhp        # Hello from RHP!
./target/debug/rhp run examples/fib.rhp # 55
./target/debug/rhp fmt [--check] <arquivo.rhp>
```

## CLI (`rhp`)

| Comando | Efeito |
|---|---|
| `rhp run <arquivo.rhp>` | Executa o programa (com `import`s resolvidos); saída 0 ok / 1 erro / 2 uso |
| `rhp fmt <arquivo.rhp>` | Reescreve no formato canônico (idempotente) |
| `rhp fmt --check <arquivo.rhp>` | Falha (exit 1) se fora do formato |
| `rhp --help`, `rhp --version` | Ajuda / versão |

## A linguagem (resumo do implementado)

```rhp
import "mat.rhp";          // modulos: DFS, uma vez, ciclo e falta = erro

let a = [1, 2];            // arrays (valor/copia), mapas {k: v}
a = push(a, 3);            // builtins: len push pop keys str int float type assert
echo a[0] + len(a);        // 1 + 3 = 4

fn fib(n) {                // funcoes, return, recursao (limite 512)
  if n < 2 { return n; }
  return fib(n - 1) + fib(n - 2);
}
echo fib(10);              // 55

let dado = http_get("http://127.0.0.1:8080/x");  // rede minima std-only
```

Regras centrais: tipagem dinâmica com erro explícito em runtime (sem
coerção silenciosa); `if`/`while` exigem `Bool`; aritmética checada
(estouro, `/0`, `%0` = erro); `a[i] = v` em um nível; `//` comentários.
Detalhes normativos em `LANGUAGE_SPEC.md`, `BYTECODE_SPEC.md`, `VM_SPEC.md`.

## Estrutura do repositório

| Crate / pasta | Papel |
|---|---|
| `crates/rhp-lexer` | Tokenizador manual |
| `crates/rhp-parser` | Parser de precedência + AST |
| `crates/rhp-bytecode` | `Value`, `Op`, `Chunk`, `compile()`, builtins |
| `crates/rhp-vm` | VM em pilha (frames, escopos, `http_get`) |
| `crates/rhp-fmt` | Formatador canônico |
| `crates/rhp-cli` | Binário `rhp` (+ loader de módulos) |
| `examples/` | `fib.rhp` (canônico, saída `55`) |
| `tools/` | `check-changes.sh` (CI de auditoria) |
| `ADRs/` | Decisões arquiteturais |
| `changes/` | Histórico auditável (um arquivo por mudança) |

Governança: `GOVERNANCE.md` (autoridade máxima), `AGENTS.md`.
Planejamento: `ROADMAP.md`. Resumo por release: `CHANGELOG.md`.

## Desenvolvimento seguro

```bash
git log --oneline            # 9 commits: Initial + 7 retrospectivos + licença
tools/check-changes.sh       # exige registro em changes/ p/ diffs relevantes
```

Branches `feat/...`/`fix/...`; hook pre-commit roda fmt+clippy+test;
push/tags/releases só com autorização; sem `reset --hard`/`push --force`.

## Licença

`MIT OR Apache-2.0` — ver `LICENSE-MIT` e `LICENSE-APACHE`.
