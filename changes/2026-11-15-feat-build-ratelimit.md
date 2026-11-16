# [Added] — `rhp build` (bundle autocontido) + limite de taxa no servidor

- ID: 2026-11-15-feat-build-ratelimit
- Data: 2026-11-15
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (comandos/opções novos; bundle executa
  idêntico ao original)

## Resumo

- `rhp build prog.rhp [saida]`: resolve imports (linker), valida e
  escreve `<base>.bundle.rhp` autocontido (sem `import`s) — artefato de
  deploy. Padrão: ao lado da entrada.
- `http_serve` ganha `limite_taxa` (requests/segundo por IP, janela fixa
  de 1 s, teto de 4096 IPs rastreados; 0 = sem limite): excedeu → 429 +
  `Retry-After: 1`.
- Correção de isolamento: nomes internos do linker mudaram de prefixo
  com NUL (irrepresentável no fonte) para `__rhp_mod{N}__` (válido, mas
  **reservado** — linker rejeita declaração com esse prefixo). NUL
  quebrava qualquer roundtrip por texto (`rhp build`, `rhp fmt` do
  bundle); descoberto testando o `build`.

## Motivação

Fase D: artefato de deploy sem resolver imports em produção + proteção
básica anti-abuso do servidor.

## O que mudou

- CLI: `rhp build` (+ USAGE, + `docs/CLI_REFERENCE.md`).
- `http_server`: `ServeOpts.limite_taxa`, contador por IP com janela
  fixa, 429; testes unitários com relógio injetado.
- Linker: `hidden_prefix()` + rejeição de `__rhp_mod_*` em top-level
  (qualquer módulo, inclusive entry).
- Spec: `LANGUAGE_SPEC.md` (`limite_taxa` no `http_serve`).

## Compatibilidade e migração

Aditivo. Exceção: código (hipotético) usando o prefixo `__rhp_mod_`
agora falha no link com erro claro (antes colidiria silenciosamente).

## Segurança e performance

429 não vaza nada. Contador limitado (evicção total ao encher).
`build` não executa nada (só lê/escreve arquivos).

## Testes e validação

- Falha-antes: comandos/opts inexistiam; bundle com NUL não reparseava.
- `rhp build` e2e: bundle sem `import`, saída idêntica (`42`), `-o`
  explícito; rate limit: unidade determinística + fiação (200s normais)
  + `limite_taxa` inválido rejeitado.
- `cargo test --workspace`: 216 passed, 0 failed (inclui e2e do bundle,
  rate limit, sessão com banco, template).

## Limitações e riscos conhecidos

Sem minificação/tree-shaking no bundle. `limite_taxa` é por processo
(sem estado distribuído). Sem `rhp fmt --check` no bundle (já sai
canônico).

## Referências

Fase D. RFC 0002 (limites do servidor).
