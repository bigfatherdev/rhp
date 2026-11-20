# [Improved] — Erros de tipo citam os dois argumentos (RHP-AUD-003)

- ID: 2026-11-20-improved-builtin-arg-types
- Data: 2026-11-20
 - Versão: 0.6.1
- Tipo: Changed (mensagens de erro)
- Compatibilidade: Compatível (só texto de diagnóstico; E-codes iguais)

## Resumo

23 builtins de 2–3 args reportavam só o tipo do 1º argumento
(`split("a,b", 1)` dizia "encontrado string"). Agora citam ambos:
`` `split` exige (string, string), encontrado (string, int) ``.

## Motivação

Auditoria stdlib 80/80: diagnóstico enganoso atrasa debug real (§8 missão).

## O que mudou

- `crates/rhp-vm/src/builtins.rs`: `(other, _)` → `(a, b)` (+1 caso
  `(a, b, c)` no `replace`; `fs_write`/`fs_append` compartilhado).
- Precedente interno já existia (HOFs em `rhp-vm/src/lib.rs:2643`).

## Detalhes técnicos

Nenhum teste asseria as strings antigas (verificado por grep). Mensagens
novas seguem o mesmo molde.

## Compatibilidade e migração

Quem faz match de texto exato em stderr precisa atualizar (improvável;
E-codes estáveis preservados). Migração: não necessária.

## Segurança e performance

Sem impacto (caminho de erro).

## Testes e validação

- Novo `rhp_run_reports_both_arg_types` (split + has).
- `cargo test --workspace`: 367 passed, 0 failed; fmt/clippy limpos.

## Limitações e riscos conhecidos

Nenhum.

## Referências

- `audit/full-system/FINDINGS.md` (RHP-AUD-003).
