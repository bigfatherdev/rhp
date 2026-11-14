# MEMORY_MODEL.md — Modelo de memoria (Fase 1)

Estrutura reservada. Decisao sobre ownership, GC/contagem de referencias e
limites fica para ADR na Fase 2 (conforme prompt §4 e governanca §10).

## A definir

- Ownership e aliasing de valores.
- Referencias e ciclos.
- Finalizacao/destrutores (se existirem).
- Coleta (GC) ou contagem de referencias.
- Concorrencia e excecoes/unwinding.
- Limites de heap/stack.
- Compatibilidade entre versoes.

Estado da Fase 1 (sem modelo definitivo): valores `Clone` em `Vec`
(pool + pilha), sem compartilhamento nem coleta; apenas `Str` alocado.
Nenhum comportamento de memoria alem disso e prometido.
