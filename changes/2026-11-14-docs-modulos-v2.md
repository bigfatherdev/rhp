# [Docs] — CP5: ADR de módulos de segunda geração (sem código)

- ID: 2026-11-14-docs-modulos-v2
- Data: 2026-11-14
- Versão: unreleased
- Tipo: Docs (decisão arquitetural)
- Compatibilidade: Compatível (zero mudança de código/comportamento)

## Resumo

`ADRs/0009-modulos-v2.md`: semântica proposta (exports opt-in, privacidade,
alias, identidade canônica, ciclos, reprodutibilidade, isolamento,
gerenciador futuro) com implementação expressamente adiada para RFC/fase
própria. O `import` atual funciona exatamente como antes.

## Motivação

Missão Next Generation CP5 (auditoria NG-203): definir antes de implementar.

## O que mudou

- `ADRs/0009-modulos-v2.md` (novo). Nenhum `.rs`, spec ou teste tocado.

## Compatibilidade e migração

Total. Migração: não necessária (nada para migrar — semântica futura).

## Segurança e performance

N/A (documento de direção).

## Testes e validação

- `cargo fmt --check` ok (arquivo novo é Markdown).
- Suite inalterada e verde por construção (nenhum código tocado).

## Limitações e riscos conhecidos

Proposta, não implementação; qualquer desvio na RFC futura exige revisão
deste ADR.

## Referências

- Auditoria: `reports/NEXT_GENERATION_AUDIT.md` (NG-203). Missão §7.
