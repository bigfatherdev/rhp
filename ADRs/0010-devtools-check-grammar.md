# ADR 0010 — Ferramentas de desenvolvedor: `rhp check`, gramática e debugger (CP6)

- Data: 2026-11-14
- Estado: aceita (missão Next Generation, CP6; sem breaking change)
- Contexto: editores e CI precisam de verificação sem execução e de
  gramática mantida junto da linguagem (NG-201 parcial, NG-301).

## Decisão

1. **`rhp check [--json] <arquivo>`**: lex + `import`s + compilação, sem
   executar. Humano: `<arquivo>: ok` ou diagnóstico renderizado (mesmo
   formato do `run`). `--json`: `{"ok":bool,"file":…,"diagnostics":[{code,
   line,col,len,message,help|null}]}` em stdout, exit 1 em falha.
   Contrato estável para LSP/CI futuros.
2. **Erros via `LoadError` estruturado** (`Rich` com `Box` + `Diagnostic`
   pronto; `Plain` para I/O/módulos), mapeado para humano (`render`) ou
   JSON no `check`. `run` preserva o formato humano anterior.
3. **Gramática VSCode versionada** (`editors/vscode/`): TextMate escrita à
   mão acompanhando a sintaxe normativa; teste de consistência
   (todo keyword/builtin citado) no lugar de engine — sem dependências.
4. **Debugger: só desenho** (`docs/DEBUGGER_DESIGN.md`): mapa op→span
   opt-in, checagem por `ip` em modo debug, `Vm::step()` pública futura,
   DAP como alvo; sem implementação improvisada (proibido pela missão).
5. **`doctor`/`test`/`build`/`inspect`: não implementados** — a missão
   proíbe comandos sem funcionalidade real; ficam especificados como
   próximos passos.

## Alternativas consideradas

- JSON via serde: rejeitado — crate externa proibida sem autorização;
  serialização manual com `json_escape` total cobre o alfabeto do formato.
- LSP server mínimo agora: rejeitado — sem spans completos na AST (NG-201
  parcial) nem protocolo; `check --json` entrega o substrato.

## Consequências

- `docs/CLI_REFERENCE.md` (+`check`), `ARCHITECTURE.md` (crate + comandos).
- Teste de gramática quebra se keyword/builtin novo não for adicionado à
  gramática (acoplamento intencional e documentado).
