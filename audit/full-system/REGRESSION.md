# REGRESSION.md — Resultados e reauditoria

## Suíte

- Baseline (main 0.6.0): 364 passed, 0 failed.
- Final (branch `audit/full-system`): **367 passed, 0 failed**
  (+`rejects_excessive_flat_chains`, +`rhp_run_rejects_deep_chains_cleanly`,
  +`rhp_run_reports_both_arg_types`; parser 52→53, integração 79→81).
- `cargo fmt --all -- --check` limpo; `clippy --workspace --all-targets`
  zero avisos (pós todas as edições).

## Reauditoria das mudanças (olhar adversarial)

1. `check_chain` em 9 laços: erro só quando `depth+elos > 64`; laços de
   vírgula/elementos (map/array/args) não contam elos (largura, não
   profundidade) — correto.
2. `nest()` em unário/ternário/`??`: balanceado (+1/-1); ramos curtos
   passam (testes); sem mudança de precedência/associatividade (só teto).
3. Teto 128→64: único teste ajustado foi o de 100 níveis (para 50, com
   justificativa na mensagem); `rejects_*` (400) seguem falhando limpo.
4. `self.seq = 0`: afeta só `command()` (COM_QUERY/COM_QUIT); handshake
   (seq 1) intacto; mock cobre query+exec+erro.
5. Mensagens type: 23 sites; formato `(a, b)`; sem asserção antiga quebrada
   (grep na suíte); E-codes inalterados (só texto após `encontrado`).
6. `stack_probe` (exemplo novo): só usa API pública; compila em `cargo test`;
   sem impacto em build normal.
7. Docs: REFERENCIA `db_query` (bool), LANGUAGE_SPEC §1 (64 + cadeias),
   sem contradição com BYTECODE_SPEC (128 de *constantes* é outro limite,
   validado por código próprio — não tocado).

## Não introduzido

Nenhum `unsafe`, nenhuma dependência, nenhuma API pública alterada em
assinatura, nenhum comportamento de programa válido <64 níveis alterado
(suíte 364/364 original segue verde dentro dos 367).
