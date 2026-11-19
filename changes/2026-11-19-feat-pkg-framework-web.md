# Added — RHP Web distribuido via `rhp pkg` (item 2)

- ID: 2026-11-19-feat-pkg-framework-web
- Data: 2026-11-19
- Versão: 0.4.0
- Tipo: Added
- Compatibilidade: Compatível (aditivo; so docs + teste)

## Resumo

O framework web oficial passa a ser distribuido como pacote v0: `rhp pkg add
rhp_web <repo>/framework` vendoriza em `pacotes/rhp_web/` com `rhp.lock`.

## Motivação

Item 2 das pendencias: antes, o unico caminho era copiar `web.rhp`
manualmente (vendorizacao manual), sem trava de integridade nem fluxo de CI.

## O que mudou

- Nenhuma mudanca de codigo no gerenciador (`pkg add` de diretorio ja
  copiava a arvore + travava hashes); o trabalho foi verificar, testar e
  documentar o fluxo para o framework.
- `docs/WEB.md`: instalacao via `rhp pkg add rhp_web ...` como recomendado
  (+ `rhp pkg install --check` no CI); vendorizacao manual como alternativa.
- `framework/web.rhp`: cabecalho de uso atualizado para o caminho
  `pacotes/rhp_web/web.rhp`.
- Teste `nextgen_framework_via_pkg`: `init` + `add` + arquivos + lock +
  `install --check` + execucao real do dispatch (`/users/7`).

## Detalhes técnicos

Sem alteracao em `pkg.rs`/loader; cobertura por teste de integracao em
`crates/rhp-cli/tests/integration.rs`.

## Compatibilidade e migração

Nenhuma; fluxo novo e opcional.

## Segurança e performance

Herda as garantias do `pkg` v0 (hashes FNV-1a por arquivo, `--check` no CI;
sem rede/registro). Sem impacto em runtime.

## Testes e validação

- `cargo test -p rhp-cli --test integration nextgen_framework_via_pkg`
  verde; suite integral sem regressoes (ver auditoria final).

## Limitações e riscos conhecidos

Sem registro remoto/versoes (escopo v0 do `pkg`); atualizacao = `pkg add`
de novo.

## Referências

RFC 0027; `docs/WEB.md`; `framework/web.rhp`.
