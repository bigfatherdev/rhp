# Getting Started — RHP em 5 minutos

## 1. Instalar a toolchain

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## 2. Clonar e compilar

```bash
git clone https://github.com/bigfatherdev/rhp.git
cd rhp
cargo build
```

## 3. Primeiro programa

```bash
printf 'echo "Hello from RHP!";\n' > hello.rhp
./target/debug/rhp run hello.rhp
# Hello from RHP!
```

## 4. Explorar a linguagem

```bash
./target/debug/rhp run examples/fib.rhp   # 55
./target/debug/rhp fmt --check hello.rhp  # formato canonico
```

Brinque com `docs/LANGUAGE_REFERENCE.md` ao lado: cada secao tem exemplos
copiaveis. Erros comuns (mensagem + o que fazer): `docs/DIAGNOSTICS.md`.

## 5. Rodar os testes

```bash
cargo test --workspace      # suite completa
cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings
```

## Proximos passos

- Como contribuir: `CONTRIBUTING.md`
- Garantias e limites: `SECURITY.md`
- Medir desempenho: `docs/BENCHMARKS.md`
- Perguntas frequentes: `docs/FAQ.md`
