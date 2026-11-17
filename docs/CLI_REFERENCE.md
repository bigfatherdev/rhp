# CLI Reference — `rhp`

Uso geral: `rhp <comando> [opcoes] <arquivo.rhp>`

## `rhp run [--max-steps N] [--max-heap-bytes N] [--trace] <arquivo.rhp> [args...]`

Executa o programa (resolve `import`s antes de compilar). Argumentos apos
o arquivo vao para `argv()` (ex.: `rhp run prog.rhp a b` => `argv()` e
`["a", "b"]`).

| Codigo de saida | Significado |
|---|---|
| 0 | Sucesso (ou `exit(0)`) |
| 1 | Erro de arquivo, lexico, sintaxe, compilacao ou execucao (`arquivo:linha:coluna: mensagem` no stderr) |
| 2 | Uso invalido (argumentos errados, `--max-steps` nao numerico) |
| N (3–255) | `exit(N)` no programa (sem mensagem; stdout anterior preservado) |

`--max-steps N` aborta apos N instrucoes (`limite de instrucoes excedido`).
`--max-heap-bytes N` limita a heap estimada (RFC 0015).
`--trace` despeja `f<frame>:<ip> <op>` em stderr a cada instrucao.
Exemplos:

```bash
rhp run hello.rhp
rhp run --max-steps 1000 loop.rhp
rhp run --trace fib.rhp 2> trace.log
```

Erros de runtime mostram a pilha de chamadas (`  em f`, da mais interna
para a mais externa); sem frames nomeados, sem pilha.

## `rhp repl` (Etapa 4)

REPL interativo: le linhas do stdin, compila e executa cada uma com
estado persistente (globais e funcoes sobrevivem; erro nao mata a
sessao). Prompt `> ` so em terminal; `sair` ou EOF encerra com 0.

```bash
rhp repl
> let x = 40;
> echo x + 2;
42
```

## `rhp lsp` (Etapa 4)

Servidor LSP minimo sobre stdio (JSON-RPC + `Content-Length`):
`initialize`, `textDocument/didOpen`, `textDocument/didChange`,
`shutdown`/`exit`; publica `textDocument/publishDiagnostics` com erros
lexicos/sintaticos (faixa de 1 caractere no ponto do erro).

## Variaveis de ambiente

| Variavel | Efeito |
|---|---|
| `RHP_CA_BUNDLE` | Caminho de PEM com certificados extras aceitos pelo `http_fetch` em `https://` (somado as raizes embutidas; ex.: CA corporativa de proxy). Ausente ou vazio = so raizes embutidas. |

## `rhp fmt [--check] <arquivo.rhp>`

Reescreve no formato canonico (2 espacos, `;` sempre, parenteses minimos,
`\n` final unico; idempotente). `--check` so verifica (exit 1 se divergir).

## `rhp test [dir|arquivo.rhp]`

Executa arquivos de teste (`*_test.rhp`, que usam `assert`): um por um,
com relatorio `ok <arquivo>` / `FALHOU <arquivo>: <motivo>` e resumo final
(`N teste(s), M falha(s)`). Exit 0 se todos passam, 1 caso contrario.
`exit(codigo)` dentro de um teste conta como falha do arquivo (nao encerra
o corredor). Sem argumento, usa o diretorio atual.

## `rhp doctor`

Diagnostico do ambiente: versao do `rhp`, versoes de `rustc`/`cargo` (ou
"nao encontrado no PATH") e autoteste em memoria (`6 * 7 == 42`). Exit 0
se o autoteste passa.

## `rhp init [dir]` e `rhp pkg ...` (pacotes v0, locais)

```bash
rhp init meuapp                # cria rhp.toml
rhp pkg add tempo ../tempo     # registra dependencia (caminho local)
rhp pkg install                # vendoriza em pacotes/ + grava rhp.lock
rhp pkg install --check        # verifica pacotes/ contra o lock (CI)
```

`import "pacotes/tempo/mod.rhp"` usa o codigo vendorizado. Sem rede,
sem registro, sem resolucao de versoes (declarado como v0). Lockfile com
hash por arquivo detecta adulteracao; `--check` falha com exit 1.

## `rhp build <arquivo.rhp> [saida]` (bundle para deploy)

Resolve `import`s, valida e escreve um `.rhp` unico autocontido (padrao:
`<base>.bundle.rhp` ao lado da entrada). O bundle executa identico ao
original e nao precisa dos modulos na maquina de destino.

## `rhp check [--json] <arquivo.rhp>`

Lex + imports + compilacao, sem executar. Exit 0 e `<arquivo>: ok` se
valido; exit 1 com diagnostico estruturado (`Error[CODIGO]` + trecho +
`help:`) caso contrario. `--json` imprime relatorio maquina-legivel em
stdout (para editores/LSP futuro):

```json
{"ok":false,"file":"app.rhp","diagnostics":[{"code":"E1004","line":1,"col":9,"len":1,"message":"…","help":"…"}]}
```

Exemplos:

```bash
rhp check app.rhp
rhp check --json app.rhp | jq .diagnostics
```

## `rhp --help`, `rhp --version`

Ajuda resumida e versao do binario (`rhp 0.1.0`; versao `unreleased`).
