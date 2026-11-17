# [Added] — Stack traces em erros de runtime (Etapa 4, DevX)

- ID: 2026-11-16-feat-stack-traces
- Data: 2026-11-16
- Versão: unreleased
- Tipo: Added
- Compatibilidade: Compatível (só adiciona linhas `  em f` após o erro;
  `message` intacta para `try`/`catch`)

## Resumo

`VmError.trace: Vec<String>` (nomes de funcao, interna → externa);
`run`/`run_value` anexam no erro (frames intactos — `unwind` só trunca
com handler); CLI imprime `  em f` após a mensagem. Ausente no nível
superior (sem ruído).

## Motivação

Etapa 4, item "stack traces": depurar sem pilha era tentativa e erro.

## O que mudou

- VM: campo `trace` + acessor + `with_trace`; `call_stack()` (rev,
  filtra frames sem `func`).
- CLI `run` e `rhp test`: apensam `  em <nome>` (fora de `Fail::Msg`
  principal? dentro; `message` da VM inalterada).
- Testes: `error_carries_call_stack` (ordem, vazio no topo, `try`
  intacto) + `rhp_run_error_shows_call_stack` (CLI, ordem + ausência
  no topo).

## Compatibilidade e migração

Compatível; exemplos e testes que casam stderr exato sem `em ` seguem
verdes (só adiciona linhas quando HÁ pilha).

## Segurança e performance

Sem `unsafe`; sem alocacao no caminho feliz (1 `Vec` so no erro).
Sem impacto mensuravel.

## Testes e validação

- 2 testes novos (VM + CLI); suite final 321/0; fmt/clippy limpos.

## Limitações e riscos conhecidos

- Sem numeros de linha (bytecode nao carrega spans; nomes apenas).
- `http_serve` nao exibe trace (500 sem detalhe, por desenho).

## Referências

- `VM_SPEC.md` (inalterado — formato de erro CLI, nao VM).
