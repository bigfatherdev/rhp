# Performance — Router do RHP Web com trie ordenada (prova diferencial)

- ID: 2026-11-19-perf-web-trie
- Data: 2026-11-19
- Versão: 0.5.0
- Tipo: Performance
- Compatibilidade: Compatível (semantica byte-identica, provada por teste)

## Resumo

O despacho do RHP Web (`framework/web.rhp`) passa de varredura linear para
trie ordenada por indice global de registro: mesma semantica (primeiro match
vence; `Allow` identico, com duplicatas), sem `split` por rota.

## Motivação

Item "roteamento linear" das limitacoes: O(R) splits por request nao escala
com centenas de rotas.

## O que mudou

- `web_registrar` constroi `__web_trie` (nos `{estaticos, param, rotas}`;
  estaticos em mapa O(1); atualizacao funcional compativel com valores por
  copia e atribuicao de um nivel).
- `web_despachar`: 1 `split` + DFS com poda por prefixo + ordenacao dos
  candidatos por indice global (insertion sort; candidatos sao poucos) +
  iteracao so nos padroes que casaram.
- `framework/web_trie_test.rhp`: diferencial trie-vs-linear (implementacao
  de referencia = algoritmo anterior copiado) em 16 casos incluindo
  armadilhas de ordem (`/users/novo` apos `:id`; `/a/b/c` apos `:x`),
  404/405 (com `Allow` duplicado), raiz e profundidades; mais asserts
  absolutos e unitarios de `web_casar`.
- `docs/WEB.md`: secao Performance com numeros.

## Detalhes técnicos

So `framework/web.rhp` (+ teste). Medicao (build dev, mesma maquina/carga:
100 rotas estaticas, match na ultima, 100 dispatches): linear ~7s, trie ~1s
(~7x; a vantagem cresce com R, pois o linear repete `split` + comparacao
por rota). Apps tipicos ficam bem abaixo de 1ms/request em build otimizado.

## Compatibilidade e migração

Nenhuma (comportamento observavel identico, provado pelo diferencial).

## Segurança e performance

Recursao limitada a profundidade do caminho (pequena); sem alocacoes
extras alem de mapas/arrays do modelo; sem impacto no worker/loader.

## Testes e validação

- `framework/web_trie_test.rhp` verde via `rhp run` e `rhp test framework/`
  (integracao `nextgen_framework_trie_test_suite`).
- `framework/exemplo_api.rhp` segue executando; suite integral verde.

## Limitações e riscos conhecidos

Roteamento ainda O(C log C) nos candidatos; sem cache de padrao compilado
(desnecessario: trie ja e a forma compilada).

## Referências

RFC 0027; `docs/WEB.md` (Performance); `framework/web_trie_test.rhp`.
