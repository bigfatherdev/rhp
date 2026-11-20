# [Fixed] — Estouro de pilha do parser com cadeias planas (RHP-AUD-001)

- ID: 2026-11-20-fix-parser-depth-chains
- Data: 2026-11-20
 - Versão: 0.6.1
- Tipo: Fixed (Security)
- Compatibilidade: Compatível (com mudança observável documentada)

## Resumo

Cadeias planas longas (`1+1+...`, `!!!x`, `a?b:c?d...`, `a??b??...`,
`a.b.c`, `a[0][1]`, `f()()()`) não tinham teto e abotavam o processo
(SIGABRT 134) em `run`, `check` e `fmt`. Agora falham com E1000 limpo
(`profundidade maxima excedida ... (limite 64)`), com posição.

## Motivação

Auditoria full-system: crashes determinísticos com input não confiável
(P0). Reproduzido com 2000 elos em 7 formas; `fmt` e `check` também caíam.

## O que mudou

- `Cursor::check_chain` (teto `depth + elos ≤ MAX_PARSE_DEPTH`) nos 9
  laços (8 binários + pós-fixo: membro/índice/chamada).
- `nest()` nas recursões (operador unário, ramos do ternário, `??`).
- `MAX_PARSE_DEPTH` 128→64 (medição com `stack_probe`: 120 ok, 128
  aborta em thread 2 MiB debug; 64 ≈ 2× margem).
- Exemplo `crates/rhp-cli/examples/stack_probe.rs` (sonda 2 MiB;
  ferramenta de regressão do teto).

## Detalhes técnicos

Arquivos: `crates/rhp-parser/src/lib.rs` (helper + 9 laços + 3 recursões +
testes), `LANGUAGE_SPEC.md` §1 (64 + cadeias contam), exemplo novo.
Sem `unsafe`, sem dependências, sem mudança de precedência/associatividade.

## Compatibilidade e migração

Cadeias/aninhamento 65–128 agora E1000 (antes: funcionavam ou abortavam
acima de ~1500). Código real não chega perto; suíte original segue verde.
Teste `accepts_nesting_within_limit` 100→50 (justificado na mensagem).
Migração: não necessária.

## Segurança e performance

Elimina crash com input arbitrário (P0). Custo: O(1) por operador
(contador + comparação); bench release sem regressão atribuível.

## Testes e validação

- `cargo test --workspace`: 367 passed, 0 failed (era 364; +3 regressões).
- Novo `rejects_excessive_flat_chains` (8 vetores falham limpo + curtos passam).
- Novo `rhp_run_rejects_deep_chains_cleanly` (end-to-end, sem abort).
- Fuzz mutacional 2500 casos: 0 crash/hang/panic pós-fix.
- `cargo fmt --check` limpo; `clippy` zero avisos.

## Limitações e riscos conhecidos

Teto 64 medido em debug/2 MiB; revalidar com `stack_probe` se frames de
parse ou `sizeof(Expr)` crescerem.

## Referências

- `audit/full-system/FINDINGS.md` (RHP-AUD-001), `FIXES.md`, `SECURITY.md`.
