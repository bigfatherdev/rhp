# ADR 0009 — Módulos de segunda geração: semântica antes de código (CP5)

- Data: 2026-11-14
- Estado: proposta aceita como direção; **implementação expressamente adiada**
  para RFC/fase própria (missão §7: definir semântica primeiro)
- Contexto: auditoria NG-203; `import` DFS atual funciona e permanece
  inalterado (compatibilidade total)

## Decisão (semântica proposta, sem código nesta entrega)

1. **Explícitos, não implícitos.** `export` opt-in por definição
   (`export fn f`, `export let x`); sem `export` = privado ao módulo.
   Reexportação explícita (`export ... from "..."` — sintaxe a confirmar em RFC).
2. **Privacidade real.** Nome não exportado é invisível fora do módulo;
   colisão só ocorre entre nomes exportados (erro alto, como hoje).
3. **Alias.** `import { dobro as dbl } from "mat.rhp"`; alias evita colisão
   sem renomear a origem.
4. **Identidade canônica.** Caminho canonicalizado + hash do conteúdo como
   identidade (mesmo arquivo via links distintos = um módulo; conteúdos
   diferentes = módulos diferentes, erro se o mesmo caminho mudar no meio).
5. **Dependências e ciclos.** Ciclos continuam proibidos (cadeia no erro);
   `import` continua no nível superior; carregamento continua em DFS com
   `once` por identidade canônica.
6. **Reprodutibilidade.** Ordem de execução determinística (DFS, fonte
   antes de dependentes como hoje); sem cache entre execuções nesta fase
   (cache em memória com chave de conteúdo fica para a implementação).
7. **Isolamento.** Módulos partilham o programa (sem sandbox); jaula na
   subárvore do entry e teto de 8 MiB mantidos; pacotes futuros resolvem
   fora da árvore via manifesto (não especificado aqui).
8. **Gerenciador de pacotes.** Fora do escopo: requer registro, versões,
   lockfile e resolução — listado como ecossistema futuro com esta ADR
   como pré-requisito.

## Alternativas consideradas

- `export` por padrão com `priv` opt-out: rejeitado — quebra silenciosa de
  encapsulamento futuro; opt-in preserva compatibilidade total com o
  `import` atual (tudo hoje é público; nada muda até a RFC).
- Implementar já: rejeitado pela missão (§7) e pela governança (semântica
  normativa antes de código; testes de compatibilidade exigidos).

## Consequências

- Nenhuma mudança de código/spec nesta entrega; `LANGUAGE_SPEC.md` §1
  continua valendo integralmente.
- Próximo passo (requer RFC + aprovação): sintaxe final, `BYTECODE_SPEC`
  (tabelas de export), `VM_SPEC` (resolução), suíte de compatibilidade
  (público x privado x alias x ciclo x identidade).
