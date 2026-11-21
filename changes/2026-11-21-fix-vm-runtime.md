# Fixed — VM: exibição com teto, `next` com limite, HOF, `catch` sem teto

- ID: 2026-11-21-fix-vm-runtime
- Data: 2026-11-21
- Versão: 0.7.2
- Tipo: Fixed
- Compatibilidade: Compatível

## Resumo
`echo`/`str()`/`throw` de valor com 128+ níveis abortava o processo
(SIGABRT, sem `catch` possível); `next` furava `MAX_CALL_DEPTH`;
`map`/`filter`/`usort` aceitavam callbacks nativas (erro interno `Halt`)
ou `async` (execução síncrona indevida); `catch` tipado errava além de
256 níveis de herança; closures furavam `max_heap_bytes`.

## Motivação
Auditoria 0.7.2: VM-001/SEC-001 (P0/P1), VM-003 (P1), VM-004/VM-005 (P2),
VM-006 (P2), VM-007 (P3).

## O que mudou
- `Value::fmt_limited` (teto 128, recursão limitada) + `to_display_string`
  estrito: `Op::Print`, `str()`, `Throw`, `uncaught_message`, template
  devolvem erro E3000 capturável; `Display` restante trunca com marcador.
- `gen_resume` checa `MAX_CALL_DEPTH` antes de tomar o frame (gerador segue utilizável).
- `hof_begin` rejeita `native_id` e `is_async` com erro claro.
- `is_subclass_of` (VM + compilador): sem teto 256; detecção de revisita
  (cadeia válida é acíclica; ciclo artesanal = `false` fail-safe).
- `Op::MakeFunc` via `push_checked` (env entra na contabilidade de heap).

## Detalhes técnicos
`crates/rhp-bytecode/src/lib.rs` (`fmt_limited`, `to_display_string`,
`is_subclass_of` do compilador); `crates/rhp-vm/src/lib.rs` (`Print`,
`Throw`, `uncaught_message`, `gen_resume`, `hof_begin`, `MakeFunc`,
`is_subclass_of`); `builtins.rs` (`str`, template).

## Compatibilidade e migração
Compatível. Só muda o que antes abortava/enganava: valor profundo =
E3000; `next` além de 512 = E3006; callbacks inválidas = erro claro;
hierarquia 256+ agora casa. Não necessária migração.

## Segurança e performance
Elimina abort por entrada RHP (DoS/500-em-massa via `http_serve`);
sem custo mensurável no caminho comum (1 contador por nível).

## Testes e validação
- `rhp_run_deep_value_display_is_catchable_error`, `rhp_run_gen_resume_respects_call_depth`,
  `rhp_run_hof_rejects_native_and_async_callbacks`, `rhp_run_typed_catch_deep_hierarchy`: ok.
- `cargo test --workspace`: 395 passed, 0 failed; fuzz 1000 limpo.

## Limitações e riscos conhecidos
Truncamento com marcador só no `Display` de backstop (caminhos de
mensagem de erro internos); comportamento determinístico e documentado.

## Referências
VM-001–VM-007 (auditoria 0.7.2); `MEMORY_MODEL.md`; RFCs 0003/0018/0024.
