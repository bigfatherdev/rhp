# ADR 0015 — Distribuição Docker oficial (`bigfatherdev/rhp`)

- Data: 2026-11-17
- Estado: aceita (implementada: `Dockerfile`, workflow, docs)
- Contexto: o RHP só era distribuído como fonte + `cargo build`. Pedido:
  imagem oficial Docker Hub com publicacao automatica via CI, sem
  credenciais no codigo e sem dependencias novas no projeto.

## Decisao

1. **Multi-stage, base Debian (`rust:1.100-bookworm` builder,
   `debian:bookworm-slim` runtime).** Alternativas descartadas:
   imagem Rust unica (toolchain+fontes vazariam p/ runtime);
   `distroless`/`alpine` (musl exigiria recompilar `libsqlite3`
   estatico ou trocar a FFI; `glibc` + `.so` do sistema e o caminho
   suportado pela ADR 0011; shell ausente piora o debug).
2. **Runtime minimo mas funcional:** so o binario + `libsqlite3-0`
   (link dinamico real, verificado com `ldd`) + `ca-certificates`
   (~100 MB; binario ~6,5 MB). Sem `--privileged`, sem `HEALTHCHECK`
   (CLI, nao daemon), usuario dedicado `rhp` (UID 10001), `ENTRYPOINT
   ["rhp"]` + `CMD ["--help"]`.
3. **Testes no CI, nao na imagem:** a suite completa (incl. Postgres
   real via service container) roda no job `test`; a imagem so compila
   + smoke (`rhp --version`). Motivo: o builder nao tem banco; exigir
   testes la dentro ou quebraria o build ou exigiria subir Postgres no
   `Dockerfile` (frágil, lento, fora do padrao).
4. **Tags da versao real (nunca ficticias):** `docker/metadata-action`
   com `type=semver` sobre tags `v*` (`v0.1.0` => `0.1.0`, `0.1`, `0`;
   `latest` so na `main`). Quando sair a 1.0.0 real, `1.0.0`/`1.0`/`1`
   passam a existir sem mudar o pipeline. (Conflito inicial: o pedido
   citava `1.0.0` com o projeto em `0.1.0`; resolvido com o solicitante
   optando pelas tags reais.)
5. **Push condicional + cache:** `push` so fora de PR (PRs constroem e
   validam); cache BuildKit (`type=gha`) + mounts de registry/target;
   `--locked` (Cargo.lock autoritativo); QEMU via `setup-qemu-action`
   (padrao, runners efemeros).
6. **Sem segredos no repo:** `DOCKERHUB_USERNAME`/`DOCKERHUB_TOKEN`
   ( Account Settings > Security; Read+Write bastam) criados a mao;
   login condicional (sem segredos = sem push, sem falha).

## Alternativas consideradas

- Registry GitHub (ghcr.io): descartado por ora (pedido explicito foi
  Docker Hub; o workflow e adaptavel com 2 linhas).
- `cargo-chef` para cache: descartado (mounts de cache do BuildKit +
  `type=gha` cobrem; uma ferramenta a menos para auditar).
- Buildx local com QEMU em vez de CI: validacao complementar feita
  (amd64 integral; arm64 em andamento — ver registro), mas a
  publicacao oficial e sempre pelo CI (reprodutibilidade + segredos).

## Emenda 2026-11-18 — base `bookworm` → `trixie` (aprovada pelo solicitante)

- `rust:1.100-bookworm` → `rust:1.100-trixie` (tag verificada como ativa
  no Docker Hub); `debian:bookworm-slim` → `debian:trixie-slim`.
- Motivo: `trixie` e o estavel atual — pacotes mais novos e backports de
  seguranca aplicados mais rapido (ex.: util-linux e zlib ja corrigidos
  em `trixie-security`; ver `SECURITY_REMEDIATION.md`). Limite honesto:
  o SQLite do `trixie` (3.46.1) continua `< 3.54.0`, logo as CVEs FTS5
  seguem MITIGADAS, nao corrigidas, ate backport do Debian.
- Sem mudanca de arquitetura: multi-stage, runtime minimo, usuario
  `rhp`, `apt-get upgrade` nos dois estagios (da entrega anterior).

## Consequencias

- Novo CI (`.github/workflows/docker-publish.yml`) + `Dockerfile` +
  `.dockerignore` + `docs/DOCKER.md` + secao no README.
- Rust do CI pinado em 1.100 (igual a imagem e ao toolchain verificado
  localmente); atualizar juntos quando o MSRV mudar.
- Dependencia operacional: segredos + repo Docker Hub criados a mao
  (checklist em `docs/DOCKER.md`; entrega nao afirma publicacao real
  sem eles — ver `changes/`).
