# FUZZING.md — Descoberta de falhas (time-boxed)

- **Mutacional de fonte** (`/tmp/audit/fuzz.py`, artefato de sessão):
  2500 casos (rodada anterior, pré-fix) + 1000 casos (novas seeds:
  classes, geradores, async, HOFs, try) = **3500 casos, 0 suspeitos**
  (pós-fix; inclui vetores RHP-AUD-001 como sementes implícitas).
  Oráculo: sinal fatal (134/139), `panicked`, `stack overflow`, timeout 5 s.
- **Internos (mantidos)**: lexer (bytes aleatórios), parser (tokens),
  bytecode hostil — todos `never-panic`, verdes na suíte.
- **Minimização**: IND-001 reduzido de programa de 30 linhas para 8
  (`m08`); IND-002/003 reduzidos a 2 comandos (erro + comando seguinte).

Recomendação: versionar `fuzz.py` como `tools/fuzz-smoke.sh` (P4).
