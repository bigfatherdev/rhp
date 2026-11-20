# LIMITATIONS.md — Limitações reais (reproduzidas ou verificadas)

## Linguagem (P2 → futuras, exigem RFC)

- **Sem `&&`/`||`/`and`/`or`** (RHP-AUD-002): só `!` unário; `&`/`|` rejeitam
  `Bool`. Workaround: `if` aninhado/ternário. Custo: gramática + short-circuit
  na VM. Sem risco de compat (adição pura).
- **Sem pertinência em array** (RHP-AUD-004): `has` é mapa-only; sem `in`.
  Workaround: loop. Custo: sobrecarga de `contains` (decisão RFC).
- **Teto sintático 64** (RHP-AUD-012): intencionado pós-medição; cadeias
  >64 e aninhamento >64 = E1000. Sem workaround necessário (código real
  não chega perto).

## Runtime (P2/P3)

- **Bool SQLite/MySQL volta `Int`** (RHP-AUD-005): inerente aos bancos;
  documentado. Sem fix de código (mapear 0/1→bool corromperia inteiros).
- **O(n²) em `push`/`+=`** (RHP-AUD-011): semântica por valor (ADR 0002);
  builder seria breaking. Documentado.
- **Valores profundos**: runtime suporta 300+ em 2 MiB (Display/str/==/async);
  `json_stringify` capa em 128 com erro limpo.

## APIs (P2 → futuras)

- **MySQL `caching_sha2_password`** (RHP-AUD-009): só `mysql_native_password`.
  Workaround: usuário native ou `--default-authentication-plugin`.
  Custo: troca de auth completa (fast_auth + sha256).
- **`http_serve`**: sem keep-alive, sem HTTPS, corpo só `Str`, trailers
  descartados, handler sem closure, recompile por request (herdado 0.6.0).

## Infraestrutura/desenvolvimento

- FS sem sandbox (por desenho, `SECURITY.md:28`); `env()` expõe ambiente
  (por desenho); sem `and`/`or` afeta legibilidade de condições.
- Fuzzers internos cobrem lexer/parser/bytecode; fuzz de fonte (2500 casos)
  foi artefato de sessão (`/tmp/audit/fuzz.py`) — considerar versionar.

## O que NÃO é limitação (verificado e correto)

Aridade em compilação (E2002); `int(3.9)` erro (documentado);
`sort` 1-arg + `usort`; ciclo de import; arquivo >8 MiB; `exit`/argv;
bool PG round-trip; transações PG; tipos MySQL; erros RESP→RHP.
