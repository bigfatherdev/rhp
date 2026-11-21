# Performance — `json_parse` de string longa: O(n²) → O(n) (43x)

- ID: 2026-11-21-perf-json-parse
- Data: 2026-11-21
- Versão: 0.7.2
- Tipo: Performance
- Compatibilidade: Compatível

## Resumo
`json_parse` de string com 400 KB levava 8.6 s (build debug); agora 0.2 s.

## Motivação
Auditoria 0.7.2: PERF-004 (P2, medido).

## O que mudou
`parse_string`: em vez de `from_utf8(restante)` por char (revalidava o
sufixo inteiro a cada caractere), decodifica pela largura do byte
inicial (`utf8_width`) e valida só a fatia do escalar. Rejeita as mesmas
entradas (controles, truncadas, continuações órfãs, `0xC0/0xC1/0xF5+`).

## Detalhes técnicos
`crates/rhp-vm/src/json.rs` (`utf8_width`, `_` em `parse_string`).
`pos` sempre em fronteira de char (estrutura só consome ASCII fora do
braço), então a fatia é válida por construção; `from_utf8` da fatia é
defesa em profundidade O(1).

## Compatibilidade e migração
Compatível: mesmos valores, mesmos erros, mesmas posições de byte.
Sem migração.

## Segurança e performance
Medido (AMD EPYC, debug): string 400 KB — antes 8.60 s, depois 0.20 s
(~43x). Array 80k ints 0.21 s, mapa 8k 0.40 s (inalterados).
`PERF-001/002/003` (O(n²) por semântica de valor) seguem documentados,
sem mudança de semântica.

## Testes e validação
- `runs_json_roundtrip` + novo caso multibyte (`olá 🌍`, par substituto): ok.
- Probe manual multibyte/escapes/surrogate: ok.
- `cargo test --workspace`: 395 passed; fuzz 1000 limpo.

## Limitações e riscos conhecidos
Nenhum novo. `json_stringify` já tinha teto 128 (inalterado).

## Referências
PERF-004 (auditoria 0.7.2).
