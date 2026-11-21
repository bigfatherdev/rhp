# Fixed — Framework web: `cabecalhos` opcional, 404/405 com selo, `Allow` único

- ID: 2026-11-21-fix-web-framework
- Data: 2026-11-21
- Versão: 0.7.2
- Tipo: Fixed
- Compatibilidade: Compatível

## Resumo
Resposta sem `cabecalhos` (docs dizem opcional) dava 500; 404/405 saíam
sem `nosniff`/`DENY`; `Allow` duplicava métodos (`GET, GET`).

## Motivação
Auditoria 0.7.2: HTTP-002 (P1), HTTP-003 (P3), HTTP-004 (P4).

## O que mudou
- `web_finalizar`: `has(resp, "cabecalhos")` com `{}` padrão; resposta
  não-str/não-mapa = erro claro (500 opaco, sem vazar).
- 404/405 passam pelo finalizar (selo de segurança em todos).
- `push_unique` (`contains`) no `Allow`.

## Detalhes técnicos
`framework/web.rhp` (`web_finalizar`, `web_despachar`, `push_unique`).
`web_seguranca`/413: inalcançável com teto 1 MiB do servidor —
documentado em `docs/WEB.md` (não é bug de código).

## Compatibilidade e migração
Compatível. Handlers que retornavam mapa sem `cabecalhos` (e recebiam
500) passam a funcionar. Sem migração.

## Segurança e performance
Selo em 404/405 fecha inconsistência de cabeçalhos. Sem custo.

## Testes e validação
- Probes live (`curl -D`): 201 sem cabecalhos + selo; 404/405 com selo;
  `allow: GET, POST` único: ok.
- `framework/exemplo_api.rhp`: "rotas ok".
- `cargo test --workspace`: 395 passed, 0 failed.

## Limitações e riscos conhecidos
Nenhuma nova. `web_seguranca` documenta o alcance real do 413.

## Referências
HTTP-002/003/004/005 (auditoria 0.7.2); RFC 0027; `docs/WEB.md`.
