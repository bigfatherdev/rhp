# ADR 0008 — Verificador de fluxo de pilha e runtime separado (CP4)

- Data: 2026-11-14
- Estado: aceita (missao Next Generation, CP4; sem breaking change)
- Contexto: auditoria NG-202 (validador nao analisava pilha) e organizacao
  do runtime (missao §6).

## Decisão

1. **Analise estatica de altura de pilha em `validate`.** Efeito liquido por
   opcode (`Constant` +1, binarias -1, `Call` -aridade, `BuildArray`
   1-n etc.); juncoes exigem alturas iguais; underflow e erro estatico;
   codigo inalancavel e ignorado pelo fluxo (mas nao pelas checagens
   estruturais). Corpos de `fn` validados com pilha zerada.
2. **`VmLimits` estendido sem mudar padrao.** `max_stack_depth` (checado ao
   fim de cada `step`) e `max_collection_len` (bytes/elementos por valor
   empilhado ou gravado; cobre constantes artesanais, `Build*`, `push`,
   `+` de strings, `StoreIndex`). Padrao = ilimitado.
3. **Runtime separado**: `rhp-vm/src/builtins.rs` (builtins + `http_get`,
   `MAX_HTTP_BODY`) vs nucleo (`Vm::step`, frames, pilha). Sem mudanca de
   comportamento; imports ajustados.

## Alternativas consideradas

- Interpretacao abstrata com tipos de operandos: rejeitada por agora —
  altura resolve underflow/juncao (o pedido da missao); tipos exigiriam
  lattice + testes maiores sem caso de uso concreto ainda.
- Tetos ligados por padrao: rejeitado — mudaria programas validos.
- Medicao de heap real: sem alocador instrumentado no std estavel;
  aproximacao por valor e o teto honesto disponivel.

## Consequências

- Chunks artesanais invalidos (ex.: `Print` isolado) agora falham no
  `validate` em vez de em runtime (mensagem menciona `pilha`).
- `CallBuiltin` teve efeito corrigido para `-aridade+1` (sem callee) —
  antes a analise nao existia; comportamento de runtime inalterado.
- Specs: `BYTECODE_SPEC.md` §5, `VM_SPEC.md` (limites/runtime).
