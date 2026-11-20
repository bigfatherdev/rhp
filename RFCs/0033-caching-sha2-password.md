# RFC 0033 (proposta) — MySQL `caching_sha2_password` (+ AuthSwitch)

- ID: RFC 0033
- Status: aceita e implementada
- Data: 2026-11-20
- Versao: unreleased (implementada na 0.7.0)
- Depende de: nenhuma (usa `sha2` existente + nova dep `rsa` pura-Rust)

## 1. Problema

O cliente MySQL do RHP so fala `mysql_native_password` (RHP-AUD-009):
contra MySQL 8 com default `caching_sha2_password`, `db_open` falha com
"resposta inesperada" — mesmo com usuario/senha corretos. Como 8.0 e um
padrao de mercado, o conector e inutilizavel no setup mais comum.

## 2. Objetivo e nao-objetivos

- Objetivo: autenticar em servidores `caching_sha2_password` (default 8.0):
  scramble stage-1, `fast_auth_success`, e caminho completo via
  recuperacao de chave publica + RSA-OAEP (conexao insegura).
- Inclui: `AuthSwitchRequest` (fallback p/ `mysql_native_password` quando
  o servidor pede; erro claro caso contrario).
- Nao-objetivos: SSL/TLS no MySQL (terminar no proxy/rede confiavel;
  documentado), `sha256_password` legado, COM_MULTI, prepared nativo
  (segue interpolacao client-side documentada).

## 3. Proposta

```rhp
let h = db_open("mysql:host=127.0.0.1 port=3306 user=rhp password=rhp dbname=rhp");
```

Fluxo (observado contra MySQL 8.0 real, validado pacote a pacote):
1. Saudacao anuncia `caching_sha2_password` (ou `AuthSwitchRequest`).
2. Cliente responde com scramble stage-1:
   `XOR(senha, SHA256(SHA256(SHA256(senha)), seed))`.
3a. Servidor: `OK` ou more-data `\x00` (`fast_auth_success`) → autenticado
    (segue `OK` do handshake quando houver).
3b. Servidor: more-data `\x01` (`fast_auth_fail`) → recuperacao de chave:
    cliente pede a chave publica RSA do servidor, cifra
    `(senha XOR seed)` em OAEP e envia → espera `OK`.
4. Qualquer resposta fora desse protocolo = erro claro e acionavel
   (ex.: "use usuario mysql_native_password ou ..."), nunca hang e nunca
   "resposta inesperada" generica sem contexto.

## 4. Alternativas

- Exigir usuario native / flag de servidor: e o status quo documentado
  (workaround, nao correcao) — mantido como fallback, nao como solucao.
- Senha em claro: o servidor nunca aceita em conexao insegura — inviavel
  e inseguro; nao implementar.
- `openssl` (C): rejeitado; `rsa` (RustCrypto, puro Rust, mesma familia de
  `sha2`/`hmac` ja usados) e auditavel e sem toolchain C.

## 5. Compatibilidade

Adicao de caminho: `mysql_native_password` intacto (mock + live seguem
verdes). Novo dep `rsa` (puro Rust). Sem mudanca de API RHP.

## 6. Seguranca

Senha nunca em claro no fio inseguro (RSA-OAEP ou erro); seed por conexao;
sem fallback silencioso para auth fraca; erros nao vazam credenciais
(`conninfo` ja nunca aparece em erro). Chave publica do servidor usada
apenas para cifrar (sem validacao de identidade — mesmo modelo dos
clientes oficiais em rede nao-TLS; documentado).

## 7. Performance

Handshake algumas viagens a mais (ms, uma vez por conexao). Sem impacto
em query/exec.

## 8. Plano de implementacao

1. `sha256` via `sha2` (existente) + dep `rsa` (verificar rede crates.io;
   sem ela, abortar item com fallback AuthSwitch+fast_success+erro claro).
2. Parse de `AuthSwitchRequest` (0x01 + plugin + seed).
3. Stage-1 scramble + leitura de `OK`/more-data `\x00`/`\x01`.
4. Recuperacao de chave + OAEP + envio; leitura de `OK`.
5. Mock estendido (caching_sha2 feliz + switch + fail) + live MySQL 8.0
   default (sem flags!) + regressao MariaDB/native intactos.
6. Docs (REFERENCIA linha mysql; erro acionavel).

## 9. Plano de testes

Mock: greeting caching_sha2, switch p/ native, fast_success, fast_fail+
chave+OK, plugin desconhecido. Live: 8.0 default (sem flags) CRUD + erros;
8.0 native + MariaDB 11 sem regressao; senha errada = erro limpo (sem hang).

## 10. Migracao e rollout

Nenhuma (conexoes que falhavam passam a funcionar). Documentar que
`caching_sha2` agora e suportado.

## 11. Criterios de aceitacao

Live 8.0 default: CRUD + tipos + erros limpos; mock cobre os 5 ramos;
suite verde; sem `unsafe`; docs atualizadas.
