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

## `switch` (RFC 0004, Fase 2)

- Sintaxe com braces (`switch e { case c { ... } default { ... } }`); sem
  `:` e sem `endswitch` (RHP nao tem sintaxe alternativa em nenhum comando).
- Comparacao com o `==` do RHP (promocao int/float; demais tipos exigem
  igualdade de tipo): difere do PHP, cujo `switch` e frouxo (`1 == true`
  casa la; aqui nao). `switch(true)` com cases int nao casa.
- Cada ramo tem escopo proprio (`let` repetido em ramos distintos nao
  colide); no PHP o `switch` inteiro compartilha um escopo.
- Sem `break N` (RHP nao tem `break` com argumento); `break` sai do
  `switch` mais interno, `continue` em `switch` equivale a `break` dele
  (gotcha do PHP, mesma semantica), `continue N` conta `switch` como nivel.

## Lote nucleo-2 (RFC 0008, Fase 3)

- `match` estrito (mesmo tipo; `Int`/`Float` promovem): como o PHP (`===`),
  divergindo do `switch`/ `==` frouxos do RHP. Sem-match sem `default` =
  erro (capturavel; no PHP e `UnhandledMatchError`).
- Elvis `?:` = `??` (só `null` aciona; sem truthiness — RHP não tem em
  nenhum lugar; no PHP, falsy aciona).
- Arrow `fn (x) => expr` = closure; fmt normaliza para bloco.
- `for init; cond; passo` igual ao PHP (partes opcionais); `continue` roda
  o passo; `let` do `init` nao vaza.
- `break N` / `continue N` como no PHP (N literal; `switch` conta como nivel
  e `continue` nele age como `break`).

## `map`/`filter`/`usort` (RFC 0005, Fase 2)

- Nomes curtos e colecao primeiro (`map(arr, f)`), convenção dos builtins
  RHP (`push`, `sort`); PHP usa `array_map(f, arr)` / `array_filter` /
  `usort(arr, cmp)` (ordem e nomes diferentes).
- Callbacks recebem exatamente os args documentados (1 para `map`/`filter`,
  2 para `usort`); RHP exige aridade exata, sem o relaxamento do PHP.
- `filter` exige retorno `Bool` (erro senão); PHP usa truthiness (RHP não tem).
- `usort` é estável (insertion sort); comparador deve retornar `Int`.

## Defaults e `...rest` (RFC 0006, Fase 2)

- Mesma sintaxe do PHP (`f(a, b = 2, ...rest)`); `...` sem conflito léxico
  (RHP não usa `.` para nada).
- Defaults restritos a expressões constantes (literais, operações entre
  constantes, compostos de constantes), como no PHP; avaliados em compilação.
- Sem spread em chamada, sem named arguments (fases futuras).

## Chamadas mistas (RFC 0009, Fase 4)

- Mesma sintaxe do PHP (`f(x: 1)`, `f(...arr)`); colecao de args igual.
- Estrito onde o PHP erra em runtime: posicional apos nomeado, duplicata
  e builtin com named/spread = erro de compilacao.
- Sem spread em literal de array (`[...a]` — futuro, exige maquinaria propria).

## Destructuring (RFC 0007, Fase 2)

- `let [a, b]` / `let {x}` / `let {k: v}` (e `const`); PHP usa `[$a, $b]` /
  `['k' => $v]` (com `$`, sem `const` desestruturado).
- Raso apenas (aninhado = erro de parse); sem `[, b]` nem `[a = 1]`.
- Estrito: posição/chave ausente = erro (PHP dá `null` + aviso).

## `static` (RFC 0010, Fase 4)

- Mesma sintaxe e semantica base do PHP (`static n = expr;` por definicao,
  primeira atribuicao vence). Init obrigatorio; fora de funcao = erro.
- Divergencia: init com efeito colateral roda por chamada (PHP avalia uma
  vez); o valor mantido e sempre o da primeira. Evite side effects no init.

## Re-export (RFC 0011, Fase 4)

- `export { a, b as c } from` (barris); transitivo; sem `export *`.
- Melhoria sobre o legado: aliases iguais da mesma origem nao duplicam
  (diamante funciona); origem distinta continua erro de colisao.

## `rand`/`fs_list` (RFC 0012, Fase 4)

- `rand()` sem args e `rand_range(min, max)` (o PHP dobra em `rand()`).
- `rand` NAO e criptografico (xorshift; sem `random_bytes` nesta leva).
- `fs_list` raso e ordenado (sem flag de recursao por ora).

## `hash_password`/`verify_password` (RFC 0013, Fase 5)

- Nomes proprios do RHP (nao `password_hash`/`password_verify`): sem
  promessa de agility de algoritmo nem constantes `PASSWORD_*`.
- Criacao sempre Argon2id (OWASP defaults); verificacao aceita `$argon2id$`
  e bcrypt `$2a$`/`$2b$`/`$2x$`/`$2y$` (migracao de bancos PHP: mesmos hashes
  verificam). PHP aceita os mesmos formatos na leitura.
- `verify` devolve `false` para senha errada, mas erro para hash malformado
  ou algoritmo desconhecido (PHP retorna `false` tambem nesses casos;
  RHP prefere erro explicito). Teto anti-DoS nos parametros (m<=64MiB,
  t<=16, p<=4) — PHP nao documenta teto equivalente.
- Truncamento bcrypt de 72 bytes herdado (igual ao PHP).

- Sintaxe comparada (`echo`, `;`, strings) apos ampliacao da linguagem.
- Sistema de tipos e conversoes.
- Semantica de `echo`/`print` vs PHP.
- Modelo de execucao (bytecode/VM proprios vs Zend).
- Biblioteca padrao e interoperabilidade (sem compatibilidade prometida).

Estado da Fase 1: `echo "s";` escreve `s\n`; programa e sequencia de `echo`s
de strings. Qualquer comparacao alem disso e especulativa e nao faz parte
do contrato.
