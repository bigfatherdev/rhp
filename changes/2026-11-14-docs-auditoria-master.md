# [Docs] — CHECKPOINT 1: auditoria integral (`reports/AUDIT_MASTER.md`)

- ID: 2026-11-14-docs-auditoria-master
- Data: 2026-11-14
- Versão: unreleased
- Tipo: Docs
- Compatibilidade: Compatível (só adiciona `reports/`; nenhum código alterado)

## Resumo

Auditoria integral do RHP (arquitetura, correção, segurança, qualidade):
linha de base verde + 1 achado P0, 5 P1, 13 P2, 10 P3 e 7 P4, cada um com
arquivo/local, causa, impacto, solução, dependências e testes necessários.

## Motivação

Missão de engenharia, CHECKPOINT 1 (Fase Zero): nenhuma refatoração antes da
auditoria. Ramo `feat/engineering-audit`, sem push.

## O que mudou

- `reports/AUDIT_MASTER.md` (novo): metodologia, linha de base, mapa do
  sistema, achados P0–P4 com evidências (comandos + saídas reais), conformité
  com a governança e priorização para os checkpoints 2–5.

## Detalhes técnicos

Principais evidências (sondas em `/tmp`, binário do HEAD, repo intacto):
stack overflow com 50k parênteses (exit 134); `rhp fmt` gerando `1e-6`,
`{a b: 1}` e `"\u{1}"` irreparáveis; `{a: 1, a: 2}` => `{a: 1}`;
`int(2^63)` => satura; `nan == nan` => `false`; `+5` rejeitado;
`import "/etc/hostname"` falha limpo; 500k `echo` em ~7,7 s.

## Compatibilidade e migração

Compatível. Migração: não necessária.

## Segurança e performance

Nenhuma mudança de comportamento (só leitura + relatório). Achados de
segurança registrados como P0/P1/P2 para os checkpoints 3–4.

## Testes e validação

- `cargo fmt --check`: ok. `cargo clippy ... -D warnings`: ok.
- `cargo test --workspace`: 93 testes, 0 falhas (17+6+10+6+12+20+22).
- `hello.rhp` => `Hello from RHP!`; `fib.rhp` => `55`; exemplos canônicos.
- `cargo tree`: zero externas; `grep unsafe`: vazio; `unwrap/expect` só testes.

## Limitações e riscos conhecidos

Auditoria não executa rede externa nem plataforma Windows. P0/P1 pendem de
correção nos checkpoints 3–4 (com teste falha-antes/passa-depois).

## Referências

Missão de engenharia, CHECKPOINT 1. Sem RFC/ADR novo (auditoria não muda
semântica). Ramo: `feat/engineering-audit` (não publicado).
