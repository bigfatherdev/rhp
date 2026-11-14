# ADR 0004 — Rede mínima e fronteira honesta (Fase 6)

- Data: 2026-11-14
- Estado: aceita (autorização "até a última fase" + execução verificada)

## Contexto

A visão (§3 do prompt) inclui HTTP, PostgreSQL e Redis. Restrições reais:
std-only (sem crates externas nesta fase), tudo validado por testes executados
de verdade neste ambiente.

## Decisão

1. **`http_get(url)` std-only.** Cliente HTTP/1.0 mínimo sobre `TcpStream`
   (somente esquema `http://`, timeouts de 5s, só 2xx, corpo UTF-8).
   Verificado contra servidor local de teste (loopback) em integração.
2. **Fronteira explícita, sem alegação.** PostgreSQL e Redis exigem servidores
   e/ou protocolos/clients fora do std; async/concorrência exige decisão de
   runtime (executor/threads no nível da linguagem); classes/generics exigem
   RFC de tipos. Nada disso é implementado nem alegado: ficam em `ROADMAP.md`
   como planejado/não-especificado, com este ADR como justificativa auditável.
3. **Sem dependências externas.** Nenhum crate de terceiros foi adicionado em
   nenhuma fase (auditoria: `cargo tree` = só workspace + std).

## Alternativas consideradas

- Implementar cliente Redis (RESP) contra stub próprio: rejeitado por agora —
  testaria o cliente contra implementação própria do protocolo, prova fraca;
  volta quando houver servidor real em CI.
- Handshake PostgreSQL à mão: rejeitado — complexidade e superfície de erro
  incompatíveis com "etapas pequenas e verificáveis" sem servidor de teste.
- `async`/`spawn` com threads: rejeitado — `Value` com `Rc` não é `Send`;
  exige redesenho (ADR/RFC próprios).

## Consequências

- Rede real e verificável no núcleo (`http_get`); resto da visão permanece
  plano honesto, não produto.
