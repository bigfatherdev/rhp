# ADR 0006 — Limites de execucao e confinamento (Checkpoint 3+)

- Data: 2026-11-14
- Estado: aceita (missao de engenharia; sem breaking change)
- Contexto: auditoria AUD-205/206/207 e missao Fase 5 (limites configuraveis)

## Contexto

Loops infinitos, crescimento descontrolado e imports fora da arvore nao
tinham nenhum teto alem do limite de recursao. Qualquer limite padrao
mudaria comportamento de programas validos — inaceitavel sem aprovacao.

## Decisão

1. **`VmLimits` opt-in, padrao ilimitado.** `max_steps: Option<u64>` conta
   instrucoes de todos os frames; excedeu => `VmError` limpo. CLI:
   `rhp run --max-steps N`. Sem limites: comportamento identico ao anterior.
2. **Jaula de imports.** `load_program` canonicaliza a entrada e exige
   `starts_with` na subarvore dela (`..` para fora = erro dedicado).
3. **Teto de fonte: 8 MiB por arquivo** (`MAX_SOURCE_BYTES`), checado por
   metadata antes de ler (erro dedicado, sem OOM).
4. **`import` so no nivel superior.** Aninhados em `fn`/blocos viram erro
   com instrucao de correcao (antes eram icados silenciosamente).

## Alternativas consideradas

- Limites padrao ligados: rejeitado — mudaria programas validos sem
  aprovacao (viola compatibilidade controlada).
- Jaula por lista de permissao configuravel: adiado — arvore do programa
  cobre o caso real; lista exigiria formato de config + spec (futuro).
- `VmLimits` com teto de memoria medido: adiado — sem alocador instrumentado
  no std estavel de forma portatil; passos + tetos de entrada cobrem os
  vetores praticos (loop, corpo HTTP, fonte, profundidade).

## Consequências

- `VM_SPEC.md` (limites), `LANGUAGE_SPEC.md` (jaula, topo, tetos),
  `docs/CLI_REFERENCE.md` (`--max-steps`).
- Testes: loop com limite, fora-da-arvore, arquivo grande, import aninhado.
- `push 20k`-style O(n²) segue permitido (semantica por valor, ADR 0002);
  limites de passos permitem conte-lo quando desejado.
