# Security — garantias, limites e reporte

## Garantido e testado (nao apenas alegado)

- **Sem `unsafe`** em todo o workspace (verificado por inspecao; proibido
  na Fase 1 e mantido desde entao).
- **Bytecode validado antes de executar**, recursivamente e com **analise
  estatica de fluxo de pilha** (underflow e juncoes divergentes viram erro
  de validacao, nao de runtime); bytecode e tratado como entrada
  nao confiavel.
- **Sem panics em entradas nao confiaveis**: lexer/parser/compiler/VM
  retornam erros; `unwrap()`/`expect()` so em testes. Excecao conhecida e
  corrigida: aninhamento profundo agora e erro `profundidade maxima`
  (limite 128), nao abort (ver `reports/AUDIT_MASTER.md`, AUD-001).
- **Aritmetica inteira checada** (estouro, `/0`, `%0`, `int()` estrito nos
  limites, sem saturacao silenciosa).
- **Limites com teste**: recursao 512 frames, corpo HTTP 1 MiB, fonte
  8 MiB/arquivo, `VmLimits` opt-in (`max_steps` via `--max-steps`,
  `max_stack_depth`, `max_collection_len`).
- **Imports confinados**: subarvore do programa, nivel superior, sem ciclos.
- **Rede minima honesta**: so `http://`, timeouts de 5s, so 2xx, corpo
  UTF-8; DNS usa o resolvedor do sistema (sem timeout proprio — documentado).
- **Diagnosticos auditaveis**: erros com codigo estavel (`E0xxx`–`E3xxx`) e
  `rhp check [--json]` para validar sem executar.

## Explicitamente NAO garantido

- Isolamento/sandbox de SO: programas RHP rodam com seus privilegios.
- `import` le arquivos locais (confinado a arvore do programa; sem rede).
- Sem TLS no servidor, limite de taxa ou autenticacao; cliente HTTPS
  verificado (raizes embutidas + `RHP_CA_BUNDLE`); redirects so via
  `http_fetch` (teto de 10, sem downgrade https->http).
- `VmLimits` e opt-in (padrao ilimitado); teto de heap em bytes do modelo
  via `max_heap_bytes`/`--max-heap-bytes` (RFC 0015; nao mede o alocador —
  teto rigido de RSS = cgroup/ulimit).
- Sem auditoria externa, fuzz continuo ou testes em Windows.
- `rand()`/`rand_range()` NAO sao criptograficos (xorshift64; so para
  sorteio/jogos). Segredos, tokens e CSRF exigem CSPRNG — ainda ausente
  (sem `random_bytes` nesta leva). Para senhas, use `hash_password`/
  `verify_password` (Argon2id/bcrypt).

## Reporte

Encontrou vulnerabilidade? Abra issue com `[security]` + PoC minimo
(programa `.rhp` ou comando) e o comportamento esperado. Nao publique
exploit antes da correcao.
