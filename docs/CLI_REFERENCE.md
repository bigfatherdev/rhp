# CLI Reference — `rhp`

Uso geral: `rhp <comando> [opcoes] <arquivo.rhp>`

## `rhp run [--max-steps N] <arquivo.rhp>`

Executa o programa (resolve `import`s antes de compilar).

| Codigo de saida | Significado |
|---|---|
| 0 | Sucesso |
| 1 | Erro de arquivo, lexico, sintaxe, compilacao ou execucao (`arquivo:linha:coluna: mensagem` no stderr) |
| 2 | Uso invalido (argumentos errados, `--max-steps` nao numerico) |

`--max-steps N` aborta apos N instrucoes (`limite de instrucoes excedido`).
Exemplos:

```bash
rhp run hello.rhp
rhp run --max-steps 1000 loop.rhp
```

## `rhp fmt [--check] <arquivo.rhp>`

Reescreve no formato canonico (2 espacos, `;` sempre, parenteses minimos,
`\n` final unico; idempotente). `--check` so verifica (exit 1 se divergir).

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
