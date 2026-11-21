# Security — garantias, limites e reporte

## Garantido e testado (nao apenas alegado)

- **`unsafe` isolado no FFI SQLite** (`sqlite.rs`, com invariantes
  documentadas e verificadas: `CString` antes do FFI, null-checks,
  `Drop` em `Connection`/`StmtGuard`, `!Send`): sem alcance por entrada
  RHP (ver `audit/` 0.7.2, SEC-008). Resto do workspace sem `unsafe`.
- **Bytecode validado antes de executar**, recursivamente e com **analise
  estatica de fluxo de pilha** (underflow e juncoes divergentes viram erro
  de validacao, nao de runtime); bytecode e tratado como entrada
  nao confiavel.
- **Sem panics em entradas nao confiaveis**: lexer/parser/compiler/VM
  retornam erros (`VmError`); `unwrap()`/`expect()` fora de testes so em
  invariantes internas (cursor apos `peek`, chaves recem-ordenadas, mutex
  do servidor). Excecoes conhecidas e corrigidas: aninhamento sintatico
  alem de 64 vira erro `profundidade maxima` (AUD-001); valor profundo
  em `echo`/`str()`/`throw` vira erro tratavel em vez de abort (VM-001,
  0.7.2).
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
