# PHP_DIFFERENCES.md — Diferencas em relacao ao PHP (Fase 1)

Arquivo curto de estrutura. Conteudo definitivo sera preenchido a medida que
a semantica for especificada e implementada.

## A definir

- Sintaxe comparada (`echo`, `;`, strings) apos ampliacao da linguagem.
- Sistema de tipos e conversoes.
- Semantica de `echo`/`print` vs PHP.
- Modelo de execucao (bytecode/VM proprios vs Zend).
- Biblioteca padrao e interoperabilidade (sem compatibilidade prometida).

Estado da Fase 1: `echo "s";` escreve `s\n`; programa e sequencia de `echo`s
de strings. Qualquer comparacao alem disso e especulativa e nao faz parte
do contrato.
