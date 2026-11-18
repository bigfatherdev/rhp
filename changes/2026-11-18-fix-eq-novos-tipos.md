# [Fixed] — `==` sempre falso para classes, instâncias e geradores

- ID: 2026-11-18-fix-eq-novos-tipos
- Data: 2026-11-18
- Versão: unreleased
- Tipo: Fixed
- Compatibilidade: Compatível (corrige para o especificado nas RFCs)

## Resumo

`values_equal` (caminho do `Op::Eq`) não tinha braços para
`Value::Class/Instance/Generator`: qualquer `==` com esses valores
devolvia `false` (inclusive `g == g`). Achado ao escrever os exemplos
da referência, que afirmavam outro comportamento.

## Motivação

Correção de bug real encontrado em verificação manual (nenhum teste
cobria `==` com os novos tipos; `.stderr`? não — resultado errado
silencioso, o pior tipo de bug).

## O que mudou

- `values_equal`: `Class` compara por nome; `Instance` compara por
  estrutura (mesma classe + mesmos campos, recursivo e finito —
  valores RHP sao arvores sem ciclos); `Generator` delega ao
  `PartialEq` por identidade (`Rc::ptr_eq` + `done`).
- Sem risco de recursao infinita: `Display` nao desce no estado,
  `json` rejeita, heap tem teto anti-ciclo, `==` estrutural so desce
  em valores (finitos por construcao).

## Compatibilidade e migração

Compatível (alinha com RFCs 0017/0018, que ja prometiam essas
semanticas).

## Segurança e performance

Sem `unsafe`; comparacao O(tamanho) como as existentes.

## Testes e validação

- 2 novos asserts (gerador: `a==a` true, `a==b` false; instancia:
  estrutural antes/depois de mutacao; classe por nome).
- `cargo test --workspace`: 324 passed, 0 failed; `fmt`/`clippy` limpos.

## Limitações e riscos conhecidos

Nenhum alem dos documentados nas RFCs (sem `==` profundo entre
classes distintas por nome igual — nomes sao a identidade).

## Referências

- RFC 0017 (OO) e RFC 0018 (geradores); `docs/REFERENCIA.md` §7–8.
