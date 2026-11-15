# FINAL_ENGINEERING_REPORT.md — Missão de engenharia do RHP

- Data: 2026-11-14 · Ramo: `feat/engineering-audit` (publicado após aprovação)
- Toolchain: rustc/cargo 1.100.0 · Zero crates externas · Sem `unsafe`
- Comandos de aceite executados no HEAD final (resultados reais abaixo)

## 1. Estado inicial

Repositório verde e sincronizado com `origin/main`: fmt/clippy ok, **93
testes, 0 falhas**, `hello.rhp` => `Hello from RHP!`, `fib.rhp` => `55`,
exemplos canônicos. Devedor conhecido: auditoria nunca feita; P0/P1 latentes.

## 2. Problemas encontrados

Auditoria integral em `reports/AUDIT_MASTER.md`: **1 P0, 5 P1, 13 P2, 7 P3,
7 P4**, todos com arquivo/linha, causa, impacto, solução, dependências e
testes necessários + evidências reproduzidas em `/tmp` (binário do HEAD,
repo intacto).

## 3. Problemas corrigidos

| ID | Correção | Prova |
|---|---|---|
| AUD-001 (P0) | Teto de profundidade 128 no parser; `validate()` iterativo com teto de constantes | 50k parênteses: `exit=134` => erro limpo `exit=1` |
| AUD-101 | fmt: floats sem notação científica | `0.000001` reparseia; 2 testes novos |
| AUD-102 | fmt: chaves de mapa citadas quando preciso | `{"a b": 1}` reparseia |
| AUD-103 | Escape `\u{HEX}` no lexer + spec | 6 malformados rejeitados |
| AUD-104 | `MAX_HTTP_BODY` 1 MiB com leitura em blocos | servidor de 2 MiB => erro limpo |
| AUD-105 | DNS sem timeout documentado + teste `.invalid` | falha rápida, sem hang |
| AUD-202/204/303 | Params dup (erro), `int()` estrito, `+x` identidade | falha-antes em todos |
| AUD-206/207 + teto fonte | Jaula na árvore, 8 MiB, import só no topo | 3 testes novos |
| AUD-205 | `VmLimits::max_steps` opt-in + `--max-steps` | loop infinito contido em teste |
| AUD-201/203/208/209 | Spec pinada + testes (1ª chave vence, NaN, shadowing, builtins) | pinos verdes |
| AUD-210–213 | Harness bench + baseline, corpus/property/exemplos, CI renames | 120 testes no total |

## 4. Melhorias documentais

README nos 15 itens da missão; `docs/` (GETTING_STARTED,
LANGUAGE_REFERENCE sem duplicar specs, CLI_REFERENCE, DIAGNOSTICS real,
BENCHMARKS, FAQ); `CONTRIBUTING.md`; `SECURITY.md` (garantias testadas +
limites honestos); `docs/ARCHITECTURE.md` e `docs/SECURITY.md` omitidos de
propósito (raiz já cobre). `LANGUAGE_SPEC`/`VM_SPEC` atualizadas a cada
mudança semântica.

## 5–8. Compilador, VM, runtime, segurança

Sem reescrita: correções cirúrgicas com teste falha-antes/passa-depois.
Segurança: elimina DoS por profundidade e corpo HTTP; jaula de imports;
documenta DNS; `VmLimits` opt-in preserva compatibilidade. Diagnósticos:
catálogo real em `docs/DIAGNOSTICS.md` (códigos `E0001` formais ficam como
P4/ADR futura — sem prometer o não implementado).

## 9. Testes

93 => **120 testes, 0 falhas** (bytecode 20, CLI unit 10, integração 13,
fmt 8, lexer 15, parser 25, VM 29). Novos: regressões P0/P1, pins P2,
corpus malformado (nunca panic), property aritmética (LCG, 200 casos),
verificação de exemplos, limites, DNS, teto HTTP.

## 10. Benchmarks

Harness std-only (`--example bench_rhp`, release): lex 0,873 ms · parse
9,207 ms · compile 10,247 ms (85 KB) · fib(20) 99,328 ms · while 200k
370,917 ms · push 20k 10465,838 ms. **Nenhuma otimização aplicada**:
sem necessidade provada; `push` O(n²) é semântica documentada (mudar =
breaking + RFC). Metodologia em `docs/BENCHMARKS.md`.

## 11–14. Arquivos/ADRs/registros

- Criados: `reports/` (2), `docs/` (6), `CONTRIBUTING.md`, `SECURITY.md`,
  `crates/rhp-cli/examples/bench_rhp.rs`, 5 `changes/`, ADRs 0005/0006.
- Modificados: 6 crates (cirúrgico), specs, `ROADMAP.md`, `CHANGELOG.md`.
- ADRs: 0005 (limites de robustez), 0006 (limites de execução).
- `changes/`: auditoria + 4 (CP3) + 5 (P2/limites/robustez/bench/docs).

## 15. Testes executados e resultados

```bash
cargo fmt --check                              # ok
cargo clippy --workspace --all-targets -- -D warnings   # ok, 0 warnings
cargo test --workspace                         # 120 passed, 0 failed
cargo build --workspace                        # ok
./target/debug/rhp run hello.rhp               # Hello from RHP!
./target/debug/rhp run examples/fib.rhp        # 55
./target/debug/rhp fmt --check hello.rhp examples/*.rhp  # ok
tools/check-changes.sh                         # cenários manuais ok
cargo run --release -p rhp-cli --example bench_rhp      # tabela acima
```

## 16–19. Pendências, riscos, próximas prioridades, roadmap

- **Não resolvido**: P2 de baixo risco restantes? Todos os P2 listados foram
  resolvidos ou pinados, exceto trabalho futuro explícito (P4).
- **Riscos restantes**: pilha de operandos/heap sem teto geral; DNS sem
  timeout; sem limite de taxa HTTP; propriedade do PRNG cobre só `+`/`*`;
  números de bench valem para a máquina citada.
- **Próximas prioridades**: RFC de tipos; exports/gerenciador de pacotes;
  PG/Redis com servidores reais; runtime async; classes/generics; LSP;
  otimizações com protocolo antes/depois.
- **Roadmap**: `ROADMAP.md` reflete CP1–CP3 + docs como concluídos e o
  restante como planejado (nada alegado como pronto).

## Tabela de estado

| Item | Estado | Evidência |
|---|---|---|
| README | Concluído | Arquivo + comandos conferidos no binário |
| Documentação | Concluído | 6 docs + CONTRIBUTING + SECURITY; specs atualizadas |
| Compiler | Concluído | Profundidade, params dup, `+x`, `\u`, imports; 25+20 testes |
| VM | Concluído | Teto HTTP, DNS doc, `int`, `VmLimits`; 29 testes |
| Runtime | Concluído | Builtins inalterados salvo conversões estritas; suite verde |
| Diagnósticos | Concluído (catálogo) | `docs/DIAGNOSTICS.md` real; códigos formais = P4 futuro |
| Segurança | Concluído (escopo) | P0/P1 fechados; `SECURITY.md` honesto; sem `unsafe` |
| Performance | Baseline concluído | Harness + tabela; sem otimização (decisão registrada) |
| Compatibilidade | Concluído | Só falha o que abria abort/era bug silencioso; specs + pins |
| CI | Concluído | Hook + `check-changes.sh` (renames, fetch-depth doc) + exemplos |
