# FIXES.md — Correções implementadas (com causa raiz e validação)

## RHP-AUD-001 — Contenção de profundidade do parser (P0)

- **Causa**: cadeias planas (`Binary` à esquerda, unários, `?:`, `??`,
  sufixos) não passavam por `nest()`; AST profunda estourava a pilha
  nativa em parse/compile/Display/`Drop` (SIGABRT).
- **Fix** (`crates/rhp-parser/src/lib.rs`): `Cursor::check_chain` (teto =
  `depth + elos ≤ MAX_PARSE_DEPTH`) nos 9 laços (8 binários + pós-fixo);
  `nest()` nas recursões (unário, ramos do ternário, `??` à direita).
- **Calibragem**: `MAX_PARSE_DEPTH` 128→64 (medição `stack_probe`: 120 ok,
  128 aborta em thread 2 MiB debug; 64 ≈ 2× margem). Spec e docs atualizados.
- **Validação**: 8 vetores viram E1000 limpo; suíte 364→367 verde (3 testes
  novos); fuzz 2500 casos sem crash; piores misturas em 2 MiB sem crash.

## RHP-AUD-008 — MySQL `seq` por comando (P1)

- **Causa**: `command()` nunca zerava `seq`; após o handshake o servidor
  esperava 0 e respondia 1156 em todo comando — conector 100% inoperante
  (nunca testado ao vivo; mock não checava `seq`).
- **Fix** (`crates/rhp-vm/src/mysql.rs`): `self.seq = 0` no início de
  `command()` (+ comentário RHP-AUD-008).
- **Validação**: mock agora asserta `seq == 0` (falha sem o fix, passa com);
  live MariaDB 11 + MySQL 8.0 (native): CRUD, tipos, 6 classes de erro limpas.

## RHP-AUD-003 — Mensagens de tipo 2-arg (P3)

- **Causa**: padrão `(other, _)` reportava só `type_name(other)`.
- **Fix** (`crates/rhp-vm/src/builtins.rs` + 1 `replace`): 23 sites passam a
  citar ambos (`(string, int)`); precedente interno já existia (HOFs).
- **Validação**: nenhum teste existente asseria as strings antigas; 2 asserts
  CLI novos; suíte verde.

## RHP-AUD-005 — Bool SQLite/MySQL (docs)

- Sem fix de código (mapear 0/1→bool corromperia inteiros); nota em
  `docs/REFERENCIA.md` (`db_query`): só PG round-tripa `Bool`.

## RHP-AUD-012 — Teto 128→64 (compatibilidade)

- Mudança observável documentada (cadeias 65–128 agora E1000; código real
  não chega perto); teste 100→50 justificado na mensagem do próprio teste;
  spec (`LANGUAGE_SPEC.md` §1) atualizada.
