# [Added] — Datas/horas std-only: `now`, `sleep`, `date_str`

- ID: 2026-11-15-feat-datas-horas
- Data: 2026-11-15
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (só adiciona builtins; ids 40–42)

## Resumo

`now()` (epoch em segundos, relógio do SO), `sleep(s)` (bloqueante,
teto de 1h) e `date_str(ts)` (epoch => ISO UTC `"AAAA-MM-DDTHH:MM:SSZ"`,
algoritmo civil próprio, sem timezones).

## Motivação

Fase A4 do plano de sistemas: sem relógio não há log, expiração,
agendamento nem medição — pré-requisito de servidor e pacotes.

## O que mudou

- `rhp-bytecode`: variantes `Now`/`Sleep`/`DateStr`, nomes, aridades
  (0/1/1), ids 40–42, anti-deriva estendido.
- `rhp-vm/src/time.rs` (novo): `SystemTime` para `now`, `thread::sleep`
  para `sleep`, civil-from-days proléptico para `date_str`.
- Gramática VSCode + teste de consistência (3 nomes).
- Specs: `LANGUAGE_SPEC.md` §6, `BYTECODE_SPEC.md` §2/§4, exemplo em
  `docs/LANGUAGE_REFERENCE.md`.

## Detalhes técnicos

`sleep` fracionário via `Duration::from_secs_f64` (sub-segundo real).
`date_str` usa `div_euclid`/`rem_euclid` (timestamps negativos corretos:
1969, 1900, bissextos 2000-02-29 verificados). Resultado passa por
`push_checked` (teto de coleção respeitado).

## Compatibilidade e migração

Compatível. Migração: não necessária. Sem cronologia além de UTC, sem
parse de datas, sem timezone/DST (fora do escopo declarado).

## Segurança e performance

Sem `unsafe`, sem I/O, sem deps. `sleep` bloqueia a thread (VM é
single-thread; documentado). Teto de 1h evita travamento acidental
(ex.: `sleep(999999999)` vira erro, não hang de CI).

## Testes e validação

- Falha-antes: builtins inexistiam (auditoria marcava AUSENTE).
- Vetores de `date_str` conferidos contra Python (`datetime`, UTC):
  epoch 0/-1, 2000-02-29, 9999-12-31, 1900-01-01.
- `cargo test --workspace`: 191 passed, 0 failed (188 antes + 3 novos:
  `runs_time_builtins`, `sleep_zero_returns_quickly`, `rhp_run_time_builtins`).
- `cargo fmt --check` ok; `cargo clippy --workspace --all-targets
  -D warnings` ok; `tools/check-changes.sh origin/main` ok.

## Limitações e riscos conhecidos

UTC sempre; sem parsing (`"2026-01-01"` => data), sem meses/dias
relativos, sem timezone. `now()` usa relógio de parede (pode andar para
trás em ajuste; sem garantia monotônica).

## Referências

Auditoria `reports/PHP_PARITY_AUDIT.md` §11 (datas, M). Sem ADR
(extensão dentro do desenho dos builtins).
