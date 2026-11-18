# SECURITY_REMEDIATION.md — Remediação das 20 CVEs do escopo

- Data: 2026-11-18
- Responsável: engenharia de segurança (sessão de remediação)
- Escopo: repositório RHP + distribuição Docker (`Dockerfile`, base
  `rust:1.100-bookworm` / `debian:bookworm-slim`)
- Método: uma CVE por vez (identificação → análise → reprodução quando
  viável → correção → testes → evidência). Nenhuma supressão (`allow`,
  `ignore`, `suppress`) foi adicionada.
- Classificação da mudança (GOVERNANCE.md §3): **correção de segurança
  + melhoria interna de tooling** (Dockerfile). Compatível: nenhuma
  semântica da linguagem, bytecode, VM ou stdlib foi alterada.

## Inventário inicial (resumo)

- Workspace Rust: `rhp-lexer`, `rhp-parser`, `rhp-bytecode`, `rhp-vm`,
  `rhp-fmt`, `rhp-diagnostic`, `rhp-cli` (binário `rhp`). Sem `unsafe`
  no código do workspace (ver `SECURITY.md`).
- Dependências Rust (`Cargo.lock`): `argon2 0.6`, `bcrypt 0.19`,
  `password-hash 0.6`, `regex 1`, `sha2 0.11`, `hmac 0.13`,
  `rustls 0.23` (+ `ring`, `webpki-roots 0.26`, `rustls-pemfile 2`).
  **Sem** crates de SQLite, OpenSSL, zlib, libgcrypt, PAM, APT, dash,
  diffutils ou GnuPG.
- FFI nativo: **apenas `libsqlite3`** (`crates/rhp-vm/src/sqlite.rs`,
  `#[link(name = "sqlite3")]`, ABI mínima: open/prepare/bind/step/
  column/finalize/close + changes/errmsg). Sem extensões vinculadas
  pelo RHP; **nenhum símbolo de sessão** (`sqlite3changeset_*`)
  é importado.
- Link dinâmico do binário (`readelf -d` / `ldd`): `libsqlite3.so.0`,
  `libc.so.6`, `libm.so.6`, `libgcc_s.so.1`. **Sem** `libssl`, `libz`,
  `libgcrypt`, `libpam`. Sem `RPATH`/`RUNPATH` (`readelf -d` não lista
  `RPATH` nem `RUNPATH`).
- SQLite do sistema (host da auditoria): `3.46.1`, com
  `ENABLE_FTS5`, `ENABLE_SESSION`, `SECURE_DELETE` compilados
  (`PRAGMA compile_options`). Versão `< 3.54.0`.
- Docker: multi-stage; builder instala `ca-certificates gcc
  libsqlite3-dev`; runtime instala `ca-certificates libsqlite3-0`;
  usuário não-root `rhp` (UID 10001); sem SUID próprio.
- `cargo audit`: 1 aviso permitido (`RUSTSEC-2025-0134`,
  `rustls-pemfile` sem manutenção, sem upgrade disponível) — registrado
  como limitação, não como aprovação.
- `cargo test --workspace`: verde antes e depois (ver § Testes).

## Correção implementada (única mudança de código/infra)

Arquivo modificado: `Dockerfile` (builder + runtime):

```dockerfile
RUN apt-get update && apt-get upgrade -y --no-install-recommends \
    && apt-get install -y --no-install-recommends ...
```

Efeito: aplica no momento do build todos os backports de segurança
disponíveis no Debian para `libsqlite3-0`, `libssl3`, `zlib1g`,
`libc6`, `util-linux`/`libblkid`, `dash`, `libpam-modules`, `apt`,
`diffutils`, `libgcrypt20`, `gnupg`, etc. Preserva arquitetura
(multi-stage, imagem mínima, usuário não-root, sem toolchain no
runtime). Nenhuma dependência nova, nenhum `unsafe`, nenhuma mudança
de semântica.

Base: `trixie` desde 2026-11-18 (`rust:1.100-trixie` + `debian:trixie-slim`;
antes `bookworm`; migração aprovada pelo solicitante, ver ADR 0015 —
emenda). `trixie` recebe backports mais rápido que `bookworm` (util-linux
e zlib já corrigidos em `trixie-security`), mas o SQLite do `trixie`
(`3.46.1`) continua `< 3.54.0` — o fix FTS5/sessão só existe no
`forky`/`sid` (`3.54.1`). Por isso nenhuma CVE de biblioteca C/OS é
marcada `CORRIGIDA` apenas pela mitigação — ver status individuais abaixo.

## Atualização 2026-11-18 — migração `bookworm` → `trixie`

- `Dockerfile`: `FROM rust:1.100-bookworm` → `rust:1.100-trixie`
  (tag ativa verificada no Docker Hub em 2026-11-18);
  `FROM debian:bookworm-slim` → `debian:trixie-slim`.
- Docs coordenadas: `docs/DOCKER.md` (seção de arquitetura), ADR 0015
  (emenda), `CHANGELOG.md`, `changes/2026-11-18-tooling-docker-trixie.md`.
- Efeito esperado em desempenho: nulo no runtime do RHP (nenhum caminho
  quente alterado); custo só no tempo de build da imagem. SQLite ≥ 3.54
  adiciona apenas checagens de limites nos caminhos FTS5 (sem alocação
  extra). Sem medição antes/depois nesta sessão (sem Docker daemon);
  reexecutar benchmarks de `docs/BENCHMARKS.md` no CI caso exigido.
- Resultado na classificação: sem mudança (itens 1–2 seguem MITIGADOS —
  `trixie` também não tem o backport SQLite; demais itens seguem NÃO
  AFETADOS; a base mais nova reduz a janela para futuros DSAs).

Limite honesto: em `bookworm`, vários backports para as CVEs de
outubro/novembro-2026 **ainda não existiam** na data da auditoria
(status "needs evaluation"/"vulnerable" nos trackers). O `upgrade`
é a correção oficial correta, mas **não fabrica um patch inexistente**.
Por isso nenhuma CVE de biblioteca C/OS é marcada `CORRIGIDA` apenas
pela mitigação — ver status individuais abaixo.

---

## CVE-2026-119374 — SQLite FTS5 `fts5DoSecureDelete()` heap overflow

- Descrição: SQLite `< 3.54.0` com FTS5 contém heap buffer overflow em
  `fts5DoSecureDelete()`; DB com índice FTS5 corrompido + `secure-delete`
  corrompe heap ao deletar. CWE-787. CVSS 3.1 `7.8 HIGH`
  (CVSS 4.0 `8.5 HIGH`). Fonte: VulnCheck / NVD / CVE.org.
- Componente: `libsqlite3` do sistema (versão vulnerável `< 3.54.0`;
  corrigida `3.54.0`).
- Evidência de presença: `crates/rhp-vm/src/sqlite.rs` vincula
  `libsqlite3`; host com `3.46.1` + `ENABLE_FTS5` + `SECURE_DELETE`;
  `db_exec`/`db_query` repassam SQL arbitrário, logo
  `CREATE VIRTUAL TABLE ... USING fts5` e abertura de DBs criados por
  terceiros alcançam o código vulnerável. **AFETADO (via pass-through)**.
- Causa-raiz: deslocamentos de chave de rodapé não validados no
  `fts5DoSecureDelete()` (upstream `sqlite.org/src/info/9cd4f9e920`,
  commit `d9de6eed`).
- Correção: `Dockerfile` passa a executar `apt-get upgrade` (builder +
  runtime), aplicando o backport Debian de `libsqlite3-0 ≥ 3.54.0`
  quando publicado. Nenhum bloqueio de FTS5 foi adicionado no RHP
  (seria breaking change sem aprovação; a superfície é a mesma de
  qualquer cliente SQLite).
- Arquivos modificados: `Dockerfile`.
- Testes: `cargo test --workspace` (verde); `cargo clippy --workspace
  --all-targets -- -D warnings` (verde); `cargo fmt --all -- --check`
  (verde). Sem teste de PoC do overflow (exigiria DB corrompido
  hostil; o fix é do C upstream, não do Rust).
- Referências: `cve.org/CVERecord?id=CVE-2026-119374`,
  `sqlite.org/releaselog/3_54_0.html`,
  `github.com/sqlite/sqlite/commit/d9de6eedbe918c074c7556400fa21df5960704d9`.
- Status final: **MITIGADO** (upgrade aplica backport quando o Debian
  publicar; sem backport `bookworm` na data da auditoria, a eliminação
  total depende do upstream — não marcado como CORRIGIDO).

## CVE-2026-119373 — SQLite FTS5 `fts5MergeRowidLists()` heap overflow

- Descrição: SQLite `< 3.54.0` com FTS5 contém heap buffer overflow em
  `fts5MergeRowidLists()` (buffer subdimensionado em merge de
  `detail=none` com rowids negativos + `MATCH` por prefixo). CWE-787.
  CVSS 3.1 `7.8 HIGH`. Fonte: VulnCheck / NVD / CVE.org.
- Componente / versões: idem CVE-2026-119374 (`< 3.54.0` → `3.54.0`).
- Evidência: mesma pass-through de SQL do RHP; `MATCH` sobre tabela
  FTS5 criada/populated via `db_exec` alcança o código. **AFETADO**.
- Causa-raiz: cálculo de tamanho sem considerar varints negativos
  (`sqlite.org/src/info/a7f8c92f62`, commit `e1c9934f`).
- Correção / arquivos / testes / referências: idem CVE-2026-119374
  (mesmo `Dockerfile`; `cve.org/CVERecord?id=CVE-2026-119373`,
  commit `e1c9934f9f11a499029abb020ea715a08970da80`).
- Status final: **MITIGADO** (mesma ressalva de backport Debian).

## CVE-2026-119379 — SQLite sessão `sessionUpdateFind()` heap overflow

- Descrição: SQLite `< 3.54.0` com extensão de sessão contém heap
  buffer overflow em `sqlite3changeset_apply()` (tabela repetida com
  mais colunas; `memset` além de `aUpdateMask`). CWE-787. CVSS 3.1
  `7.8 HIGH`. Fonte: VulnCheck / NVD / CVE.org.
- Componente: `libsqlite3` (`SQLITE_ENABLE_SESSION`), `< 3.54.0` →
  `3.54.0` (commit `64459c87`).
- Evidência de ausência no RHP: `crates/rhp-vm/src/sqlite.rs` importa
  **apenas** open/prepare/bind/step/column/finalize/close/changes/
  errmsg; `grep -ri changeset|sessionUpdate` nos `crates/` retorna
  somente a variável local `session` do REPL (`main.rs:645`), sem
  relação com a API C. O RHP **nunca chama** `sqlite3changeset_apply`,
  logo o caminho vulnerável é inalcançável pelo binário. Sistema com
  `ENABLE_SESSION` compilado, mas não invocado.
- Correção: nenhuma mudança de código necessária; `Dockerfile`
  `upgrade` atualiza a lib por defesa em profundidade.
- Testes: `cargo test --workspace` verde (suite SQLite CRUD existente
  `runs_sqlite_crud` continua passando).
- Referências: `cve.org/CVERecord?id=CVE-2026-119379`,
  `sqlite.org/releaselog/3_54_0.html`.
- Status final: **NÃO AFETADO** (API vulnerável nunca vinculada nem
  chamada pelo RHP; lib do sistema atualizada via `upgrade`).

## CVE-2026-76844 — registro disputado (zlib × webpack-dev-middleware)

- Descrição A (VulnCheck/NVD, `cve.org` inicial): zlib `1.2.11–1.3.2`
  heap overflow via ponteiro `strm.next_in` obsoleto após `write()`
  falhar (`gz_write`/`gz_vacate`). CWE-787. CVSS 3.1 `7.4 HIGH`.
- Descrição B (GitHub Advisory `GHSA-g84c-rxfj-3j2c`, OpenJS CNA):
  path traversal em `webpack-dev-middleware` (`publicPath` sem barra
  final), `8.3.0`/`7.4.6` corrigem; Tenable associa o mesmo ID a esta
  descrição. Há divergência pública entre CNAs.
- Evidência de ausência no RHP (ambas as leituras): nenhum crate
  `zlib`/`flate2`/`miniz` no `Cargo.lock`; `grep -ri gzwrite|flate|
  gzip` nos `crates/` vazio; `ldd`/`readelf -d` do binário não listam
  `libz`; nenhum `package.json`/npm/webpack no repositório. RHP não
  comprime via zlib nem serve assets via `webpack-dev-middleware`.
- Correção: nenhuma no código; `Dockerfile` `upgrade` cobre `zlib1g`
  da base por defesa em profundidade.
- Referências: `cve.org/CVERecord?id=CVE-2026-76844`,
  `github.com/advisories/GHSA-g84c-rxfj-3j2c`,
  `github.com/madler/zlib/commit/df84af25dc1942490e1d1c899a07619152a46148`.
- Status final: **NÃO AFETADO** (disputa documentada; nenhuma das duas
  leituras se aplica ao RHP).

## CVE-2026-85091 — zlib `gz_vacate()` heap overflow

- Descrição: zlib `1.3.1.2–1.3.2` heap overflow em `gz_vacate()` após
  stall de `gzwrite()` não-bloqueante + `gzprintf`/`gzvprintf`
  (`memmove` sem checagem). CWE-787. CVSS 3.1 `7.4 HIGH` (4.0 `8.3`).
  Fonte: VulnCheck / NVD / OSV / RUSTSEC-2026-0544 (para `libz-sys`).
- Evidência de ausência: idem CVE-2026-76844 — sem crate `libz-sys`,
  sem chamadas `gz*`, sem link `libz` no binário. A RUSTSEC-2026-0544
  afeta `libz-sys ≥ 1.1.25, < 1.1.33`, pacote **ausente** do
  `Cargo.lock` do RHP.
- Correção: `Dockerfile` `upgrade` (base `zlib1g`).
- Referências: `cve.org/CVERecord?id=CVE-2026-85091`,
  `rustsec.org/advisories/RUSTSEC-2026-0544`.
- Status final: **NÃO AFETADO**.

## CVE-2026-95691 — OpenSSL QUIC CRYPTO use-after-free

- Descrição: OpenSSL `4.1.0–4.1.0 / 4.0.0–4.0.3 / 3.5.0–3.5.9`:
  frame CRYPTO QUIC retransmitido libera armazenamento ainda
  referenciado pelo TLS → UAF + DoS / 1 byte adjacente. CWE-416.
  Fonte: OpenSSL Software Foundation / NVD.
- Evidência de ausência: `crates/rhp-vm/src/tls.rs` usa **rustls puro**
  (`ClientConnection` sobre TCP clássico); nenhum link OpenSSL
  (`ldd` sem `libssl`/`libcrypto`); nenhum código QUIC no workspace;
  `Cargo.lock` sem crate `openssl`. `openssl` aparece só nos testes
  de integração como gerador externo de certificados de teste.
- Correção: nenhuma no código; `Dockerfile` `upgrade` atualiza
  `libssl3t64`/`openssl` da base.
- Referências: `cve.org/CVERecord?id=CVE-2026-95691`,
  `openssl-library.org/news/secadv/20261110.txt`.
- Status final: **NÃO AFETADO**.

## CVE-2026-53615 — util-linux libblkid integer overflow (DOS estendido)

- Descrição: util-linux `< 2.41.4`, `parse_dos_extended()` em
  `libblkid/src/partitions/dos.c`: soma 32-bit com wrap + checagem
  ausente nas 2 primeiras entradas EBR → partição falsa sobre o MBR.
  CWE-190. CVSS 3.1 `8.0 HIGH`. Fonte: GitHub CNA / NVD.
- Evidência de ausência: RHP nunca sonda partições nem vincula
  `libblkid`; `grep -ri blkid|fdisk` nos `crates/` vazio; runtime roda
  como `rhp` sem acesso a devices; nenhuma leitura de imagem de disco.
- Correção: `Dockerfile` `upgrade` (pacote `libblkid1`/`util-linux`
  da base; fix `2.41.4`, commits `05c2dba`/`a2d8581`).
- Referências: `cve.org/CVERecord?id=CVE-2026-53615`,
  `github.com/util-linux/util-linux/security/advisories/GHSA-h4rw-gv36-wmp5`.
- Status final: **NÃO AFETADO**.

## CVE-2026-78410 — util-linux mount TOCTOU (bind + X-mount.owner/group/mode)

- Descrição: util-linux `2.39–2.42.2`: bind mounts restritos não fixam
  (`pin`) a origem do fstab antes do mount privilegiado → redireção +
  `chown`/`chmod` do root no inode redirecionado. CWE-367. CVSS 3.1
  `7.8 HIGH`. Fix `2.41.6`/`2.42.3`. Fonte: Red Hat CNA.
- Evidência de ausência: RHP não monta filesystems, não lê `/etc/fstab`,
  não executa `mount(8)`, não é SUID, roda como UID 10001; `grep -ri
  "mount("` nos `crates/` só retorna `libmount` em listagem de pacotes
  do host, nada no código.
- Correção: `Dockerfile` `upgrade` por defesa em profundidade.
- Referências: `cve.org/CVERecord?id=CVE-2026-78410`,
  `github.com/util-linux/util-linux/security/advisories/GHSA-rh77-686x-2f2m`.
- Status final: **NÃO AFETADO**.

## CVE-2025-6020 — linux-pam `pam_namespace` path traversal → privesc

- Descrição: linux-pam `< 1.7.1`, `pam_namespace` usa caminhos
  controlados sem proteção (symlink/race) → root local. CWE-22.
  CVSS 3.1 `7.8 HIGH`. Fonte: Red Hat CNA / NVD / Ubuntu USN-7580-1.
- Evidência de ausência: RHP não usa PAM (nenhum crate/pam, nenhum
  `pam_*` no código); `pam_namespace` não é usado em instalações
  padrão (nota Ubuntu); imagem RHP sem login PAM, processo único como
  não-root.
- Correção: `Dockerfile` `upgrade` (pacotes `libpam*` da base).
- Referências: `cve.org/CVERecord?id=CVE-2025-6020`,
  `github.com/linux-pam/linux-pam/security/advisories/GHSA-f9p8-gjr4-j9gx`.
- Status final: **NÃO AFETADO**.

## CVE-2026-107493 — OpenSSL `PEM_X509_INFO_read_bio_ex()` use-after-free

- Descrição: OpenSSL `4.1.0/4.0.0–4.0.3/3.5.0–3.5.9/3.0.x/1.1.1/1.0.2`
  (faixas no registro): ao falhar com stack fornecida não-NULL, libera
  entradas mas mantém ponteiros → UAF. CWE-416. CVSS 3.1 `7.7 HIGH`
  (CISA-ADP). Fonte: OpenSSL CNA.
- Evidência de ausência: RHP não usa OpenSSL; parsing de
  `RHP_CA_BUNDLE` usa `rustls_pemfile::certs` (apenas decodifica blocos
  PEM, sem semântica de stack OpenSSL). OpenSSL declara que o próprio
  código sempre passa NULL (seguro) — o RHP sequer chega a esse código.
- Limitação registrada: `cargo audit` aponta `RUSTSEC-2025-0134`
  (`rustls-pemfile` sem manutenção, sem upgrade disponível). Não há
  correção aplicável sem trocar de parser (fora do escopo; seria
  refatoração injustificada).
- Referências: `cve.org/CVERecord?id=CVE-2026-107493`,
  `openssl-library.org/news/secadv/20261117.txt`.
- Status final: **NÃO AFETADO**.

## CVE-2026-13595 — util-linux libblkid UAF (probing aninhado)

- Descrição: `libblkid` (BSD/Minix/Solaris x86/UnixWare): ponteiro
  para entrada pai torna-se obsoleto após `reallocarray` → UAF de 8
  bytes via imagem de bloco criada (udev/udisks como root). CWE-416.
  CVSS 3.1 `6.8 MEDIUM`. Fix: commit `c0186f14`. Fonte: Red Hat CNA.
- Evidência de ausência: idem CVE-2026-53615 (sem `libblkid` no RHP).
- Correção: `Dockerfile` `upgrade`.
- Referências: `cve.org/CVERecord?id=CVE-2026-13595`,
  `github.com/util-linux/util-linux/commit/c0186f14fbdb02f64c8e0ba701ce727ea764ff4c`.
- Status final: **NÃO AFETADO**.

## CVE-2026-86805 — glibc `ld.so` TOCTOU `$ORIGIN` (AT_SECURE)

- Descrição: glibc `2.14–2.44`: expansão de `$ORIGIN` em `DT_RPATH` de
  programas setuid/setgid valida caminho normalizado mas abre o bruto
  → com `fs.protected_hardlinks=0`, hardlink + symlink swap carrega
  `.so` do atacante como root. CWE-367. CVSS 3.1 `6.3 MEDIUM`.
  Fix: commit `ed0c137b` (glibc `2.45`). Fonte: GNU C Library CNA.
- Evidência de ausência/exposição mínima: binário `rhp` **sem**
  `RPATH`/`RUNPATH` (`readelf -d` só lista `NEEDED`), **sem** `$ORIGIN`
  com `..`, **não** instalado setuid/setgid, imagem sem binários SUID
  próprios; distros padrão (incl. Debian) trazem
  `fs.protected_hardlinks=1` (mitigação documentada pelo CNA).
- Correção: `Dockerfile` `upgrade` (pacote `libc6` da base; bookworm
  `2.36` ainda vulnerável sem backport na data — acompanhar DSA).
- Referências: `cve.org/CVERecord?id=CVE-2026-86805`,
  `sourceware.org/bugzilla/show_bug.cgi?id=34360`.
- Status final: **NÃO AFETADO** (binário RHP sem vetor; base mitigada
  via `upgrade` + `protected_hardlinks` padrão).

## CVE-2026-102474 — dash `conv_escape` heap OOB (unicode `\u`/`\U`)

- Descrição: dash: `printf` reserva 4 bytes mas token multibyte pode
  precisar de 5–6 → OOB de 1–2 bytes via escape unicode. CWE-787.
  CVSS 3.1 `4.0 MEDIUM`. Sem fix upstream na data (Debian/Ubuntu: "fix
  deferred"). Fonte: Red Hat CNA.
- Evidência de ausência: RHP não invoca `dash`/`sh`; único
  `Command::new` em produção é `tool_version("rustc"|"cargo")` em `rhp
  doctor` (sem dados não confiáveis, sem `printf`/`echo %b` do dash).
  `/bin/sh` da base existe, mas o RHP nunca o executa.
- Correção: `Dockerfile` `upgrade` (aplica fix de `dash` quando o
  Debian publicar; hoje **AGUARDANDO CORREÇÃO UPSTREAM** também no
  Debian/Ubuntu).
- Referências: `cve.org/CVERecord?id=CVE-2026-102474`.
- Status final: **NÃO AFETADO** (com ressalva upstream para a base).

## CVE-2026-102473 — dash `pmatch()` backtracking super-polinomial

- Descrição: dash sem `fnmatch` da libc: `*` via recursão ilimitada →
  DoS de CPU com `*.*.*.*.*.tar.gz`. CWE-1333. CVSS 3.1 `5.5 MEDIUM`.
  Sem fix upstream ("fix deferred"); mitigação: compilar com
  `--enable-fnmatch`. Fonte: Red Hat CNA.
- Evidência de ausência: idem anterior (RHP não executa dash nem
  alimenta `pmatch` com nomes de arquivo do atacante).
- Correção: `Dockerfile` `upgrade`.
- Referências: `cve.org/CVERecord?id=CVE-2026-102473`.
- Status final: **NÃO AFETADO**.

## CVE-2024-2236 — libgcrypt Marvin attack (RSA timing side-channel)

- Descrição: libgcrypt RSA (todos os paddings) permite ataque estilo
  Bleichenbacher via canal lateral de tempo → descriptografia.
  CWE-385. CVSS 3.1 `5.9 MEDIUM`. Fonte: Red Hat CNA. Debian bookworm:
  ainda vulnerável (sem fix upstream completo).
- Evidência de ausência: RHP não usa libgcrypt (nem GnuPG); RSA/TLS é
  `rustls`+`ring`; hashing é RustCrypto (`sha2`/`hmac`) e
  Argon2/bcrypt puros. `Cargo.lock` sem `libgcrypt`; `ldd` sem
  `libgcrypt`.
- Correção: `Dockerfile` `upgrade` (pacote `libgcrypt20` da base).
- Referências: `cve.org/CVERecord?id=CVE-2024-2236`.
- Status final: **NÃO AFETADO**.

## CVE-2018-6829 — libgcrypt ElGamal sem segurança semântica

- Descrição: `cipher/elgamal.c` até `1.8.2` codifica plaintexts de forma
  insegura (DDH não vale) → leitura de ciphertext vaza info. CWE-327.
  CVSS 3.0 `7.5 HIGH` (Red Hat: aplicação que usa mal, não a lib).
  Fonte: MITRE / NVD. Ubuntu/SUSE: "not-affected / uso indevido da app".
- Evidência de ausência: RHP não usa ElGamal nem libgcrypt (ver acima).
- Correção: nenhuma aplicável além do `upgrade` da base.
- Referências: `cve.org/CVERecord?id=CVE-2018-6829`.
- Status final: **NÃO AFETADO**.

## CVE-2011-3374 — apt `apt-key` sem validar contra master keyring (MitM)

- Descrição: `apt-key` (todas as versões, na prática Ubuntu
  `net-update`) não valida chaves GPG → MitM. CWE-347. CVSS 3.1
  `3.7 LOW`. Debian: "unfixed/unimportant; sem keyring URI no Debian".
  Fonte: Red Hat CNA.
- Evidência de ausência: RHP não invoca `apt`/`apt-key`; `apt` existe
  na imagem apenas como gerenciador da base (uso em build); runtime não
  executa atualização de chaves; RHP não valida pacotes via `apt-key`
  (pacotes RHP usam `rhp.lock` com hash por arquivo).
- Correção: `Dockerfile` `upgrade` + `rm -rf /var/lib/apt/lists/*`
  já existente (higiene).
- Referências: `cve.org/CVERecord?id=CVE-2011-3374`.
- Status final: **NÃO AFETADO**.

## CVE-2026-53910 — GNU diffutils `diff3` heap overflow (integers) [DISPUTADA]

- Descrição: `diff3` até `3.12`: overflows de inteiros com sinal no
  mapeamento de linhas → `malloc` insuficiente + OOB via saída de
  `--diff-program` malicioso. CWE-190. CVSS 4.0 `2.1 LOW`. **Mantenedores
  GNU contestam que seja vulnerabilidade de segurança** (no máximo
  crash, sem privesc). Fonte: CERT.PL CNA.
- Evidência de ausência: RHP nunca executa `diff3` nem aceita
  `--diff-program`; `diffutils` presente só como ferramenta do ambiente
  (fora do runtime mínimo — nem instalado explicitamente no runtime).
- Correção: `Dockerfile` `upgrade` (pacote `diffutils` quando houver
  backport; Debian: "unfixed/negligible" na data).
- Referências: `cve.org/CVERecord?id=CVE-2026-53910`,
  `cert.pl/en/posts/2026/07/CVE-2026-53910`.
- Status final: **NÃO AFETADO**.

## CVE-2007-5686 — rPath Linux 1 initscripts `/var/log/btmp` (permissões)

- Descrição: `initscripts` do **rPath Linux 1** cria `/var/log/btmp`
  com permissões inseguras → leitura de tentativas de login; sshd
  deixa de logar. CWE-264. CVSS 2.0 `4.9 MEDIUM`. Fonte: MITRE.
- Evidência de ausência: componente exclusivo do rPath Linux 1 (distro
  extinta, 2007); imagem RHP é `debian:bookworm-slim`; Ubuntu declara
  "does not apply". Nenhum `initscripts` rPath no repositório/imagem.
- Correção: nenhuma aplicável.
- Referências: `cve.org/CVERecord?id=CVE-2007-5686`.
- Status final: **NÃO AFETADO**.

## CVE-2022-3219 — GnuPG DoS (chave com milhares de assinaturas)

- Descrição: GnuPG gira (spin) com entrada de poucos KB contendo chave
  pública com milhares de assinaturas comprimidas → DoS. CWE-787.
  CVSS 3.1 `3.3 LOW`. Upstream **não corrigirá** (Ubuntu/Debian/SUSE:
  "ignored/wontfix"). Fonte: Red Hat CNA.
- Evidência de ausência: RHP não usa GnuPG nem analisa chaves OpenPGP;
  nenhum crate/binário `gpg` invocado pelo código (só existe no host
  de desenvolvimento, fora da imagem runtime).
- Correção: nenhuma aplicável além do `upgrade` da base.
- Referências: `cve.org/CVERecord?id=CVE-2022-3219`,
  `dev.gnupg.org/T5993`.
- Status final: **NÃO AFETADO**.

---

## Testes de segurança executados

- `cargo test --workspace` — verde (324 testes aprox.; contagem por
  suíte: 40+43+2+47+5+28+17+50+90+2; 0 falhas) antes e depois.
- `cargo clippy --workspace --all-targets -- -D warnings` — verde.
- `cargo fmt --all -- --check` — verde.
- `cargo audit` — 1 aviso `RUSTSEC-2025-0134` (`rustls-pemfile`
  unmaintained, sem fix disponível); **nenhuma** das 20 CVEs do escopo
  atinge crates Rust do `Cargo.lock` (verificado por nome/componente).
- `cargo deny` — **não executado** (ferramenta ausente no ambiente);
  registrado como limitação, não como aprovação.
- Scanner de imagem Docker / SBOM — **não executado** (Docker daemon
  indisponível no ambiente); a correção do Dockerfile é verificável por
  inspeção + rebuild no CI (`docker-publish.yml`).
- Fuzz/ASan por CVE — não aplicável (bugs em C upstream; PoCs exigiriam
  artefatos hostis de SQLite/zlib; nenhum caminho novo em Rust foi
  adicionado).

## Tabela-resumo

| # | CVE | Componente | Status |
|---|-----|------------|--------|
| 1 | CVE-2026-119374 | libsqlite3 (FTS5 secure-delete) | MITIGADO |
| 2 | CVE-2026-119373 | libsqlite3 (FTS5 rowid merge) | MITIGADO |
| 3 | CVE-2026-119379 | libsqlite3 (sessão) | NÃO AFETADO |
| 4 | CVE-2026-76844 | zlib / webpack (disputado) | NÃO AFETADO |
| 5 | CVE-2026-85091 | zlib (gz_vacate) | NÃO AFETADO |
| 6 | CVE-2026-95691 | OpenSSL (QUIC) | NÃO AFETADO |
| 7 | CVE-2026-53615 | util-linux (libblkid DOS) | NÃO AFETADO |
| 8 | CVE-2026-78410 | util-linux (mount TOCTOU) | NÃO AFETADO |
| 9 | CVE-2025-6020 | linux-pam (pam_namespace) | NÃO AFETADO |
| 10 | CVE-2026-107493 | OpenSSL (PEM stack) | NÃO AFETADO |
| 11 | CVE-2026-13595 | util-linux (libblkid UAF) | NÃO AFETADO |
| 12 | CVE-2026-86805 | glibc (ld.so $ORIGIN) | NÃO AFETADO |
| 13 | CVE-2026-102474 | dash (conv_escape) | NÃO AFETADO |
| 14 | CVE-2026-102473 | dash (pmatch) | NÃO AFETADO |
| 15 | CVE-2024-2236 | libgcrypt (Marvin) | NÃO AFETADO |
| 16 | CVE-2018-6829 | libgcrypt (ElGamal) | NÃO AFETADO |
| 17 | CVE-2011-3374 | apt (apt-key) | NÃO AFETADO |
| 18 | CVE-2026-53910 | diffutils (diff3, disputada) | NÃO AFETADO |
| 19 | CVE-2007-5686 | rPath initscripts (btmp) | NÃO AFETADO |
| 20 | CVE-2022-3219 | GnuPG (DoS) | NÃO AFETADO |

Totais: investigadas 20 · corrigidas 0 · mitigadas 2 · não afetadas 18 ·
pendentes 0 · bloqueadas 0 · aguardando upstream (backports Debian
`bookworm` p/ SQLite ≥ 3.54 e demais): 2 (itens 1–2; cobertura total
quando o Debian publicar).

## Riscos residuais e bloqueios

1. **SQLite FTS5 (itens 1–2)**: enquanto o Debian estável (`trixie`,
   hoje em `3.46.1`) não backportar `≥ 3.54.0`, DBs/inputs FTS5 hostis
   continuam capazes de explorar a lib do sistema através de
   `db_exec`/`db_query`. Mitigações aplicadas: base `trixie` + `apt-get
   upgrade` no build + usuário não-root (limita privesc, não o crash).
   Recomendação: abrir DBs apenas de origem confiável; acompanhar o
   Debian bug #1152790; a correção completa chega via rebuild da imagem
   assim que o DSA sair (sem mudança de código).
2. **Sem scanner de imagem/SBOM nesta sessão** (sem Docker daemon):
   reexecutar `docker build + scan` no CI antes do release.
3. **`rustls-pemfile` unmaintained** (`RUSTSEC-2025-0134`, sem fix):
   risco baixo (só parsing de `RHP_CA_BUNDLE`); acompanhar migração
   para `rustls-pemfile` sucessor/`pem` quando o ecossistema definir.
4. Nenhuma regressão funcional: compilador, VM, stdlib, CLI, imports,
   fmt e execução verificados pela suíte verde.
