# [Added] — Sessões e templates: cookies, `template()`, padrão com banco

- ID: 2026-11-15-feat-sessao-template
- Data: 2026-11-15
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (builtin novo id 49; `req` ganha `cookies`)

## Resumo

Fase C3: parsing de `Cookie` (vira `req["cookies"]`), builtin
`template(texto, mapa)` (`{{chave}}` com escape HTML obrigatório) e
padrão documentado de sessão = cookie + linha no banco (stateless por
desenho). Middleware componível fica pendente (exige funções de primeira
classe).

## Motivação

Sem cookies/templates não há sites (login, páginas dinâmicas, XSS-safe).
Sem estado no servidor (decisão C1), sessão = cookie + banco (Fase B).

## O que mudou

- `rhp-vm/src/http_server.rs`: `Cookie` parseado (`;`-separado,
  aparado, aspas externas removidas) em `req["cookies"]`.
- Builtin `Template` (id 49, aridade 2): interpolação de passada única,
  escape `&<>"'` (nessa ordem), chave ausente/`{{` aberto/tipo errado =
  erro; sem laços/condicionais por desenho.
- `docs/EXEMPLO_SERVIDOR.md` com padrão de sessão + `docs/GETTING_STARTED`
  apontando; exemplo verificado de verdade contra servidor local.
- Spec: `LANGUAGE_SPEC.md` §6 (`template`); `BYTECODE_SPEC` §2/§4 (id 49);
  gramática + teste (`template`).

## Detalhes técnicos

Escape sempre (sem modo raw — XSS por padrão é proibido pelo desenho).
`Null` interpola como `null` (explícito, igual ao `Display`).

## Compatibilidade e migração

Aditivo. `req` ganha a chave `cookies` (código que iterava todas as
chaves de `req` veria uma a mais — aceito e documentado).

## Segurança e performance

Escape obrigatório previne a classe inteira de XSS por interpolação.
Sessões: id de sessão é opaco ao servidor; usar token não-adivinhável
em produção (sem CSPRNG na stdlib ainda — declarado).

## Testes e validação

- Falha-antes: sem `cookies`, sem `template`.
- VM: escape dos 5 caracteres, tipos, passthrough, erros.
- Integração (servidor real): login com nome malicioso (`<b>`) sai
  escapado no corpo E no `Set-Cookie` ecoado, perfil via cookie escapa,
  sem cookie/desconhecido = anônimo; `max_requisicoes` exato (4/4).
- `cargo test --workspace`: 205 passed, 0 failed (203 antes + 2 novos).
- `cargo fmt --check` ok; `clippy -D warnings` ok; `check-changes.sh` ok.

## Limitações e riscos conhecidos

Sem `{{else}}`/`{{if}}`/laços (lógica em RHP, template só interpola).
Sem `HttpOnly`/`Secure`/`SameSite` automáticos (app define o header).
Middleware componível aguarda primeira-classe. Sem CSRF dedicado.

## Referências

RFC 0002 (C3 dentro do escopo: cookies/sessões via banco/templates;
roteamento dedicado via `if` no handler; middleware componível aguarda
primeira-classe). Resto da Fase C: só docs de fechamento + PR.
