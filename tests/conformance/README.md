# Suite de conformidade (`tests/conformance/`)

Programas RHP com resultado esperado pinado, executados via `rhp run`
pelo teste `conformance_suite_*` (`crates/rhp-cli/tests/conformance.rs`).

## Formato

- `NNN-nome.rhp` + `NNN-nome.expected`: programa válido; exit 0 e
  stdout **byte a byte** igual ao `.expected`.
- `NNN-nome.fail.rhp` + `NNN-nome.fail.expected`: programa inválido;
  exit != 0 e stderr **contém** o `.fail.expected`.
- `*-lib.rhp`: auxiliar importado por outro caso; não executa sozinho
  (o runner pula).

## Regras

- Casos devem ser herméticos e determinísticos: sem rede, sem `now()`,
  sem `rand()`, sem escrita fora do diretório do caso.
- Todo comportamento pinado deve estar na `LANGUAGE_SPEC.md` (ou em
  RFC/ADR aceita). Não fixar bug como esperado: se a saída real
  contradiz a spec, o caso não entra — abre-se correção.
- Ao adicionar um caso, rode o binário, confira a saída contra a spec
  e só então escreva o `.expected` (nunca cole saída cega).
- Números mínimos cobrados pelo runner: 18 casos válidos, 6 inválidos.
