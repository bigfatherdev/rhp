# FAQ — RHP

**O RHP roda PHP?**
Nao. E uma linguagem independente com sintaxe de inspiracao PHP. Nao ha
compatibilidade de codigo nem runtime compartilhado.

**E tipado?**
Dinamico com erros explicitos em runtime. Sem coercao silenciosa e sem
`null` implicito (use `let x;` => `null` explicito). Tipos estaticos exigem
RFC futura.

**Por que `if 1` da erro?**
Condicoes exigem `Bool`. Use comparacoes explicitas (`if x != 0`).

**Por que `a[0] = v` funciona mas `a[0][1] = v` nao?**
Atribuicao indexada cobre um nivel (`nome[expr]`); cadeias em leitura
(`m["a"][0]`) funcionam normalmente.

**`push` muda meu array?**
Nao — valores sao copiados. `a = push(a, x)` retorna array novo. (Custo O(n);
ver `docs/BENCHMARKS.md`.)

**`map["k"]` inexistente retorna `null`?**
Nao — e erro (`chave inexistente`). Faca `keys(m)` para inspecionar.

**Posso importar qualquer arquivo?**
So dentro da arvore do programa (`..` para fora e erro), no nivel superior
do arquivo, sem ciclos. Teto de 8 MiB por arquivo. Nomes sao privados por
padrao: `export fn/let/const` publica; `import { a as b } from "m.rhp"`
vincula so exportados (`import "m.rhp"` mantem o legado: vincula tudo).

**`http_get` faz HTTPS?**
Nao — so `http://`, sem redirects, teto de 1 MiB, timeouts de 5s (DNS sem
timeout proprio). Para redirects/headers/timeout configuravel use
`http_fetch(url, opts)` (HTTPS continua sem suporte: exige TLS via ADR).

**Como limito um programa possivelmente infinito?**
`rhp run --max-steps N <arquivo>` (ou `VmLimits` via API interna).

**Como conecto em banco de dados (PostgreSQL/Redis)?**
Ainda nao ha drivers nem bibliotecas de rede para bancos — estao planejados
(ver `ROADMAP.md`) e exigem servidores reais para validar. Para modelar dados
em memoria, use arrays de mapas (ver `examples/mini_db.rhp` e
`examples/agenda.rhp`).

**Onde esta a especificacao de verdade?**
`LANGUAGE_SPEC.md` (linguagem), `BYTECODE_SPEC.md`, `VM_SPEC.md`,
`MEMORY_MODEL.md`. Este `docs/` e guia; em divergencia, valem as specs e,
acima delas, `GOVERNANCE.md`.
