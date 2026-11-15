# Contributing — como contribuir com o RHP

## 0. Leia antes de tudo

1. `GOVERNANCE.md` (autoridade maxima), `AGENTS.md`, specs/ADRs da area.
2. `git status`, diff, log. Classifique a mudanca (bug, refatoracao,
   recurso compativel, breaking change — §3 de `GOVERNANCE.md`).

## 1. Fluxo de trabalho

- Branch por tarefa: `feat/...`, `fix/...`, `docs/...`, `test/...`, `perf/...`.
- Mudancas pequenas e verificaveis; sem reescritas gerais sem ADR.
- Commits pequenos com proposito; mensagens claras no imperativo.

## 2. Gates obrigatorios (antes de cada commit)

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
./target/debug/rhp fmt --check hello.rhp examples/*.rhp
tools/check-changes.sh
```

O hook pre-commit roda os tres primeiros e bloqueia se falharem.

## 3. Registro e documentacao

- Toda mudanca incorporada exige `changes/YYYY-MM-DD-<tipo>-<slug>.md`
  (modelo em `changes/README.md`); use a data real.
- Mudanca semantica => atualize a spec + exemplo + teste.
- Decisao arquitetural duradoura => ADR em `ADRs/`.
- Releases/tags/push: **somente com autorizacao expressa**.
- Nunca: `reset --hard`, `push --force`, `clean -fd`, `unwrap()` em
  producao, `unsafe`, crates externas sem aprovacao, enfraquecer testes,
  alegar o nao implementado.

## 4. Testes

- Correcao acompanha teste de regressao (falha-antes/passa-depois).
- Novos recursos: testes positivos + negativos + erro com posicao.
- Fuzz/property com harness deterministico (sem crates externas).
- Benchmarks: `docs/BENCHMARKS.md` antes/depois, mesma maquina/modo.

## 5. Revisao

Diff revisado, escopo respeitado, limitacoes declaradas, sem codigo morto.
Em duvida sobre escopo ou decisao: pare e pergunte (nao assuma).
