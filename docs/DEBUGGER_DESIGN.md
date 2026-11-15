# DEBUGGER_DESIGN.md — Projeto de debugger (desenho, sem implementacao)

> Decisao consciente: nenhum debugger improvisado. Este documento registra a
> arquitetura necessaria e o que falta para implementa-lo com seguranca.

## Capacidades alvo

Breakpoints (linha), step over/into/out, inspecao de variaveis (escopo
corrente + globais), inspecao de frames, stack trace com nomes de funcao,
avaliacao de `echo`/estado da VM pausada.

## Pre-requisitos ja existentes

- Frames explicitos com `ip` e pilhas de escopos (`rhp-vm`).
- Nomes de funcoes/parametros preservados no chunk (`FuncDef`, pool `names`).
- Diagnosticos estruturados com posicao (`rhp-diagnostic` + `rhp check`).
- `VmLimits` (teto de instrucoes — util para "executar ate N passos").

## Lacunas a fechar antes de implementar

1. **Mapa op→span**: o bytecode nao guarda posicao de origem. Exige
   tabela lateral `Vec<(ip, linha, coluna)>` emitida pelo `compile`
   (sem mudar semantica; custo de memoria opt-in, so em modo debug).
2. **Pontos de parada sem custo no caminho rapido**: trocar instrucao por
   `Break` exigiria reescrever `code` (invalida saltos absolutos — recalcular
   ou tabela de remap). Alternativa: checagem `if breakpoint.contains(ip)`
   por passo (simples, custo por instrucao; aceitavel so em modo debug).
3. **Pausa/inspecao**: `Vm` precisaria de API `step()` publica + acesso de
   leitura a frames/escopos (hoje privados) e protocolo de comandos
   (DAP e o alvo natural, fora do std — exigiria crate externa ou protocolo
   proprio documentado).
4. **Step over/out**: contadores de profundidade de frame (ja ha `frames.len()`)
   + teto temporario de passos (reuso de `VmLimits`).

## Roteiro proposto (quando aprovado, nesta ordem)

1. ADR propria com formato do mapa op→span e politica de custo.
2. `compile(debug_info: bool)` opt-in (padrao desligado; bytecode de
   release inalterado e sem custo).
3. API `Vm::step()` + inspecao somente-leitura + teste de sessoes
   scriptadas (sem UI).
4. Adaptador DAP ou protocolo proprio documentado (ai sim, ferramenta real).

## Nao-metodos

Monkey-patch de `code` em runtime sem remap de saltos; `println!`-debugging
como "debugger"; dependencias externas sem aprovacao.
