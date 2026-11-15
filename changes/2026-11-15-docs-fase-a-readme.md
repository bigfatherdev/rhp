# [Changed] — Docs sincronizados com Fases 7 e A (README/FAQ/SECURITY)

- ID: 2026-11-15-docs-fase-a-readme
- Data: 2026-11-15
- Versão: unreleased
- Tipo: Changed
- Compatibilidade: Compatível (só documentação)

## Resumo

`README.md`, `docs/FAQ.md` e `SECURITY.md` afirmavam o estado da Fase 6
(133 testes, stdlib antiga, "sem redirects", exports no planejado).
Atualizados para o estado real pós-Fase A.

## Motivação

Auditoria de "o que falta": docs de entrada desatualizados enganam mais
que código — usuário leria que redirects não existem e que exports são
planos futuros.

## O que mudou

- `README.md`: características (exceções, stdlib atual, módulos v2),
  contagem de testes (133 → 195), segurança (redirects via `http_fetch`;
  HTTPS segue fora), roadmap (Fase 7 + Fase A concluídas; planejado sem
  exports, com pacotes/servidor HTTP/TLS).
- `docs/FAQ.md`: `http_get` × `http_fetch` (HTTPS segue sem suporte);
  import com `export`/seletivo resumido.
- `SECURITY.md`: redirects qualificados (só `http_fetch`, teto 10).

## Detalhes técnicos

Nenhum código alterado. Sondas manuais confirmaram o documentado
(seletivo + alias, `catch` com mapa `{tipo}`, `export` aninhado
rejeitado, `rhp check` ok).

## Compatibilidade e migração

Não necessária.

## Segurança e performance

Sem impacto (documentação).

## Testes e validação

- `cargo test --workspace`: 195 passed, 0 failed (sem mudanças de código).
- `cargo fmt --check` ok; `cargo clippy --workspace --all-targets
  -D warnings` ok; `tools/check-changes.sh origin/main` ok.

## Limitações e riscos conhecidos

Contagem de testes pinada no README (195) apodrece a cada lote (mesmo
padrão anterior com 133; aceito como débito leve e explícito).

## Referências

Fases 7 e A (`ROADMAP.md`, `CHANGELOG.md`).
