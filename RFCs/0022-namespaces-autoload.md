# RFC 0022 — Namespaces e autoload

- ID: RFC 0022
- Status: aceita e implementada
- Data: 2026-11-19
- Versao: unreleased (0.4.0 planejada)

## 1. Problema

Projetos com dezenas de arquivos colidem em nomes (`Controller` em varios
modulos). Imports por caminho relativo nao expressam estrutura logica.

## 2. Objetivo e nao-objetivos

- Objetivo: `namespace`, nomes qualificados `A\B\C`, `import A\B [as C]`,
  autoload PSR-4 simplificado (`A\B` -> `A/B.rhp`).
- Nao-objetivos: autoload global inseguro, `use function`, alias de funcoes
  individuais por namespace, `rhp.toml` `[autoload]` (futuro).

## 3. Proposta

```rhp
namespace App\Controllers;
class App\Controllers\UsuarioController {
  fn novo() { this["x"] = 1; }
  fn index() { return "Usuarios"; }
}
```

```rhp
import App\Controllers\UsuarioController;
let c = new App\Controllers\UsuarioController();
import App\Controllers\UsuarioController as Ctrl;
```

- Lexer: token `\` (`Backslash`); parser combina `Ident (\ Ident)*`.
- `namespace X;` define prefixo das definicoes seguintes (multiplos = ultimo
  vence; necessario para modulos ligados). Definicoes explicitas qualificadas
  sao recomendadas (sem ambiguidade).
- Nomes com `\` sao globais: o linker nao renomeia (isolamento so para nomes
  simples). Autoload tenta relativo ao importador, depois a raiz.
- `as` cria `let Alias = Qualificado;` local.

## 4. Alternativas consideradas

- Prefixo automatico obrigatorio + reescrita de referencias: rejeitado
  (complexidade de resolucao com forward refs; v1 exige qualificacao
  explicita ou definicao antes do uso no mesmo namespace).
- Mapa em `rhp.toml`: adiado (convenção de diretorios cobre v1).

## 5. Compatibilidade

Compativel: `\` era erro lexico fora de strings; nenhum programa valido
quebra. Imports antigos seguem funcionando.

## 6. Seguranca

Autoload confinado a arvore do programa (mesma jaula dos imports
relativos); sem path traversal (`..` canonicalizado e rejeitado fora da
raiz). Sem carregamento remoto.

## 7. Performance

Resolucao em compilacao (O(1) por nome via tabelas); autoload so no loader
(uma vez por arquivo, com dedup).

## 8. Plano de implementacao

Lexer (`namespace`, `\`) -> parser -> compilador (`qualify`/`resolve_name`)
-> linker (globais, autoload, alias) -> fmt/lsp/gramatica -> testes.

## 9. Plano de testes

Nomes evitam colisao; alias funciona; resolucao por diretorios; imports
antigos intactos; formatter/LSP reconhecem.

## 10. Migracao e rollout

Opcional: projetos existentes podem adotar namespaces incrementalmente.

## 11. Criterios de aceitacao

`import A\B` carrega `A/B.rhp`; alias vincula; sem regressoes.
