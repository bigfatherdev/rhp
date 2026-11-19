# RFC 0028 (proposta) — `[autoload]` em `rhp.toml` + `use function`

- ID: RFC 0028
- Status: proposta (nao implementada)
- Data: 2026-11-19
- Versao: —
- Depende de: RFC 0022 (namespaces/autoload por convenção)

## 1. Problema

O autoload da Fase 2 e so por convenção (`A\B` -> `A/B.rhp` relativo ao
importador/raiz). Projetos maiores precisam: (a) mapear prefixos para
diretorios arbitrarios (ex.: `App\` -> `src/`); (b) importar funcoes por
namespace (`use function App\Util\slug`) sem carregar a classe inteira.

## 2. Objetivo e nao-objetivos

- Objetivo: secao `[autoload]` em `rhp.toml` (PSR-4: prefixo -> dir) +
  `use` para nomes qualificados (classe, funcao, constante) com alias.
- Nao-objetivos: autoload remoto/rede, `classmap`/`files`, composer
  completo, alteracao da jaula de imports.

## 3. Proposta

```toml
nome = "meuapp"
versao = "0.1.0"

[autoload]
"App\\" = "src/"
"Rhp\\Web\\" = "pacotes/rhp_web/"
```

```rhp
use App\Controllers\UsuarioController;
use App\Controllers\UsuarioController as Ctrl;
use function App\Util\slug;

let c = new Ctrl();
echo slug("Ola Mundo");
```

- Regras: prefixo mais longo vence; dir relativo ao `rhp.toml`; fora da
  arvore = erro (mesma jaula); `use` so no topo, antes de qualquer codigo
  (como `import`); `use function`/`use const` separados de `use` (classe).
- Precedencia de resolucao: `use` > namespace atual > global.
- `rhp check` valida o mapeamento (prefixo sem dir existente = aviso/erro).

## 4. Alternativas consideradas

- So convenção (status quo): rejeitada para monorepos com `src/`, `lib/`.
- `rhp pkg` com reescrita de caminhos: rejeitado (acopla distribuicao a
  resolucao; autoload deve funcionar sem pacote).

## 5. Compatibilidade

Aditiva: `rhp.toml` sem `[autoload]` comporta-se como hoje; `use` era
palavra reservada de traits (`use T;` em classe) — `use` no TOPO do arquivo
e novo (contexto distinto; dentro de classe mantem semantica de trait).

## 6. Seguranca

Mesma jaula (arvore do programa); `..` canonicalizado e rejeitado fora da
raiz; sem rede; mapeamento so de leitura do manifesto local.

## 7. Performance

Mapa prefixo->dir consultado uma vez por import (cache no loader); sem
impacto em runtime.

## 8. Plano de implementacao

Parser (`use`, `use function/const`, validacao de posicao) -> `pkg.rs`
(parse de `[autoload]`) -> loader (resolucao por prefixo + alias) ->
compilador (`resolve_name` com alias) -> fmt/lsp/gramatica -> testes.

## 9. Plano de testes

Prefixo mais longo vence; dir inexistente = erro; fora da arvore = erro;
alias; colisao com global = erro; `use` em classe segue trait.

## 10. Migracao e rollout

Opcional e incremental; projetos sem `[autoload]` nao mudam.

## 11. Criterios de aceitacao

Mapeamento resolve; `use function` vincula; erros claros; suite verde.
