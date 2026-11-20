# FINDINGS.md — Achados independentes (RHP-AUD-IND-*)

| ID | Cat. | Problema | Evidência | Impacto | Prior. | Estado |
|---|---|---|---|---|---|---|
| IND-001 | VM | HOF com callback que lança: `pending` obsoleto consumido por `Return` posterior (erro fantasma ou valor errado silencioso) | m06–m12, v03 | P0 — corrupção de estado/resultados | P0 | **Corrigido** (+regressão que falha sem o fix) |
| IND-002 | PG | Erro no meio do result-set não drenava até `ReadyForQuery`: próximo comando lia `Z` alheio (sucesso falso + erro atrasado) | d13/d14 (v3-ok + v4 com erro de v3) | P1 — integridade de dados | P1 | **Corrigido** (+teste live) |
| IND-003 | MySQL | `db_exec(SELECT)` deixava result-set não lido → dessincronia (`lenenc truncado`) | d15 | P1 — integridade de dados | P1 | **Corrigido** (+teste mock que falha sem o fix) |
| IND-004 | Stdlib | Sem `fs_rename`/`fs_remove`/`fs_mkdir` (ciclo de vida incompleto) | f-probes, REFERENCIA | P2 — API incompleta | P2 | **Implementado** (83 builtins + teste + docs) |
| IND-005 | CLI | USAGE dizia `build ... [--check]` mas 4º arg é saída (criava arquivo `--check`) | build manual | P3 — docs/UX | P3 | **Corrigido** (+teste) |
| IND-006 | Docs | `db_exec(SELECT)`: MySQL erra, PG retorna tag (inconsistente) | d15/d16 | P3 — docs | P3 | **Documentado** (REFERENCIA) |
| IND-007 | HTTP | CL com OWS, TE maiúsculo, TE+CL, extensões, hex maiúsculo — todos corretos | hsrv2 (5×200) | — | — | Verificado, sem ação |

Não confirmados como bug (verificados corretos): OWS-trim, TE+CL→chunked,
HEAD como GET, pipelining ignorado (sem keep-alive, documentado), slow-headers
dentro do prazo, `async_cancelled` fora de tarefa = false (documentado),
`int(3.9)` erro (documentado), `has` mapa-only (documentado), `min` binário
(documentado), `sort` 1-arg (documentado), `assert` 1-arg (documentado),
bool sqlite/mysql→int (documentado 0.6.1), O(n²) (semântica), FS sem sandbox
(desenho), root ignora chmod (ambiente), `1+...+1`×64 no limite (correto).
