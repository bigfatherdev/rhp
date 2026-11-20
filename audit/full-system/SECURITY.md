# SECURITY.md — Auditoria de segurança (evidências, sem hipótese)

## Confirmado e tratado (P0)

- **RHP-AUD-001 — Stack overflow via fonte**: `1+1+...` (2000), `!`×2000,
  `?:`×500, `??`×2000, `.x`×2000, `[0]`×2000, `()`×2000 abortavam o processo
  (SIGABRT 134) em `run`, `check` e `fmt`. **Corrigido** (teto 64 uniforme +
  `nest`); 2500 casos fuzz pós-fix sem crash; regressão permanente.
  Severidade pré-fix: **P0** (crash determinístico com input não confiável).

## Confirmado, por desenho (sem ação de código)

- **FS sem sandbox** (Info): `/etc/passwd` legível, escrita em `/tmp` ok.
  `SECURITY.md:28` declara privilégios do usuário. Ação: nenhuma (não
  prometer sandbox que não existe).
- **`unwrap`/`expect` em produção**: só invariantes checadas (`location`
  após `contains_key`, chaves de mapas próprios); fuzz + suíte sem panics.
- **Imports confinados** à árvore + sem rede; ciclos detectados; >8 MiB
  rejeitado (verificado).
- **HTTP**: tetos (corpo 1 MiB, headers 64 KiB, prazo 30 s, passos/heap por
  request) verificados na 0.6.0; percent-decode e CL estritos (0.6.0).
- **Secrets**: sweep em código/docs/examples — limpo.
- **TLS cliente**: `https://` funciona (teste real example.com 200).

## Erros limpos verificados (anti-DoS)

Overflow int, float/0, `sqrt(-1)`, `pop([])`, divisão por zero, profundidade
JSON 128, `json_stringify` 128, validação de bytecode artesanal (fuzz
hostil existente), arg inválido em todos os 80 builtins (sem panic).

## Risco residual

- Teto 64 medido em debug/2 MiB; release tem folga maior. Revalidar se
  `sizeof(Expr)` ou frames de parse crescerem (usar `stack_probe`).
- O(n²) `push`/`+=` permite DoS de CPU/memória com input gigante **do
  próprio programa** (não de rede) — aceito por semântica.
