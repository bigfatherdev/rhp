# RFC 0026 — Extensoes nativas estaticas

- ID: RFC 0026
- Status: aceita e implementada
- Data: 2026-11-19
- Versao: unreleased (0.4.0 planejada)

## 1. Problema

Baixo nivel (cripto extra, drivers, bindings) em Rust nao chega ao RHP sem
alterar o core a cada necessidade.

## 2. Objetivo e nao-objetivos

- Objetivo: registro estatico de extensoes (`import native "ext"`),
  funcoes `fn(Vec<Value>) -> Result<Value, String>`, ABI versionada,
  erros propagados, exemplo `demo`.
- Nao-objetivos: `.so` dinamico/`dlopen`/`unsafe`/FFI (rejeitados em v1),
  extensoes com estado global mutavel, async nativo.

## 3. Proposta

```rhp
import native "demo";
echo demo_eco("RHP");
echo demo_soma(20, 22);
```

- `rhp-vm/src/extensions.rs`: `RHP_EXTENSION_ABI = 1`, `registry()`,
  `call(ext, func, args)` com checagem de aridade e fronteira de tipos
  (so `Null/Bool/Int/Float/Str/Array/Map`; outros = erro claro).
- `FuncDef.native_id = Some("ext::func")` (execucao sincrona sem frame);
  op `LoadNative(nome)` declara as funcoes como globais; dupla carga e
  colisao com global existente = erro.
- Loader valida a extensao na coleta (falha cedo com lista disponivel).

## 4. Alternativas consideradas

- `dlopen` de `.so`: rejeitado (ABI instavel, `unsafe`, path traversal,
  crashes fora do controle do runtime).
- Builtins novos no core por demanda: rejeitado (acoplamento; extensoes
  isolam evolucao).

## 5. Compatibilidade

Aditivo (`import native` era erro). Sem mudanca em builtins existentes.

## 6. Seguranca

Sem codigo dinamico: so extensoes compiladas no binario. Sem `unsafe`.
Fronteira valida tipos; sem I/O implicito; sem segredos em logs. Auditoria
por extensao antes de incluir no registro.

## 7. Performance

Chamada direta (sem thread/JSON); custo de uma chamada de funcao Rust.

## 8. Plano de implementacao

`extensions.rs` + `FuncDef.native_id` + `LoadNative` + despacho em
`call`/`call_mixed` + loader + `demo` + docs de autor.

## 9. Plano de testes

`demo_*` com tipos certos/errados, aridade, extensao desconhecida, dupla
carga, colisao.

## 10. Migracao e rollout

Para nova funcao nativa: implementar em Rust, registrar, documentar, testar.

## 11. Criterios de aceitacao

Registro carrega, args/retornos convertem, erros propagam e sao capturaveis
por `try`, ABI incompativel recusada.
