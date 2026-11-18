# Docker — imagem oficial `bigfatherdev/rhp`

Imagem multi-arquitetura (`linux/amd64`, `linux/arm64`) publicada
automaticamente pelo CI (`.github/workflows/docker-publish.yml`).

## Baixar

```bash
docker pull bigfatherdev/rhp:latest
```

Versão fixada (recomendado para produção):

```bash
docker pull bigfatherdev/rhp:0.3.0
```

## Executar

`rhp --version` (sanidade):

```bash
docker run --rm bigfatherdev/rhp:latest --version
# rhp 0.3.0
```

Programa local (monte o diretório como volume; o `WORKDIR` é `/work`):

```bash
docker run --rm -v "$PWD:/work" bigfatherdev/rhp:latest run hello.rhp
```

REPL interativo (terminal alocado):

```bash
docker run --rm -it bigfatherdev/rhp:latest repl
```

Qualquer comando da CLI funciona como argumento (o `ENTRYPOINT` é
`rhp`; o `CMD` padrao e `--help`):

```bash
docker run --rm bigfatherdev/rhp:latest fmt --check app.rhp
docker run --rm -v "$PWD:/work" bigfatherdev/rhp:latest test .
```

O container roda como usuario nao privilegiado (`rhp`, UID 10001).
Arquivos criados via volume pertencem ao UID 10001 no host (use
`--user $(id -u):$(id -g)` se precisar do seu UID — por sua conta).

## Tags disponíveis

| Tag | Significado |
|---|---|
| `latest` | `main` atual (sempre com CI verde) |
| `0.3.0` | Release exata (imutavel) |
| `0.3` | Ultimo `0.3.x` |
| `0` | Ultimo `0.x` |

Tags `X.Y.Z`/`X.Y`/`X` sao geradas automaticamente da tag git
(`v0.3.0` => `0.3.0`, `0.3`, `0`); `latest` acompanha a `main`.
Quando sair a `1.0.0` real, `1.0.0`/`1.0`/`1` passam a existir sem
mudar o pipeline.

## Publicar uma nova versao

1. Atualize `CHANGELOG.md` (secao da versao) e os `version = "..."` dos
   `crates/*/Cargo.toml` (mantidos sincronizados).
2. Crie e envie a tag (o `v` inicial e removido nas tags Docker):
   ```bash
   git tag v0.3.0 && git push origin v0.3.0
   ```
3. O workflow `docker-publish` (gatilho: tags `v*`): roda fmt + clippy +
   testes (com Postgres de servico), depois build amd64+arm64 e push
   (`0.3.0`, `0.3`, `0`; conferir no Docker Hub).

> `latest` acompanha a `main` (qualquer push nela republica `latest`,
> sempre com CI verde). Versoes fixadas (`0.3.0` etc.) so mudam com a
> tag correspondente — use-as em producao.

## Secrets do GitHub (obrigatorio, manual)

Criados uma vez em Settings > Secrets and variables > Actions
(nunca no codigo):

| Nome | Valor |
|---|---|
| `DOCKERHUB_USERNAME` | Usuario do Docker Hub (ex.: `bigfatherdev`) |
| `DOCKERHUB_TOKEN` | Token de acesso (Account Settings > Security > New Access Token; permissoes Read & Write bastam) |

Sem esses segredos, o job de push e pulado (o login e condicional);
PRs de forks nunca recebem segredos (o workflow so constroi, nao
publica — por desenho).

## Repositorio Docker Hub

`bigfatherdev/rhp`: criado automaticamente no primeiro `push` com
sucesso (conta com permissao de escrita). Apos criar, confira a
visibilidade (publico) na pagina do repositorio.

## Arquitetura da imagem

- `builder` (`rust:1.100-trixie` + `gcc` + `libsqlite3-dev`):
  `cargo build --release --locked` + smoke (`rhp --version`).
  A suite completa roda no job `test` do CI (com Postgres real),
  nao dentro do build da imagem.
- `runtime` (`debian:trixie-slim` + `libsqlite3-0` — o RHP vincula
  o SQLite do sistema via FFI — + `ca-certificates`): so o binario
  (`/usr/local/bin/rhp`); usuario `rhp` (UID 10001); `ENTRYPOINT`
  `["rhp"]`, `CMD ["--help"]`. Sem toolchain, sem fonte, sem root,
  sem `--privileged`, sem `HEALTHCHECK` (CLI, nao daemon).
  Ambos os estagios executam `apt-get upgrade` no build para aplicar
  os backports de seguranca do Debian (ver `SECURITY_REMEDIATION.md`).
  Base `trixie` (estavel atual) desde 2026-11-18 (antes: `bookworm`;
  ver ADR 0015, emenda).

~100 MB (base 85 + apt 11 + binario ~6,5).
