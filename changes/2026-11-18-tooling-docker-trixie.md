# Tooling — Base Docker `bookworm` → `trixie`

- ID: 2026-11-18-tooling-docker-trixie
- Data: 2026-11-18
- Versão: unreleased
- Tipo: Tooling
- Compatibilidade: Compatível

## Resumo

Imagem Docker migra da base `bookworm` para `trixie` (estável atual):
`rust:1.100-bookworm` → `rust:1.100-trixie`,
`debian:bookworm-slim` → `debian:trixie-slim`. Pacotes mais novos e
backports de segurança aplicados mais rápido. Nenhuma mudança no código
Rust, na semântica da linguagem ou no contrato da imagem (mesmo usuário
`rhp`, mesmo `ENTRYPOINT`, mesmos pacotes instalados).

## Motivação

Auditoria das 20 CVEs mostrou que `bookworm` recebe backports mais
lentamente; `trixie-security` já corrigiu util-linux e zlib, por exemplo.
Migração aprovada pelo solicitante como passo para a correção total.
Limite honesto: o SQLite do `trixie` (3.46.1) continua `< 3.54.0` —
as CVEs FTS5 seguem MITIGADAS (ver `SECURITY_REMEDIATION.md`).

## O que mudou

- `Dockerfile`: 2 linhas `FROM` + comentário da base.
- `docs/DOCKER.md`: seção de arquitetura (nomes das bases + nota do
  `upgrade` e da migração).
- `ADRs/0015-distribuicao-docker.md`: emenda 2026-11-18 (decisão original
  preservada, migração registrada com motivo e limite).
- `SECURITY_REMEDIATION.md`: bloco da correção + seção de atualização.

## Detalhes técnicos

- Tag `rust:1.100-trixie` verificada como ativa no Docker Hub
  (API `v2/repositories/library/rust/tags/1.100-trixie`, 2026-11-18).
- Tripla de verificação Debian (security-tracker, 2026-11-18): sqlite3
  `bookworm` 3.40.1 vulnerável; `trixie` 3.46.1 vulnerável; fix só em
  `forky`/`sid` 3.54.1 (bug #1152790). Ou seja: a migração melhora a
  postura e acelera futuros DSAs, mas não fabrica o patch SQLite.
- `apt-get upgrade` nos dois estágios foi mantido.

## Compatibilidade e migração

Compatível. Mesma arquitetura multi-stage, mesmo runtime mínimo, mesmo
UID 10001, mesmos pacotes. Rebuild da imagem no CI aplica tudo; nenhuma
ação do usuário da imagem. Não altera matriz de compatibilidade da
linguagem/bytecode/VM.

## Segurança e performance

- Segurança: reduz a janela de exposição a DSAs (bases mais novas).
- Performance: sem impacto no runtime do RHP (nenhum caminho quente
  alterado); custo restrito ao tempo de build. Sem medição antes/depois
  nesta sessão (sem Docker daemon); benchmarks de `docs/BENCHMARKS.md`
  podem ser reexecutados no CI.

## Testes e validação

- `cargo test --workspace`: verde (0 falhas).
- `cargo clippy --workspace --all-targets -- -D warnings`: verde.
- `cargo fmt --all -- --check`: verde.
- Build Docker + scanner de imagem: **não executados** (sem Docker daemon
  no ambiente) — obrigatórios no CI (`docker-publish.yml`) antes do
  release; PRs constroem sem push, por desenho.

## Limitações e riscos conhecidos

- SQLite FTS5 continua MITIGADO até DSA do Debian (bug #1152790).
- Tamanho final da imagem pode variar alguns MB (bases diferentes);
  valor `~100 MB` em `docs/DOCKER.md` mantido como ordem de grandeza,
  sem novo número inventado.

## Referências

- ADR 0015 (+ emenda 2026-11-18).
- `SECURITY_REMEDIATION.md` (atualização 2026-11-18).
- Docker Hub API: tag `rust:1.100-trixie` ativa.
- Debian security-tracker: `CVE-2026-119374` (bug #1152790).
