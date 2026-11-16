# [Added] — Pacotes v0: `rhp init`, `rhp pkg`, `rhp.lock`

- ID: 2026-11-15-feat-pacotes
- Data: 2026-11-15
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (só adiciona comandos; loader inalterado)

## Resumo

Gerenciador mínimo local: `rhp init [dir]` cria `rhp.toml`;
`rhp pkg add <nome> <caminho>` registra dependência;
`rhp pkg install [dir] [--check]` vendoriza em `pacotes/` + grava/verifica
`rhp.lock` (hash FNV-1a por arquivo). Sem rede, sem registro, sem
resolução de versões (declarado como v0).

## Motivação

Fase D (pacotes v0): distribuir e travar dependências locais com
auditoria, sem introduzir rede nem dependências externas novas.

## O que mudou

- `crates/rhp-cli/src/pkg.rs` (novo): manifesto em subconjunto estrito
  de TOML (parser/recusa claros), lockfile com `[[pacote]]`, cópia
  recursiva (pula `.git`), verificação por hash, 4 testes unitários.
- CLI: `init` (+`--help`/USAGE), `pkg add|install [--check]`; loader
  inalterado (`import "pacotes/x/..."` relativo já funcionava).
- Docs: `docs/CLI_REFERENCE.md` (seção pacotes).

## Compatibilidade e migração

Aditivo. `rhp.toml` opcional (projetos sem manifesto continuam iguais).

## Segurança e performance

Sem `unsafe`, sem rede, sem deps. Lockfile detecta adulteração
acidental (FNV-1a; não é proteção anti-adversário — documentado).
`--check` para CI.

## Testes e validação

- Falha-antes: comandos inexistiam.
- Ciclo completo e2e: init → add → install → run usa pacote → `--check`
  ok → adultera → `--check` falha (exit 1).
- `cargo test --workspace`: 216 passed, 0 failed (inclui 4 testes de
  unidade + 1 e2e de pacotes).

## Limitações e riscos conhecidos

Sem versionamento/resolução (dependência = caminho local); sem
transitivas automáticas (copia o que existe, inclui `pacotes/` aninhados
como estão); validação é por conteúdo, não assinatura. Reexport e
registro central seguem futuros (reabrem discussão).

## Referências

Fase D, item "pacotes v0". ADR 0011 (contexto de dependências).
