# REGRESSION_AUDIT.md — Revalidação das correções 0.6.x (com variantes)

Todas as correções 0.6.0/0.6.1 foram re-testadas com variações, não só com o
teste original.

## return/try (0.6.0)

- Casos originais: OK via suíte.
- Variantes novas (todas OK): return de catch, try triplo-aninhado, closure
  em return, recursão com falha (`-2` correto), HOF com throw em return,
  while+return parcial (`g=1` preservado), return em finally, yield em try
  com catch interno, throw em `usort`/comparador, erro em nível 3 com catch
  em nível 1, loop com throw+acumulação (`104`).
- Estado da VM após erro capturado: intacto (aritmética, coleções, funções,
  loops, match executam normal depois).

## http_serve host/bind (0.6.0)

- `0.0.0.0`, `127.0.0.1`, `localhost`, IP específico: OK. Inválidos
  rejeitados com E3000 claro. Porta ocupada: erro com porta citada.

## HTTP endurecido (0.6.0) + chunked/localhost (0.6.1)

- 400s, 64 KiB, CL estrito/inválido, corpo incompleto, chunked (válido,
  extensão, hex maiúsculo, `Chunked`, TE+CL, `0`-imediato, inválido,
  truncado), OWS em CL (trim correto → 200), UTF-8, loop limitado,
  aninhado rejeitado, 8× concorrentes, PUT/PATCH/DELETE/OPTIONS, HEAD,
  handler com sqlite (3 requests, estado em arquivo), logs opacos+stderr.
- Sem regressão: 15 testes HTTP da suíte verdes.

## Teto de profundidade 64 (0.6.1)

- 62/63 passam, 64/65 falham limpo (E1000 com posição); misto ninho+cadeia
  no limite passa; `stack_probe` em thread 2 MiB sem abort em nenhum caso
  ≤ limite (antes: abort em 128).
- Sem regressão: framework `web.rhp`, exemplos e suíte passam.

## MySQL seq (0.6.1)

- Mock asserta `seq == 0` por comando; live MariaDB 11 + MySQL 8.0
  (native): CRUD, tipos, 6 classes de erro limpas.

## Mensagens 2-arg (0.6.1)

- `split/has` citam `(string, int)`/`(array, string)`; nenhuma asserção
  antiga quebrada (grep na suíte).

## Regressão encontrada nesta auditoria

- **RHP-AUD-IND-001** (HOF `pending` obsoleto): somente a forma com HOF de
  0.6.x estava vulnerável — as correções 0.6.x em si não regrediram.
- **RHP-AUD-IND-002/003** (dreno pg/mysql): idem — lacunas, não regressões.
