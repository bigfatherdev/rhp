# RHP — Estado atual: o que faz e o que falta

> Panorama consolidado em 2026-11-21 sobre a versão **0.7.2**
> (auditoria de engenharia; base anterior 0.7.1 em 2026-11-20).
> Este documento **consolida e aponta** (links para as fontes normativas);
> ele não duplica especificações. Em caso de divergência, valem
> `GOVERNANCE.md` > `LANGUAGE_SPEC.md` > demais specs > este arquivo.

**Leitura em 30 segundos:** RHP é uma linguagem de script interpretada em
Rust, com classes, geradores, async real, 85 builtins e conectores
(SQLite/Postgres/MySQL/Redis/TCP/HTTP). Suíte: **395 testes, 0 falhas**.
Faltam: TLS no MySQL, `fs_chown`/ACL, `and`/`or` e `in` (rejeitados por
decisão), builder de strings e unificar `db_exec(SELECT)`.

## Índice

- [1. Estado geral](#1-estado-geral)
- [2. Linguagem](#2-linguagem)
- [3. Biblioteca-padrão (85 builtins)](#3-biblioteca-padrao-85-builtins)
- [4. Módulos, pacotes e CLI](#4-modulos-pacotes-e-cli)
- [5. Concorrência](#5-concorrencia)
- [6. Persistência e rede](#6-persistencia-e-rede)
- [7. Servidor HTTP e framework web](#7-servidor-http-e-framework-web)
- [8. Segurança e limites](#8-seguranca-e-limites)
- [9. O que precisa correção](#9-o-que-precisa-correcao)
- [10. O que precisa melhoria](#10-o-que-precisa-melhoria)
- [11. Explicitamente fora de escopo](#11-explicitamente-fora-de-escopo)
- [12. Mapa de versões e próximos passos](#12-mapa-de-versoes-e-proximos-passos)

## 1. Estado geral

| Item | Estado (verificado) |
|---|---|
| Versão | `0.7.2` (`rhp --version`; 7 crates sincronizados) |
| Implementação | Rust estável (workspace `rhp-lexer/parser/bytecode/vm/fmt/diagnostic/cli`) |
| Suíte | 395 testes, 0 falhas; `fmt --check` e `clippy` limpos |
| Docs normativas | `LANGUAGE_SPEC.md`, `BYTECODE_SPEC.md`, `VM_SPEC.md`, `MEMORY_MODEL.md`, `ARCHITECTURE.md` |
| RFCs | 32 arquivos (`RFCs/0001`–`RFCs/0034`; sem 0013/0014) |
| Releases | `CHANGELOG.md` com seções 0.1.0 → 0.7.2 |

## 2. Linguagem

Tipos: `null`, `Bool`, `Int` (i64, aritmética checada), `Float`, `String`
(Unicode + escapes `\" \\ \n \r \t \u{HEX}`), `Array` e `Mapa` (por valor,
sem aliasing), função, classe, instância, gerador. Detalhe:
[`docs/REFERENCIA.md` §1](REFERENCIA.md#1-tipos-e-valores).

```rhp
let x = 1;
const NOME = "rhp";
fn soma(a, b = 10) { return a + b; }
echo soma(1);                        // 11
```

Operadores com curto-circuito (RFC 0031), estritamente `Bool` (E3003 fora
disso). Precedência (menor → maior): ternario < `||` < `&&` < `??` < `==`
< `|` < `^` < `&` < comparações < `<< >>` < `+ -` < `* / %` < unários.

```rhp
echo true && false || true;          // true
if ok && n > 0 || emergencia { }
```

Controle: `if/else`, `while`, `for x in` (array em ordem, mapa por chaves
ordenadas), `for` C, `switch` (frouxo, fallthrough) / `match` (estrito),
`break N`/`continue N`, `try/catch [var]/finally` + `throw`. Erro de runtime
capturado vira `{tipo: "runtime", mensagem: "..."}`; `exit()` nunca é
capturado; `return e;` dentro de `try` avalia `e` antes do `finally`.
Detalhe: [`docs/REFERENCIA.md` §4](REFERENCIA.md#4-controle-de-fluxo).

```rhp
try {
  echo json_parse("nope");
} catch e {
  echo e["tipo"];                    // runtime
}
```

Funções: defaults constantes, `...rest`, nomeados/spread, closures com
captura **por valor**, HOFs (`map`/`filter`/`usort` estável), anotações de
tipo checadas em runtime, generics com bounds apagados (RFCs 0025/0029).
Classes: herança simples, interfaces, traits, `super`, métodos `async`
com snapshot (sem write-back). Geradores lazy (`yield`/`next`/`exhausted`).
Detalhe: [`docs/REFERENCIA.md` §§5–8](REFERENCIA.md#5-funcoes-e-closures).

```rhp
class Contador {
  fn novo() { this["n"] = 0; }
  fn inc() { this["n"] = this["n"] + 1; return this["n"]; }
}
let c = new Contador();
echo c.inc();                        // 1
```

Limites sintáticos intencionais: profundidade máxima de 64 níveis (E1000),
arquivo > 8 MiB rejeitado, só comentários `//`.

## 3. Biblioteca-padrão (85 builtins)

Aridade checada na compilação; erros em português com código estável
(`E1001`, `E2002`, `E3000`–`E3012`). Contagem verificada no código
(ids 0–84). Detalhe: [`docs/REFERENCIA.md` §11](REFERENCIA.md#11-biblioteca-padrao-85-builtins).

| Categoria | Funções |
|---|---|
| Núcleo | `len push pop keys values entries merge has str int float type assert exit` |
| Strings | `contains starts_with ends_with split join trim upper lower replace` |
| Matemática | `abs min max sqrt pow floor ceil rand rand_range sort` |
| Alta ordem | `map filter usort` |
| JSON | `json_parse json_stringify` (teto 128 níveis) |
| Arquivos/ambiente | `fs_read fs_write fs_append fs_rename fs_remove fs_mkdir fs_chmod fs_stat fs_exists fs_list argv env now sleep date_str` |
| HTTP | `http_get http_fetch http_serve template` |
| Banco | `db_open db_exec db_query db_close` |
| Cripto | `hash_password verify_password sha256 hmac_sha256 hmac_verify regex_match regex_replace regex_split` |
| Geradores | `next exhausted` |
| Concorrência | `async_spawn async_join await_timeout async_cancel async_cancelled await_all await_race` |
| Rede | `tcp_connect tcp_send tcp_recv tcp_close redis_open redis_exec redis_close` |

```rhp
echo contains([1, 2, 3], 2);         // true
echo template("ola, {{nome}}!", {"nome": "Rex"});  // ola, Rex!
```

## 4. Módulos, pacotes e CLI

Módulos: `import "lib.rhp"`, seletivo `import {f} from`, re-export,
`rhp pkg` (v0, local, com `rhp.lock`), `rhp build` gera bundle autocontido.
CLI (`docs/CLI_REFERENCE.md`): `run` (`--max-steps`, `--max-heap-bytes`,
`--trace`), `repl`, `lsp` (stdio), `fmt`, `check`, `test`, `doctor`,
`init`, `pkg`, `build`, `--help/--version`. Exit codes: 0 ok, 1 erro,
2 uso inválido.

## 5. Concorrência

Tarefas isoladas em thread própria (1 thread/tarefa, sem pool): só código
é compartilhado, nunca estado; captura cruza só valores planos e instâncias
com classe. `await`/`async_join` interoperam; `await_timeout`,
`await_all`, `await_race`; cancelamento cooperativo. Detalhe:
[`docs/REFERENCIA.md` §9](REFERENCIA.md#9-concorrencia-rfcs-001900230030).

```rhp
fn trabalho() { return 42; }
let h = async_spawn("trabalho");
echo async_join(h);                  // 42
```

## 6. Persistência e rede

- **SQLite**: arquivo local, `?` posicionais (escape client-side), BLOB = erro.
- **PostgreSQL**: `$1...` nativos, tipos por OID, `Bool` com round-trip real.
- **MySQL**: protocolo 4.1 + `mysql_native_password` **e** `caching_sha2_password`
  (stage-1, fast_auth e full auth RSA-OAEP; validado contra 8.0 default).
- **Redis**: RESP2 sem AUTH; **TCP**: sockets com timeout.
- SQLite/MySQL não têm booleano nativo: `true` gravado volta como `1`.

```rhp
let h = db_open("/tmp/t.db");
echo db_exec(h, "CREATE TABLE t (id integer, nome text)", []);  // 0
echo db_exec(h, "INSERT INTO t VALUES (?, ?)", [1, "ana"]);     // 1
echo db_query(h, "SELECT * FROM t", []);   // [{id: 1, nome: ana}]
echo db_close(h);                          // null
```

## 7. Servidor HTTP e framework web

`http_serve(porta, handler, opts)`: thread por conexão, sem keep-alive.
Handler = nome de `fn` (string) ou valor de função; `req` =
`{metodo, caminho, query, cabecalhos, cookies, corpo}`; retorno `Str`
(corpo, 200) ou mapa `{status, cabecalhos, corpo}`. Opts: `host`
(default `127.0.0.1`; `0.0.0.0` = todas IPv4), `max_conexoes`,
`max_requisicoes`, `limite_taxa` (429), `max_passos` (default 10M),
`max_heap_bytes`; request malformado = 400; `chunked` suportado; erro de
handler = 500 opaco. Framework `framework/web.rhp`: rotas, params,
middleware, JSON, templates com escape. Detalhe: [`docs/WEB.md`](WEB.md).

## 8. Segurança e limites

Sem sandbox de filesystem (programas rodam com os privilégios do usuário —
`SECURITY.md`); teto de profundidade sintática 64 (E1000); corpo HTTP
≤ 1 MiB, headers ≤ 64 KiB, prazo total 30 s; orçamento de instruções/heap
por request; `unsafe` proibido fora de justificativa; segredos nunca
versionados.

## 9. O que precisa correção

Nada com severidade P0/P1 em aberto — todos os bugs confirmados das
auditorias foram corrigidos com regressão (ver `changes/` e
[`audit/0.6.1/FINDINGS.md`](../audit/0.6.1/FINDINGS.md),
[`audit/full-system/FINDINGS.md`](../audit/full-system/FINDINGS.md)):
parser SIGABRT em cadeias planas → teto 64; `return` em `try` não capturado;
HOF com `pending` obsoleto; MySQL sem `seq` por comando; dessincronia de
dreno pg/mysql; diagnósticos 2-arg. Reabrir qualquer item exige reprodução
nova.

## 10. O que precisa melhoria

| Item | Severidade | Status / dono |
|---|---|---|
| TLS no MySQL (verificação por padrão + pino) | P2 | [RFC 0034](../RFCs/0034-mysql-tls.md) (proposta, alvo 0.8.0); hoje: proxy TLS |
| `fs_chown` / ACL, `stat` estendido | P3 | Planejado 0.8.0+ |
| Builder de strings (tipo novo; `+=` é O(n²) — usar array+`join`) | P4 | Precisa RFC; padrão documentado |
| Unificar `db_exec(SELECT)` (MySQL erra, PG retorna tag) | P3 | Documentado; mudar quebraria compat |
| Fuzz de fonte versionado como teste de CI | P4 | [`tools/fuzz-smoke.sh`](../tools/fuzz-smoke.sh) existe; falta gate no CI |

## 11. Explicitamente fora de escopo

Com motivo registrado (não são esquecimento): `and`/`or` e operador `in`
(rejeitados nas RFCs 0031/0032 — sintaxe duplicada sem ganho); truthiness
(`if`, `&&`, `filter` exigem `Bool` de verdade); sem `private`/estáticos;
sem keep-alive; resposta HTTP só texto (sem corpo binário); trailers de
`chunked` descartados; closures não servem de handler HTTP.

## 12. Mapa de versões e próximos passos

`0.1.0` fundação → `0.2.0`–`0.3.0` DevOps/segurança → `0.4.0` Next
Generation → `0.5.0` bounds/async-trie → `0.6.0` robustez HTTP/return →
`0.6.1` parser 64 + MySQL seq → `0.6.2` HOF pending + dreno + fs base →
`0.7.0` `&&`/`||`, `contains` array, `caching_sha2` (+`tools/fuzz-smoke.sh`)
→ **`0.7.1`** `fs_chmod`/`fs_stat`, RFC TLS proposta
→ **`0.7.2`** auditoria (5 P0 + 9 P1/P2 zerados, 20 regressões novas):
corpo `for` único (linear), `yield`-em-`try` correto, exibição com teto
(E3000), `next`+512, HOF só-RHP, `catch` sem 256, `http_serve` filtrado no
topo, `?` MySQL literal-aware, tetos RESP/ncol, `RHP_CA_BUNDLE=""`,
userinfo rejeitado, `json_parse` O(n) (~43x), REPL multilinha,
`test` recursivo, docs honestas. Detalhe por release:
[`CHANGELOG.md`](../CHANGELOG.md). Roadmap:
[`ROADMAP.md`](../ROADMAP.md). Candidatos a 0.8.0: §10, priorizados por
uma nova rodada de auditoria — não por lista de desejos.
