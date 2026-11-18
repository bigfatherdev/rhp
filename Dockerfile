# RHP — imagem oficial (`bigfatherdev/rhp`)
#
# Multi-stage, multi-arquitetura (linux/amd64, linux/arm64 via QEMU):
#   1. `builder`: compila o binario `rhp` em release + smoke test
#      (`rhp --version`). A SUITE COMPLETA (incl. Postgres real) roda no
#      job `test` do CI, com service container — nao aqui, pois o
#      builder nao tem banco (ver workflow + `docs/DOCKER.md`).
#   2. `runtime`: so o binario + `libsqlite3-0` (o RHP vincula o SQLite
#      do sistema via FFI; ADR 0011) + `ca-certificates`. Sem toolchain
#      Rust, sem codigo-fonte, sem root (usuario dedicado `rhp`, UID
#      10001; nenhum processo roda como root, sem `--privileged`).
#
# Build local (amd64):
#   docker build -t rhp:local .
#   docker run --rm rhp:local --version
#   docker run --rm -v "$PWD:/work" -w /work rhp:local run hello.rhp

# ---------- builder ----------
FROM rust:1.100-trixie AS builder

# Dependencia nativa do link: `libsqlite3` (FFI direto, sem bindgen nem
# vendoring; `gcc` para o `ring`/`rustls`).
# `upgrade` aplica backports de seguranca do Debian (openssl, zlib,
# glibc, util-linux, libsqlite3, etc.) no momento do build — ver
# SECURITY_REMEDIATION.md (CVEs 2026-119373/119374/119379, 2026-76844,
# 2026-85091, 2026-95691, 2026-107493, 2026-53615, 2026-78410,
# 2026-13595, 2026-86805, 2025-6020 e demais). Sem `upgrade`, a imagem
# congelaria versoes vulneraveis da base. Base `trixie` (estavel atual):
# pacotes mais novos que `bookworm` e backports aplicados mais rapido
# (ex.: util-linux e zlib ja corrigidos em `trixie-security`).
RUN apt-get update && apt-get upgrade -y --no-install-recommends \
    && apt-get install -y --no-install-recommends \
    ca-certificates \
    gcc \
    libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src

# Tudo que o build/teste precisa (o `.dockerignore` tira lixo do
# contexto: `target/`, `.git/`, logs). A imagem final leva SOMENTE
# o binario (estagio `runtime` abaixo).
COPY . .

# Cache de compilacao persistente entre builds (BuildKit `cache` +
# `type=gha` no CI). `--locked`: Cargo.lock versionado e autoritativo.
# Smoke test do binario final (`--version`); a suite completa (com
# Postgres real) roda no CI, nao aqui.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked \
    && cp /src/target/release/rhp /usr/local/bin/rhp \
    && /usr/local/bin/rhp --version

# ---------- runtime ----------
FROM debian:trixie-slim AS runtime

RUN apt-get update && apt-get upgrade -y --no-install-recommends \
    && apt-get install -y --no-install-recommends \
    ca-certificates \
    libsqlite3-0 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 rhp \
    && useradd --system --uid 10001 --gid rhp --home-dir /home/rhp --no-log-init --shell /usr/sbin/nologin rhp \
    && mkdir -p /home/rhp /work \
    && chown -R rhp:rhp /home/rhp /work

COPY --from=builder --chown=rhp:rhp /usr/local/bin/rhp /usr/local/bin/rhp

USER rhp
WORKDIR /work
ENTRYPOINT ["rhp"]
CMD ["--help"]
