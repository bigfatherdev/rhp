# [Added] — Fase 6: rede mínima (`http_get`) e fronteira da visão

- ID: 2026-11-14-feat-fase-6-rede-minima
- Data: 2026-11-14
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (novo builtin `http_get`, id 9; resto intacto)

## Resumo

Builtin `http_get(url)` std-only (somente `http://`, timeouts 5s, só 2xx,
corpo UTF-8), verificado contra servidor local. PostgreSQL/Redis,
async/concorrência, classes/generics, exports e métodos ficam planejados e
NÃO implementados (fronteira honesta em `ADRs/0004` + `ROADMAP.md`).

## Motivação

Fechar a última fase executável da visão com prova real de I/O, sem alegar o
que não foi validado. Desenho e fronteira em `ADRs/0004`.

## O que mudou

- `rhp-bytecode`: `Builtin::HttpGet` (id 9), nome, aridade, validação.
- `rhp-vm`: cliente HTTP/1.0 mínimo sobre `TcpStream` (parse de
  `http://host[:porta]/caminho`, `connect_timeout`/leitura 5s, split de
  cabeçalhos, status 2xx, corpo UTF-8); erros como `VmError`.
- Specs (`LANGUAGE_SPEC` §6, `VM_SPEC` §3) + `ARCHITECTURE.md`/`ROADMAP.md`/
  `CHANGELOG.md` atualizados.
- Zero dependências externas em todas as fases (só workspace + std).

## Detalhes técnicos

Sem redirect automático, sem `https://`, sem IPv6 literal (erros explícitos).
`http_get` de não-string, esquema, host vazio, porta inválida, DNS, conexão,
status não-2xx e corpo não-UTF-8 são todos erros com mensagem.

## Compatibilidade e migração

Compatível. Migração: não necessária.

## Segurança e performance

Timeouts evitam travamento; resposta lida integralmente (sem limite de
tamanho — documentado como limitação); sem TLS (só `http://`).
Performance não medida.

## Testes e validação

`cargo fmt --check` ok; `cargo clippy ... -D warnings` ok;
`cargo test --workspace`: 93 testes, 0 falhas (bytecode 17, CLI unit 6,
integração 10 — inclui `http_get` contra `TcpListener` local servindo
`ola http` —, fmt 6, lexer 12, parser 20, VM 22 — inclui 3 erros de rede).
`hello.rhp` → `Hello from RHP!`; `fib.rhp` → `55`; ambos `--check` canônicos.

## Limitações e riscos conhecidos

Sem limite de tamanho de resposta; sem HTTPS/redirect/IPv6; sem testes com
rede externa (só loopback). PG/Redis/async/classes não implementados por
exigirem servidores, runtime ou RFC (ver ADR 0004). Diretório sem Git.

## Referências

- ADR: `ADRs/0004-fase-6-rede-e-fronteira.md`. Autorização: "até a última fase".
