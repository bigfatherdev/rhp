# [Tooling] — Suite de conformidade + property tests ampliados (Etapa 3)

- ID: 2026-11-16-test-conformidade-etapa3
- Data: 2026-11-16
- Versão: unreleased
- Tipo: Tooling
- Compatibilidade: Compatível (só testes; nenhum comportamento alterado)

## Resumo

Cria `tests/conformance/` (18 programas válidos com stdout byte a byte
+ 6 inválidos com trecho de stderr, executados via `rhp run` por
`crates/rhp-cli/tests/conformance.rs`) e amplia os property tests
std-only da VM (`-`, `<`/`==`, `len()` de concatenação; 200 → 1800
programas gerados por seed fixa). Formato documentado em
`tests/conformance/README.md`.

## Motivação

GOVERNANCE §8.2 exige suite de conformidade; §8.5, fuzzing/property.
O estado anterior (levantado em 2026-11-16) era pinagens ad-hoc em
`integration.rs` + gerador só `+`/`*` com 200 casos. A suite nova é a
rede de segurança para os próximos passos da Etapa 3 (teto de heap,
otimizações).

## O que mudou

- 18 casos válidos (001–018): aritmética (inclui divisão inteira `7/2=3`
  vs `7.0/2=3.5`), strings, comparação, `?:`/`??` (só `null`), controle,
  `for-in`/`for-C`/`for(;;)`, `break 2`, `match`, `switch` (fallthrough
  pinado), funções/default/rest, closures (captura de local por valor =
  `1`; global lido ao vivo, cf. RFC 0003), HOFs, named/spread,
  destructuring, coleções, exceções, módulos (`import { } from`),
  `static`, ordem de avaliação esq→dir.
- 6 casos inválidos (101–106): `;` ausente, tipo, nome, aridade, índice,
  chave — com códigos E1004/E3001/E3002/E3004/E3007.
- Property: `gen_arith` ganha `-` (1000 casos, teto ~1.8e15 sem overflow
  de referência); `gen_cmp` (`<`/`==`, 500); `gen_concat` + `len`
  (300, inclui fragmento vazio).

## Detalhes técnicos

- Runner itera entradas ordenadas; caso válido exige `.expected` irmão
  (falha alto se ausente); `*-lib.rhp` pulado (auxiliar de import);
  mínimos cobrados: 18 válidos / 6 inválidos.
- Casos herméticos/determinísticos (sem rede, `now`, `rand`, escrita
  externa). saídas observadas no binário e conferidas contra
  `LANGUAGE_SPEC`/RFCs antes de pinar — não coladas às cegas.
- Durante a criação, um caso inicial de closure com global retornou `2`
  em vez de `1`; investigação mostrou comportamento correto e
  especificado (RFC 0003: globais lidos ao vivo; só locais capturados).
  Caso corrigido para local; nenhum código alterado.

## Compatibilidade e migração

Compatível; migração não necessária. Limitação conhecida: `check-changes.sh`
não rastreia `tests/` no diff relevante — o registro existe porque
`crates/` mudou junto; propor extensão do script depois (fora de escopo).

## Segurança e performance

Sem impacto: só arquivos de teste + `#[cfg(test)]`. Suite de
conformidade roda em ~0,3 s; property tests em ~0,4 s.

## Testes e validação

- `cargo fmt --all -- --check`: limpo.
- `cargo clippy --workspace --all-targets -- -D warnings`: 0 warnings
  (1 `needless_borrows_for_generic_args` encontrado e corrigido).
- `cargo test --workspace`: 288 passed, 0 failed em 14 suites (baseline
  284 + 4 novos: 2 `conformance_suite_*`, `property_comparison_*`,
  `property_string_concat_length`; `property_arithmetic_*` ampliado
  200 → 1000 casos).

## Limitações e riscos conhecidos

- Sem fuzz contínuo (cargo-fuzz/libfuzzer): gap assumido permanece;
  property std-only é substituto finito, não cobertura de fuzzer.
- Sem casos de rede/servidor/DB na conformidade (não herméticos);
  seguem cobertos por `integration.rs`.
- `RHP` não tem `&&`/`||` (só bitwise): sem caso de curto-circuito.

## Referências

- GOVERNANCE.md §8.2, §8.5; LANGUAGE_SPEC.md (ordem esq→dir, Elvis=`??`);
- RFC 0003 (captura: locais por valor, globais ao vivo);
- reports/PHP_PARITY_AUDIT.md (lacuna de conformidade).
