# PHP_DIFFERENCES.md — Diferencas em relacao ao PHP (Fase 1)

Arquivo curto de estrutura. Conteudo definitivo sera preenchido a medida que
a semantica for especificada e implementada.

## Closures / funcoes anonimas (RFC 0003, P1)

- Sintaxe igual ao PHP: `fn (params) { corpo }` como expressao;
  `fn nome(params) { }` declara no escopo atual.
- Captura implicita por valor (snapshot dos escopos): difere do PHP, que
  exige lista `use (...)` (por valor) ou `&` (por referencia). Nao ha
  `use` no RHP; toda variavel visivel e capturada automaticamente.
- Sem `use (&$x)`: nao ha captura por referencia (decisao de seguranca;
  reavaliar apenas com proposta e aprovacao).
- Sem ligacao automatica de `$this` (fora do escopo P1; metodos/OO sao
  fase futura — ver RFC 0003 §6).
- `echo $f` / concatenacao de closure segue `Display` (`<fn ...>`); no PHP,
  `Closure` nao converte para string (erro). Detalhes de conversao serao
  definidos na fase de tipos.

- Sintaxe comparada (`echo`, `;`, strings) apos ampliacao da linguagem.
- Sistema de tipos e conversoes.
- Semantica de `echo`/`print` vs PHP.
- Modelo de execucao (bytecode/VM proprios vs Zend).
- Biblioteca padrao e interoperabilidade (sem compatibilidade prometida).

Estado da Fase 1: `echo "s";` escreve `s\n`; programa e sequencia de `echo`s
de strings. Qualquer comparacao alem disso e especulativa e nao faz parte
do contrato.
