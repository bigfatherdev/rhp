# [Fixed] — HTTP: 400s corretos, limites de protocolo e robustez do accept

- ID: 2026-11-20-fix-http-protocolo
- Data: 2026-11-20
- Versão: 0.6.0
- Tipo: Fixed
- Compatibilidade: Compatível (status de erro mais correto)

## Resumo

Requests malformados devolvem `400 Bad Request` (antes: 500). Teto de
cabecalhos (64 KiB) aplicado à seção de headers, `Content-Length`
estrito, corpo incompleto rejeitado, prazo total de leitura (30 s),
`accept` transiente não para o servidor, percent-decode correto em
UTF-8 e `reason()` para 400/429 (+408/413/422/502/504 no registro
`2026-11-20-feat-http-chunked-localhost.md`).
(`chunked`: rejeitado neste registro; IMPLEMENTADO em seguida — ver
`changes/2026-11-20-feat-http-chunked-localhost.md`, que prevalece.)

## Motivação

Auditoria do ciclo HTTP (missão 0.6.0 §5): 500 para erro do cliente
mascarava bugs; seção de headers podia crescer além do teto antes do
`MAX_HEADERS+MAX_BODY` combinado; `Content-Length` inválido era tratado
como 0 (corpo silenciosamente ignorado); EOF antes do corpo declarado era
aceito truncado; `chunked` produzia corpo errado sem aviso; `%C3%A9`
virava mojibake; erro de `accept` encerrava o servidor silenciosamente.

## O que mudou

- `respond_400` novo; `read_request`: teto `header_end > MAX_HEADERS`,
  `Content-Length` com parse estrito (`usize`; inválido = 400), rejeição
  de `Transfer-Encoding` não-`identity`, corpo curto = 400, deadline
  total (`Instant` + `IO_TIMEOUT_SECS`) nas fases de header e corpo.
- Accept-loop: erro não-`WouldBlock` = log + sono 100 ms + continua.
- `percent_decode`: acumula bytes + `from_utf8_lossy` (antes: `as char`).
- `reason()`: 400 `Bad Request`, 429 `Too Many Requests` (antes caíam em `Desconhecido`).

## Detalhes técnicos

Arquivo: `crates/rhp-vm/src/http_server.rs`. Sem mudança de API pública;
mapa `Value` do `req` inalterado. Casos 400 contam em `max_requisicoes`
como antes (conexão atendida).

## Compatibilidade e migração

Clientes corretos não observam diferença. Observável: erro de protocolo
agora é 400 (era 500);
query/path UTF-8 agora decodifica certo (era latin1-mojibake). Migração:
não necessária (clientes devem tratar 400 como erro deles — padrão HTTP).
(`chunked`: ver registro `2026-11-20-feat-http-chunked-localhost.md`.)

## Segurança e performance

Superfície de DoS reduzida: headers limitados, corpo limitado (1 MiB),
prazo total anti-gotejamento, sem alocação extra no caminho quente
(parse continua em 1 passada + cópia única).

## Testes e validação

- Integração: `400s_e_sobrevive` (4 casos + servidor segue), `cabecalhos_grandes_e_utf8` (70 KiB → 400; `caf%C3%A9` → `café`).
- `cargo test --workspace`: 354 passed, 0 failed; fmt/clippy limpos.

## Limitações e riscos conhecidos

Sem keep-alive (fecha por request); `chunked` implementado em registro
próprio; HTTP/1.x apenas;
`absolute-URI` passa adiante sem normalizar (origin-form é o caso dos proxies).

## Referências

- Missão 0.6.0 §5; `docs/WEB.md`; branch `feat/rhp-0.6.0`.
