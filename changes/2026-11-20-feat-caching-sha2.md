# [Added] — MySQL `caching_sha2_password` (RFC 0033)

- ID: 2026-11-20-feat-caching-sha2
- Data: 2026-11-20
- Versão: 0.7.0
- Tipo: Added
- Compatibilidade: Compatível (caminho novo; native intacto)

## Resumo

Autenticação em MySQL 8 default: stage-1 SHA256, `fast_auth_success` e
full auth via chave pública + RSA-OAEP(SHA1) (puro Rust). Erros claros e
acionáveis em vez de "resposta inesperada".

## Motivação

RHP-AUD-009: conector inutilizável no setup mais comum (MySQL 8 default).

## O que mudou

- `connect()`: lê plugin do greeting; stage-1 por plugin
  (`native_password` inalterado; `caching_sha2_token` novo).
- `finish_auth`/`auth_switched`/`caching_more_data`/`caching_full_auth`:
  AuthSwitch, fast (n==3 + OK), full (0x02 + chave + RSA + OK).
- `rsa_oaep_encrypt` (`rsa` + `sha1` + `getrandom` + `rand_core`, puros Rust;
  `SysRng` local sobre `getrandom::fill`).
- Sequência compartilhada: `read_packet` atualiza `seq` (N+1); `command()`
  segue zerando por comando (RHP-AUD-008 preservado).
- Mock estendido: greeting caching, stage-1 validado, fast (n==3) e full
  (n==4 + chave + RSA decifrado e verificado no mock).
- Docs: REFERENCIA `db_open`; RFC 0033 com o protocolo validado pacote a
  pacote (incl. beco sem saída documentado: `0x00` solitário).

## Detalhes técnicos

Sem TLS (documentado: terminar no proxy/rede). Sem `unsafe`. Deps novas
puras Rust. Senha nunca em claro no fio inseguro; `conninfo` segue fora
das mensagens.

## Compatibilidade e migração

Conexões que falhavam passam a funcionar. Migração: não necessária.

## Segurança e performance

RSA-OAEP(SHA1) padrão do protocolo; handshake +2-4 viagens (uma vez por
conexão). Falha de entropia = erro fatal (sem degradação silenciosa).

## Testes e validação

- Mock: fast + full (RSA real, decifra e confere XOR) + nativo intacto.
- Live MySQL 8.0 default: CRUD + tipos + erros; senha errada = 1045 limpo;
  usuário virgem pós-restart (caminho full) OK.
- Suite verde; fmt/clippy limpos (deps auditadas: `rsa`, `sha1`, etc.).

## Limitações e riscos conhecidos

Sem TLS no MySQL (documentado); sem `sha256_password` legado; sem
verificação de identidade da chave (mesmo modelo dos clientes oficiais
em rede não-TLS).

## Referências

- RFC 0033; `audit/0.6.1/FINDINGS.md` (RHP-AUD-009); PyMySQL (oráculo).
