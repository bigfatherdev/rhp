# [Tooling] — Distribuição Docker oficial (`bigfatherdev/rhp`)

- ID: 2026-11-17-tooling-docker-publish
- Data: 2026-11-17
- Versão: unreleased
- Tipo: Tooling
- Compatibilidade: Compatível (só adiciona arquivos de distribuição e CI;
  nenhum código RHP alterado)

## Resumo

`Dockerfile` multi-stage multi-arch (builder `rust:1.100-bookworm` +
`libsqlite3-dev`; runtime `debian:bookworm-slim` + `libsqlite3-0` +
`ca-certificates`, usuário não-root `rhp`), `.dockerignore`,
`.github/workflows/docker-publish.yml` (gates + build amd64/arm64 +
push condicional + cache + tags semver reais), `docs/DOCKER.md`,
seção no README e ADR 0015.

## Motivação

Pedido: distribuição oficial pelo Docker Hub com CI (build, testes,
multi-arch, publicação), sem credenciais no código e sem dependências
novas no projeto.

## O que mudou

- `Dockerfile` (novo), `.dockerignore` (novo),
  `.github/workflows/docker-publish.yml` (novo),
  `docs/DOCKER.md` (novo), `README.md` (seção Docker + índice),
  `ADRs/0015-distribuicao-docker.md` (novo), `CHANGELOG.md` (entrada).
- Decisões (detalhes na ADR 0015): suíte completa no job `test` do CI
  (com Postgres real via service) em vez de dentro do build da imagem;
  tags da versão real (`latest`, `0.1.0`, `0.1`, `0`; sem `1.x`
  fictícios); push só fora de PR.

## Compatibilidade e migração

Nenhum impacto no runtime/linguagem; migração não necessária.

## Segurança e performance

Auditoria prévia: binário vincula `libsqlite3` do sistema (FFI
preexistente, ADR 0011) — runtime inclui `libsqlite3-0`; resto Rust
puro. Imagem ~100 MB (base 85 + apt 11 + binário 6,5). Usuário não
privilegiado (UID 10001), sem `--privileged`, sem segredos no repo
ou nos logs (login via `secrets.*`, fora de PR não há login).

## Testes e validação

- `docker build -t rhp:local .`: OK (achou e corrigiu 2 gaps reais:
  `include_str!` de `examples/` e `hello.rhp`/`tests/conformance`/`editors/`
  exigidos por testes de integracao — incluidos no contexto).
- Dentro do container: `rhp --version` (= `rhp 0.1.0`); `run` com
  volume montado (sort/json/tipos/funcoes); usuario `uid=10001(rhp)`.
- CI no proprio PR validou o workflow (2 falhas rapidas encontradas e
  corrigidas aqui: toolchain sem `rustfmt`/`clippy` — adicionados
  `components:`; tags vazias em PR — adicionado `type=ref,event=pr`).
- Suite do projeto: 324 passed, 0 failed (inclui 3 testes novos desta
  leva? nao — esta leva nao adiciona testes Rust; o gate roda a suite
  existente como regressao).
- amd64 validado localmente; arm64: build sob QEMU local em andamento
  + garantido no CI do PR (workflow constroi as 2 arquiteturas em todo
  push/PR) — nunca afirmar sem evidencia.

## Limitações e riscos conhecidos

- Publicação real NÃO validada aqui: exige (1) segredos
  `DOCKERHUB_USERNAME`/`DOCKERHUB_TOKEN` criados à mão, (2) repo
  `bigfatherdev/rhp` existente/criado no primeiro push, (3) workflow
  verde no GitHub. Checklist em `docs/DOCKER.md`; entrega não declara
  integração concluída sem esses passos (regra do programa).
- `latest` move a cada push na `main` (documentado; use tags fixas em
  produção).
- QEMU/emulação: builds arm64 lentos; mitigado com cache GHA.

## Referências

- ADR 0015 (`ADRs/0015-distribuicao-docker.md`);
- `docs/DOCKER.md` (instruções completas de operação).
