# RFC 0034 (proposta) — MySQL sobre TLS (opcional, opt-in)

- ID: RFC 0034
- Status: proposta
- Data: 2026-11-20
- Versao: unreleased (0.8.0 planejada)
- Depende de: RFC 0020 (conectores), RFC 0033 (caching_sha2)

## 1. Problema

O conector MySQL fala apenas TCP em claro. Em rede nao confiavel, senha
(e dados) trafegam sem confidencialidade — exceto via `caching_sha2`
com RSA (só a auth). Quem precisa de sigilo total precisa de TLS.

## 2. Objetivo e nao-objetivos

- Objetivo: opt-in de TLS na conexao (`mysql:... tls=1` ou similar),
  com verificacao de certificado configuravel (sistema por padrao;
  fingerprint pinado como alternativa; `inseguro` explicito e documentado
  como ultimo recurso).
- Nao-objetivos: TLS obrigatorio (quebraria compat), mTLS com cliente
  (segunda fase), STARTTLS em outros conectores, validacao frouxa como
  padrao.

## 3. Proposta

```rhp
let h = db_open("mysql:host=db interno port=3306 user=rhp password=... dbname=rhp tls=1");
```

- Sequencia: apos greeting, `SSLRequest` (caps + `CLIENT_SSL`) e handshake
  TLS via `rustls` (ja dependencia) sobre o `TcpConn`; segue o handshake
  normal dentro do tunel. `rustls` com `webpki-roots`; alternativa
  `tls_pino=sha256:...` para self-signed (comum em MySQL gerenciado).
- Erros de TLS claros (ex.: certificado expirado, hostname divergente).
- Sem `inseguro` silencioso: `tls=inseguro` exige opt-in textual e avisa.

## 4. Alternativas

- Tunel externo (stunnel/proxy): funciona hoje; documentar como alternativa
  (zero codigo, operacao extra).
- `rustls` com `NoVerifier`: rejeitado como padrao (falso senso de
  seguranca); aceitavel apenas atras de `tls=inseguro` explicito.

## 5. Compatibilidade

Opt-in; default segue TCP claro. Sem mudanca de API existente.

## 6. Seguranca

Verificacao por padrao; sem fallback silencioso para claro; senha nunca
em claro fora de TLS (mantido); documentar implicacoes de `inseguro`.

## 7. Performance

Handshake TLS uma vez por conexao; sem impacto em query/exec.

## 8. Plano de implementacao

1. `SSLRequest` + `rustls::ClientConnection` sobre o socket (reuso de
   `TcpConn` via `complete_io`-like; feature-gate se necessario).
2. `ServerName` IP/DNS; verificador padrao + pino opcional.
3. Opts `tls={0,1,inseguro}`, `tls_pino=...`; erros claros.
4. Mock TLS? (difícil sem certs; validar live contra MySQL 8 com TLS +
   proxy reverso com cert proprio).
5. Docs (REFERENCIA + DOCKER/compose exemplo).

## 9. Plano de testes

Live 8.0: `tls=1` com cert valido; pino self-signed; `inseguro` avisa;
falha de hostname = erro; sem TLS segue igual (mock/regressao intactos).

## 10. Migracao e rollout

Nenhuma (opt-in). Documentar topologia (proxy vs direto).

## 11. Criterios de aceitacao

Live com/sem TLS; pino funcional; erros claros; suite verde; docs.
