# SECURITY_AUDIT.md — Resistência a entradas (independente)

## Confirmados e corrigidos

- **IND-001 (P0)**: corrupção de estado via HOF+erro — erro fantasma com
  código errado (`E3004` para erro de JSON!) ou valor silenciosamente
  errado (`[7]` intacto só após o fix; antes: `[7, ...]` corrompido ou
  500 indevido). Qualquer programa com `map/filter/usort` + `try/catch`
  + chamadas posteriores estava exposto. Corrigido e com regressão que
  falha sem o fix.
- **IND-002/003 (P1)**: dessincronia de conexão DB após erro — sucesso
  falso + erro atrasado (pg) / `lenenc truncado` (mysql). Corrigidos com
  dreno limitado; regressões que falham sem o fix.

## Revalidados (sem problema)

Teto 64 (2 MiB sem abort), JSON 128, `json_stringify` 128, valores 300 em
2 MiB (Display/str/==/async), percent-decode, CL estrito, TE, headers 64K,
prazo 30 s, unlink de `pending` em unwind (auditado: demais flags morrem
com o frame por construção), fuzz 3500 sem crash, secrets sweep limpo,
TLS real, int/float overflow limpos, `import` confinado, arquivo >8 MiB
rejeitado.

## Riscos remanescentes

`pending_gen` em cenários exóticos de unwind (sem evidência — sem mudança);
O(n²) como vetor de DoS pelo próprio programa (aceito); FS sem sandbox
(desenho documentado).
