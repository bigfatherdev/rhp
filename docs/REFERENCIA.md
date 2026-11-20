# RHP — Referência completa da linguagem e da biblioteca-padrão

> Versão desta referência: acompanha o RHP `0.3.0` (`rhp --version`).
> Fonte normativa continua sendo `LANGUAGE_SPEC.md` (+ specs);
> aqui vai o panorama consolidado: sintaxe, todas as funções built-in,
> erros, CLI e exemplos.

## Índice rápido

- [Tipos e valores](#1-tipos-e-valores)
- [Literais, variáveis e constantes](#2-literais-variaveis-e-constantes)
- [Operadores](#3-operadores)
- [Controle de fluxo](#4-controle-de-fluxo)
- [Funções e closures](#5-funcoes-e-closures)
- [Anotações de tipo](#6-anotacoes-de-tipo-rfc-0016)
- [Classes e objetos](#7-classes-e-objetos-rfc-0017)
- [Geradores](#8-geradores-rfc-0018)
- [Concorrência](#9-concorrencia-rfc-0019)
- [Módulos e pacotes](#10-modulos-e-pacotes)
- [Biblioteca-padrão (80 builtins)](#11-biblioteca-padrao-80-builtins)
- [Erros e códigos](#12-erros-e-codigos)
- [CLI](#13-cli)
- [Exemplo completo](#14-exemplo-completo)

## 1. Tipos e valores

| Tipo RHP | `type()` | Exemplos |
|---|---|---|
| `null` | `"null"` | `null` |
| `Bool` | `"bool"` | `true`, `false` |
| `Int` | `"int"` | `42`, `-7` (i64, aritmetica checada) |
| `Float` | `"float"` | `1.5` (sempre com digitos apos o ponto) |
| `String` | `"string"` | `"ola"`, `"a\nb"`, `"\u{1F600}"` |
| `Array` | `"array"` | `[1, 2]`, `[]` (por valor/copia, sem aliasing) |
| `Mapa` | `"mapa"` | `{x: 1}`, `{}` (chaves ordenadas na impressao) |
| Funcao | `"funcao"` | `fn (x) { return x; }` |
| Classe | `"classe"` | `class Contador { ... }` |
| Instancia | `"instancia"` | `new Contador()` |
| Gerador | `"gerador"` | `fn g() { yield 1; }` + `g()` |

Semântica de valores: atribuicao, argumentos, retorno e `push` copiam
integralmente. Mutacao observavel so existe em variaveis nomeadas
(`a[i] = v`, `obj.campo = v`, `obj.metodo()` com write-back).

## 2. Literais, variáveis e constantes

```rhp
let x = 1;          // declaracao (mutavel)
let y;              // inicializa com null
const NOME = "rhp"; // imutavel (reatribuir = erro)
x = x + 1;          // atribuicao exige variavel visivel
```

Identificadores: `[a-zA-Z_][a-zA-Z0-9_]*`. Strings com escapes
`\" \\ \n \r \t \u{HEX}` (1–6 digitos, Unicode valido).

Palavras reservadas: `let const fn if else while for switch case
default match break continue try catch finally throw return import
export new class yield echo true false null`
(contextuais, i.e. ainda usaveis como identificadores: `in`, `from`,
`as`; `this` e um identificador comum vinculado pelo metodo).

## 3. Operadores

Precedencia (da menor para a maior): ternario `a ? b : c` (a direita)
< Elvis/?? (`a ?: b` = `a ?? b`; so `null` aciona)
< `==`, `!=` < `|` < `^` < `&` < `< <= > >=` < `<< >>` < `+ -` <
`* / %` < unarios `! - ~` (+ unario e identidade) < primario.

- `/` e `%` por zero (int ou float) sao erro; estouro de `Int` e erro.
- `Int + Float` promove para `Float`.
- `Str + Str` concatena; demais misturas sao erro (use `str()` etc.).
- `%` exige `Int` e `Int`; `-x` exige numero; `!x` exige `Bool`.
- Comparacao `< <= > >=` aceita numero-numero (com promocao); demais
  combinacoes sao erro. `==`/`!=` comparam por valor.

## 4. Controle de fluxo

```rhp
if condicao { } else { }            // condicao deve ser Bool
while condicao { }                  // reavalia a cada iteracao
for x in [3, 1, 2] { echo x; }      // array: em ordem
for k in {b: 1, a: 2} { echo k; }   // mapa: chaves ordenadas (a, b)
for let i = 0; i < 3; i = i + 1 { } // for-C (partes opcionais)
for ;; { break; }                   // infinito
for x in gen() { echo x; }          // gerador: consome sob demanda
break; continue; break 2; continue 2; // N = niveis para fora (padrao 1)
```

- `break`/`continue` fora de laco/`switch` = erro de compilacao.
- `if`/`while` aninhados criam escopo; `let` interno nao vaza.
- `switch v { case 1 { ... } default { ... } }`: comparacao frouxa
  `==`, fallthrough sem `break`, `default` unico em qualquer posicao.
- `match e { 1 => "um", default => "outro" }`: expressao, igualdade
  estrita, sem fallthrough.
- `try { } catch e { } finally { }` (ao menos um de `catch`/`finally`
  obrigatorio); `throw v` lanca; nao capturado = `excecao nao capturada`.
  Erro de runtime capturado vira mapa `{tipo: "runtime",
  mensagem: "..."}`; `exit()` nunca e capturado. `return e;` dentro do
  `try` tem `e` avaliado antes do `finally`: erros de `e` (ex.:
  `return json_parse(s);`) sao capturados pelo `catch` ativo.

## 5. Funcoes e closures

```rhp
fn soma(a, b = 10) { return a + b; }
echo soma(1);        // 11
echo soma(1, 2);     // 3
fn junta(primeiro, ...resto) { return resto; }
echo junta(1, 2, 3); // [2, 3]
let dobra = fn (x, fator = 2) => x * fator; // arrow: corpo e o retorno
```

- Defaults constantes (`b = 1 + 2` dobra na compilacao); opcionais por
  ultimo; `...rest` coleta excedentes (sempre por ultimo).
- Chamada mista: `f(b: 2, a: 40)`, `f(...[1, 2])` (`CallMixed`).
- Closures capturam o ambiente **por valor** (snapshot na criacao);
  globais sao lidos ao vivo na chamada (RFC 0003).
- HOFs: `map([1,2,3], fn (v) => v * 2)`, `filter`, `usort` (estavel).

## 6. Anotacoes de tipo (RFC 0016)

```rhp
let x: int = 1;
const N: string = "rhp";
fn soma(a: int, b: int): int { return a + b; }
let dobro = fn (v: int): int { return v * 2; };
```

Tipos: `int | float | string | bool | array | mapa | null | funcao`
(nomes de `type()`). Checagem em **runtime** no vinculo (declaracao,
argumento apos defaults, retorno): mismatch = erro
`tipos incompativeis: esperava T, encontrou V` (E3001, capturavel).
`let x: int;` falha (nulo implicito nao satisfaz `int`). Sem anotacao,
zero checagem (nenhuma instrucao nova). `static`/destructuring sem
anotacao.

### Generics com bounds (RFCs 0025/0029)

```rhp
class Caixa<T> {
  fn novo(v) { this["v"] = v; }
  fn obter() { return this["v"]; }
}
let c = new Caixa<int>(100);   // ok (apagado; sem custo em runtime)
fn primeiro<T>(items) { return items[0]; }

class Entidade { fn novo() { this["id"] = 0; } fn id() { return this["id"]; } }
class Usuario extends Entidade { fn novo() { this["id"] = 1; } }
class Repo<T: Entidade> {      // bound: classe ou interface conhecida
  fn novo() { this["x"] = 0; }
}
let r = new Repo<Usuario>();   // ok (subclasse satisfaz)
```

- Parametros `<T, U: Bound>` em `fn`/`class`/`interface`; uso
  `f<T>(...)` / `new C<T>(...)` guarda os argumentos de tipo.
- Checagem em **compilacao** (apagamento mantido): aridade; nao-generica
  com args = erro; builtin com args = erro; bound violado
  (`` `Outra` nao satisfaz `T: Entidade` ``), bound desconhecido e tipo
  desconhecido (inclui primitivas como `int` contra bound) = erro.
- Sem argumentos de tipo = apagado, sem checagem (`new Caixa(v)`, `f(x)`).
  Parametro de tipo visivel adia a checagem (`new Caixa<T>` dentro de
  `fn f<T>`; erro so na instanciacao concreta).

## 7. Classes e objetos (RFCs 0017/0021)

```rhp
class Contador {
  fn novo() {           // construtor (obrigatorio para `new`)
    this["n"] = 0;      // `this` e a instancia em construcao/uso
  }
  fn inc() {
    this["n"] = this["n"] + 1;
    return this["n"];   // retorno normal; `new` descarta e devolve a instancia
  }
}
let c = new Contador(); // exige `novo`; sem `novo` = erro
echo c.inc();           // 1 (persiste: write-back na variavel)
echo c.inc();           // 2
echo c.n;               // 2 (campo)
c.n = 10;               // atribuicao em campo
```

- `obj.metodo(args)`: procura campo, senao metodo (vinculado a `this`).
  Chamado direto na **variavel**, mutacoes de `this` persistem
  (write-back; coerente com `a[i] = v`). Metodo destacado
  (`let f = obj.m; f()`) nao persiste. Metodo inexistente = erro.
- `new` com aridade errada = erro normal de chamada; `new` em nao-classe
  = erro; `this` fora de metodo = `nao definida`.
- Instancias sao por valor (copia; sem aliasing). Sem `private` nem
  estaticos (nao-objetivos declarados).
- `echo` mostra `{n: 2}`; classe mostra `<classe Contador>`.
- `==`: instancias comparam por estrutura (mesma classe + mesmos campos);
  classes comparam por nome; geradores, por identidade.

### Heranca, interfaces e traits (RFC 0021)

```rhp
class Animal {
  fn novo(n) { this["nome"] = n; }
  fn falar() { return "Som"; }
}
class Cachorro extends Animal {   // heranca simples (pai definido antes)
  fn falar() { return "Au au"; } // sobrescrita
}
let c = new Cachorro("Rex");
echo c.falar();                  // "Au au"

interface Logger { fn log(mensagem); }
class ConsoleLogger implements Logger {  // contrato verificado em compilacao
  fn novo() { this["x"] = 0; }
  fn log(m) { echo m; }
}

trait Saudacao {
  fn ola(nome) { return "ola " + nome; }
}
class Pessoa {
  use Saudacao;                  // composicao
  fn novo() { this["x"] = 0; }
}

class Filha extends Base {
  fn quem() { return super.quem() + "!"; }  // metodo do pai, mesmo `this`
}
```

- `extends`: unico; pai desconhecido, auto-heranca e ciclo = erro de
  compilacao. Metodos achatados (propria > trait > pai).
- `implements I, J`: todo metodo exigido deve existir com mesma aridade
  (ou e erro `nao implementa`).
- `use T1, T2;` no corpo: conflito trait-trait sem override na classe
  = erro (`conflito de trait`).
- `super.metodo(args)`: so em chamada de metodo; pai ausente ou sem o
  metodo = erro em runtime; write-back no `this` do chamador.

### Metodos `async` (snapshot, sem write-back)

```rhp
class Api {
  fn novo() { this["x"] = 0; }
  async fn buscar(id) { sleep(0.1); return "dado " + str(id); }
}
let api = new Api();
let h = api.buscar(1);   // gera tarefa na hora (nao bloqueia)
echo await h;            // "dado 1"
```

- `async fn` vale em classe e trait, para metodo, chamada destacada
  (`let f = obj.m`) e `super.m()`: a chamada **gera tarefa** e devolve
  handle (compativel com `await`/`async_join`).
- Semantica de **snapshot**: `this` viaja como copia (com a classe);
  mutacoes **nao voltam** ao chamador — devolva valores com `return`.
- Restricoes: sem named/spread em chamada async; funcao/classe/gerador
  como argumento = erro; mapa com chave `__rhp_instancia__` = erro
  (chave reservada ao envelope de instancia).

## 8. Geradores (RFC 0018)

```rhp
fn cont(n) {
  let i = 0;
  while i < n {
    yield i;            // suspende e devolve; `yield;` devolve Null
    i = i + 1;
  }
}
for x in cont(3) { echo x; }   // 0 1 2 (lazy, sem materializar)
let g = cont(10);
echo next(g);                  // 0 (retoma ate o proximo yield)
echo exhausted(g);              // false
```

- Chamar nao executa: devolve o gerador lazy (corpo roda no 1o `next`).
- `return` dentro de gerador descarta o valor e encerra (`Null` ao fim).
- `for x in gen` consome (`break`/`continue` funcionam; gerador
  esgotado no inicio = zero iteracoes; infinito + `break` = ok).
- `next` em gerador esgotado = erro (teste `exhausted` antes).
- Erro dentro do gerador: `try` externo captura; resume seguinte falha
  ("interrompido"; crie outro). `this` de metodo-gerador e vinculado na
  criacao (sem write-back). `yield` fora de funcao = erro de compilacao.

## 9. Concorrencia (RFCs 0019/0023/0030)

```rhp
fn lenta() { sleep(1); return "pronta"; }
let h = async_spawn("lenta");   // Int (handle); roda em thread propria
echo async_join(h);              // bloqueia; devolve o retorno ("pronta")

async fn buscarA() { sleep(0.2); return "A"; }
let a = buscarA();               // gera tarefa na hora (nao bloqueia)
echo await a;                    // "A"
echo await_timeout(a, 5);         // com prazo (estouro = erro capturavel)
async_cancel(a);                 // cooperativo (a tarefa sonda async_cancelled())
echo await_all([a(), b()]);      // junta em ordem
echo await_race([a(), b()]);     // primeiro pronto vence
```

- Tarefas isoladas: compartilham o CODIGO (definicoes de topo: funcoes,
  classes, `import native`), nao o ESTADO (globais mutaveis nao atravessam).
  Captura cruza so valores planos e instancias (com classe, via envelope
  `{"__rhp_instancia__": {"classe", "campos"}}`); funcao, classe e gerador
  nao atravessam (erro claro); mapa com a chave reservada = erro.
  Metodos `async` rodam sobre snapshot (sem write-back; devolva valores
  com `return`).
- Args posicionais (nao nomeados/spread); funcao sem captura vira tarefa;
  `await`/`async_join` interoperam (handles one-shot, exceto apos timeout:
  handle consumido vira `Int` comum e `await` dele = identidade).
- Erro da tarefa vira erro do `join` (`tarefa ... falhou: ...`).
- Cancelamento e cooperativo (`async_cancel` + `async_cancelled`); timeout
  nao mata a tarefa (segue em voo; junte depois). Sem thread-pool (1 thread
  por tarefa; documentado).
- Paralelismo real: 2x `sleep(1)` em ~1s (nao ~2s).

## 10. Modulos e pacotes

```rhp
// lib.rhp
export fn dobro(x) { return x + x; }
export let base = 40;
```

```rhp
// main.rhp
import "lib.rhp";                       // bare: vincula tudo
import {dobro, base as b} from "lib.rhp"; // seletivo (privacidade real)
import {u} from "./c1.rhp";
```

- `export { a, b as c } from "mod.rhp"` faz re-export transitivo
  (sem `export *`). `export fn/let/const` marca o exportavel.
- `rhp pkg`: `rhp init`, `rhp pkg add <nome> <caminho>`,
  `rhp pkg install [--check]`, `rhp.lock`, `rhp build` (bundle).
- Comentarios `//` de linha e linhas em branco sao preservados pelo
  `rhp fmt` na mesma ordem relativa ao statement.

## 11. Biblioteca-padrao (80 builtins)

Aridade checada na compilacao; erros sao `VmError` com mensagem em
portugues (ver [codigos](#12-erros-e-codigos)).

### Nucleos (tipos, colecoes, controle)

| Funcao | Efeito |
|---|---|
| `len(x)` | Tamanho: array, mapa (`len({})`) ou string (caracteres). |
| `push(arr, v)` | Novo array com `v` ao fim (nao muta o original). |
| `pop(arr)` | Ultimo elemento; array vazio = erro. |
| `keys(mapa)` | Array de chaves ordenado. |
| `values(m)` | Array com os valores (ordem das chaves). |
| `entries(m)` | Array de pares `[chave, valor]` (ordem das chaves). |
| `merge(a, b)` | Une dois mapas; chaves de `b` vencem. |
| `has(m, chave)` | `true` se a chave existe. |
| `str(v)` | Representacao `Display` (mapa ordenado por chave). |
| `int(v)` | `Int`->igual; `Float` integral->trunca (faixa `i64`); `Str`->parse; resto = erro. |
| `float(v)` | `Float`->igual; `Int`->converte; `Str`->parse; resto = erro. |
| `type(v)` | `"null"`, `"bool"`, `"int"`, `"float"`, `"string"`, `"array"`, `"mapa"`, `"funcao"`, `"classe"`, `"instancia"`, `"gerador"`. |
| `assert(cond)` | `true` -> `null`; `false`/nao-`Bool` = erro (`assercao`). |
| `exit(codigo)` | Encerra com codigo 0–255 (fora disso ou nao-`Int` = erro). Desenrola chamadas; em `rhp test` conta como falha. |

### Strings

| Funcao | Efeito |
|---|---|
| `contains(s, parte)` | `true` se `parte` ocorre em `s` (ambos `Str`). |
| `starts_with(s, prefixo)` / `ends_with(s, sufixo)` | Prefixo/sufixo (`Bool`). |
| `split(s, sep)` | `Array` de pedacos; separador vazio divide em caracteres. |
| `join(arr, sep)` | Junta `Array` de `Str`; nao-`Str` = erro. |
| `trim(s)` | Remove espacos das pontas (Unicode). |
| `upper(s)` / `lower(s)` | Caixa alta/baixa (Unicode). |
| `replace(s, de, para)` | Substitui todas as ocorrencias. |

### Matematica

| Funcao | Efeito |
|---|---|
| `abs(x)` | Valor absoluto (`Int` com checagem de estouro). |
| `min(a, b)` / `max(a, b)` | Minimo/maximo (misto vira `Float`). |
| `sqrt(x)` | Raiz quadrada (`Float`); negativo = erro. |
| `pow(base, exp)` | Potencia (`Float`; aceita numericos). |
| `floor(x)` / `ceil(x)` | Para baixo/cima (`Int`); fora de `i64` = erro. |
| `rand()` | `Int` em `[0, i64::MAX]` (xorshift64*; **NAO criptografico**). |
| `rand_range(min, max)` | `Int` em `[min, max]`; `min > max` ou nao-`Int` = erro. |
| `sort(arr)` | Copia ordenada (homogeneo: numeros, strings ou bools). Nao muta. |

### Funcoes de alta ordem (RFC 0005)

| Funcao | Efeito |
|---|---|
| `map(arr, f)` | Aplica `f(elem)` e devolve array de resultados (mesma ordem). |
| `filter(arr, f)` | Devolve elementos cujo `f(elem)` retorna `true` (sem truthiness). |
| `usort(arr, cmp)` | Ordenacao estavel por `cmp(a, b): Int` (`< 0`: `a` antes; `> 0`: depois; `0`: empate). |

### JSON

| Funcao | Efeito |
|---|---|
| `json_parse(texto)` | Texto JSON => valor (`null`/`Bool`/`Int`/`Float`/`Str`/`Array`/`mapa`). Gramatica estrita; erro com posicao (`json_parse`). Objeto: primeira chave vence; teto de 128 niveis. |
| `json_stringify(valor)` | Valor => texto JSON canonico (chaves ordenadas, sem espacos; `2.0` para floats inteiros). `NaN`/infinito e funcao = erro. |

### Arquivos e ambiente

| Funcao | Efeito |
|---|---|
| `fs_read(caminho)` | Conteudo do arquivo (`Str` UTF-8); I/O ou > 8 MiB = erro. |
| `fs_write(caminho, texto)` / `fs_append(caminho, texto)` | Escreve/anexa; devolve bytes escritos. Relativos resolvem no diretorio atual. |
| `fs_exists(caminho)` | `true` se legivel (qualquer falha = `false`). |
| `fs_list(dir)` | Nomes de entradas ordenados (`[]` se vazio); inexistente/nao-diretorio = erro. |
| `argv()` | `Array` de `Str` com os argumentos apos o arquivo. |
| `env(nome)` | Valor da variavel de ambiente ou `null` se ausente. |
| `now()` | Segundos Unix (`Int`). |
| `sleep(s)` | Bloqueia `s` segundos (`Int`/`Float`); negativo/NaN/>3600 = erro; devolve `null`. |
| `date_str(ts)` | Epoch `Int` => `"AAAA-MM-DDTHH:MM:SSZ"` (UTC). |

### Rede HTTP (Fases 6/A/C)

| Funcao | Efeito |
|---|---|
| `http_get(url)` | GET `http://` com timeouts de 5s (DNS: resolvedor do sistema), teto de corpo 1 MiB, so 2xx com corpo UTF-8; resto = erro. |
| `http_fetch(url, opts)` | GET com `opts` (`seguir`, `max_redirecionamentos` 0–10, `timeout` 0–60s, `headers`, `metodo`: so `GET`); `https://` verificado (raizes embutidas + `RHP_CA_BUNDLE`); redirects 301/302/303/307/308 (ate 10; sem downgrade https→http); devolve mapa `{status, cabecalhos, corpo}` (chaves minusculas). |
| `http_serve(porta, handler, opts)` | Servidor HTTP (thread por conexao; handler stateless; erro 500 sem vazar; `exit(codigo)` encerra). `porta` 0–65535 (0 = livre). Handler: nome de `fn` (string) ou valor de funcao do topo (`app`, nao closure). `req` = mapa `{metodo, caminho, query, cabecalhos, cookies, corpo}`; retorno `Str` (corpo, 200) ou mapa `{status?, cabecalhos?, corpo?}`. Opts: `host` (default `"127.0.0.1"`; `"localhost"` = idem; `"0.0.0.0"` = todas IPv4 — ver seguranca em `docs/WEB.md`), `max_conexoes`, `max_requisicoes`, `limite_taxa` (req/s por IP; excedeu = 429), `max_passos` (default 10M; estouro = 500), `max_heap_bytes`. Request malformado = 400; corpo `chunked` suportado; `http_serve` no handler = erro; falha de `accept` nao para o servidor. |
| `template(texto, valores)` | Interpola `{{chave}}` com escape HTML obrigatorio; chave ausente/`{{` aberto/tipo invalido = erro; sem laco/condicional. |

### Banco de dados (Fase B + Etapa 5)

| Funcao | Efeito |
|---|---|
| `db_open(caminho)` | Abre/cria SQLite (`Int` = handle); relativo resolve no cwd; roda com as permissoes do usuario. |
| `db_open("pg:...")` | Conecta no PostgreSQL (`host`, `port`, `user`, `dbname`; auth `trust`; padroes host 127.0.0.1/porta 5432/user `postgres`/dbname = user). |
| `db_open("mysql:...")` | Conecta no MySQL (protocolo 4.1 + `mysql_native_password`; chaves `host`, `port` (padrao 127.0.0.1:3306), `user` (padrao `root`), `password`, `dbname`). |
| `db_exec(h, sql, params)` | Uma instrucao (INSERT/UPDATE/DELETE/DDL...); `?` posicionais (SQLite/MySQL via escape client-side; PG `$1`... nativo); devolve linhas afetadas (`Int`). Linhas de SELECT sao descartadas. |
| `db_query(h, sql, params)` | Consulta; devolve array de mapas coluna=>valor (`Int`/`Float`/`Bool`/`Str`/`null`; BLOB SQLite = erro; tipos PG mapeados por OID, resto vira `Str`; MySQL: numericos=>numero, resto (incl. datas)=>`Str`). |
| `db_close(h)` | Fecha (reuso = erro); devolve `null`. Fim do programa fecha o resto. |

### Criptografia e hashing (RFCs 0013/0014)

| Funcao | Efeito |
|---|---|
| `hash_password(senha)` | Argon2id (`$argon2id$v=19$...`, defaults OWASP); `Str` requerida. |
| `verify_password(senha, hash)` | `Bool`; aceita `$argon2id$` e bcrypt `$2a$`/`$2b$`/`$2x$`/`$2y$` (migracao de bancos PHP); resto = erro; teto anti-DoS (`m<=65536`, `t<=16`, `p<=4`). |
| `sha256(s)` | Hex lowercase de SHA-256 (puro Rust). |
| `hmac_sha256(chave, msg)` | Tag hex HMAC-SHA256 (comparacao em tempo constante via `hmac_verify`). |
| `hmac_verify(chave, msg, tag)` | `Bool`; tag fora do formato hex = erro. |
| `regex_match(s, padrao)` | `Bool`; tempo linear garantido; sem lookaround/backrefs (erro na compilacao do padrao). |
| `regex_replace(s, padrao, subst)` | `Str`; grupos `$1`/nomeados `$name` na substituicao. |
| `regex_split(s, padrao)` | `Array` de `Str` (vazias incluidas). |

### Geradores (RFC 0018)

| Funcao | Efeito |
|---|---|
| `next(gen)` | Retoma ate o proximo `yield` (devolve o valor); no fim, marca esgotado e devolve `Null`. Gerador esgotado = erro (teste `exhausted` antes). Nao-gerador = erro. |
| `exhausted(gen)` | `Bool`: `true` se terminou (`false` antes de qualquer `next`). Nao-gerador = erro. |

Padrao manual: `while !exhausted(g) { let v = next(g); if exhausted(g) { break; } ... }` (o `next` final entrega `Null` descartavel).

### Concorrencia (RFC 0019)

| Funcao | Efeito |
|---|---|
| `async_spawn("f")` | Roda `fn` de nivel superior sem captura em thread propria (recompila o programa; definicoes de topo reconstruidas no worker); devolve handle `Int` imediatamente. Gerador/closure/nao-funcao = erro. |
| `async_join(h)` | Bloqueia ate terminar; devolve o retorno (desserializado de JSON + instancias com classe; resto = erro). One-shot (reuso = `handle invalido`). Erro da tarefa = `tarefa ... falhou: ...`. |
| `await_timeout(h, s)` | Como `await`, com prazo em segundos (0–3600; `0` = sondagem); estouro = erro capturavel e a tarefa segue em voo (junte depois). |
| `async_cancel(h)` | Sinaliza cancelamento cooperativo (`Null`); a tarefa precisa sondar `async_cancelled()`. |
| `async_cancelled()` | `Bool`: `true` dentro de tarefa cancelada; `false` fora de tarefa. |
| `await_all([h, ...])` | Junta em ordem, devolve array; falha rapida (demais seguem no mapa). |
| `await_race([h, ...])` | Devolve o primeiro pronto (demais seguem no mapa); vazio = erro. |

### Sockets e Redis (RFC 0020)

| Funcao | Efeito |
|---|---|
| `tcp_connect(host, porta)` | Conecta (DNS do sistema; timeout 5s); devolve handle `Int`. |
| `tcp_send(h, dados)` | Escreve `Str`; devolve bytes escritos (`Int`). |
| `tcp_recv(h, max)` | Le ate `max` bytes; devolve `Str` (nao-UTF-8 = erro) ou `Null` em EOF limpo. |
| `tcp_close(h)` | Fecha; devolve `null` (reuso = erro). |
| `redis_open(host, porta)` | Conecta (RESP2; sem AUTH v1 — servidor com senha = erro). Devolve handle `Int`. |
| `redis_exec(h, [cmd, args...])` | Executa; simples/bulk => `Str`, inteiro => `Int`, nulo => `null`, array => `Array` (recursivo, teto 128). Erro RESP (`-...`) vira erro RHP; comando vazio/arg nao-`Str` = erro. |
| `redis_close(h)` | Fecha; devolve `null` (reuso = erro). |

## 12. Erros e codigos

Lexicos exibem `linha:coluna` (base 1). Runtime exibe mensagem; erros
`try`-capturaveis viram `{tipo: "runtime", mensagem: "..."}`.
Exemplos: `esperado ';'`, `tipos incompativeis: esperava int, encontrou
string`, `variavel nao definida: x`, `condicao deve ser bool`,
`indice 5 fora do intervalo (tamanho 2)`, `chave inexistente no mapa: k`,
`heap excedeu o limite (512 bytes)`, `gerador esgotado`,
`` `new` exige classe ``, `membro inexistente`.

Codigos estaveis (prefixo `Error[CODIGO]` na CLI):
`E1001` (lexico), `E1004` (sintaxe `;`), `E2000`–`E2004` (compilacao),
`E3000` (runtime generico), `E3001` (tipos), `E3002` (nome indefinido),
`E3004` (aridade), `E3007` (indice/chave), `E3009` (import),
`E3010` (teto de instrucoes), `E3011` (http), `E3012` (teto de heap).
Erros fatais (ex.: bytecode invalido em arquivo `.rhp`? nao — todo
programa passa pelo validador) nunca exibem pilha interna.

## 13. CLI

| Comando | Efeito |
|---|---|
| `rhp run [--max-steps N] [--max-heap-bytes N] [--trace] <arq> [args]` | Executa (resolve `import`s); exit 0 ok / 1 erro / 2 uso invalido. |
| `rhp repl` | REPL interativo com estado (`sair`/EOF sai com 0). |
| `rhp lsp` | Servidor LSP stdio (initialize/didOpen/didChange/shutdown/exit). |
| `rhp fmt [--check] <arq>` | Formata no estilo canonico idempotente (preserva comentarios). |
| `rhp check [--json] <arq>` | Lex + imports + compilacao **sem executar**. |
| `rhp test [dir\|arq]` | Roda `*_test.rhp` (assert; exit != 0 = falha). |
| `rhp doctor` | Autodiagnostico do ambiente. |
| `rhp init [dir]` / `rhp pkg add/install` / `rhp build` | Pacotes v0 + bundle autocontido. |
| `rhp --help`, `rhp --version` | Ajuda e versao (`rhp 0.3.0`). |

Detalhes operacionais (Docker, CI, publicacao): ver `docs/DOCKER.md`.

## 14. Exemplo completo

```rhp
// Contador com estado, tipos e funcao de alta ordem.
class Contador {
  fn novo() {
    this["n"] = 0;
  }
  fn inc() {
    this["n"] = this["n"] + 1;
    return this["n"];
  }
}

fn main() {
  let c = new Contador();
  echo c.inc(); // 1 (write-back: a variavel e atualizada)
  echo c.inc(); // 2
  // Closures capturam por valor: `c` aqui dentro e uma copia congelada.
  let vals = map([1, 2, 3], fn (x) => c.inc() + x);
  echo vals;   // [4, 5, 6] (inc sempre parte do `c` capturado, {n: 2})
  echo c["n"]; // 2 (o `c` original nao foi tocado pelo map)
}

main();
```

## Divergencias honestas do PHP

Sem `&&`/`||` (so bitwise); `?:`/`??` sem truthiness (so `null`
aciona); `for x in mapa` itera chaves ordenadas; `for-in` nao tem
`&$ref`; `define()`/variaveis dinamicas nao existem; OO sem heranca;
geradores com `==` por identidade; tarefas sem memoria compartilhada;
`rand()` nao e criptografico (use Argon2id/bcrypt p/ senhas).
Divergencias completas com motivacao: ver `PHP_DIFFERENCES.md`.
