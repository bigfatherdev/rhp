#!/bin/bash
# tools/docker-http-proof.sh — Prova real de rede do `http_serve` (0.6.0).
#
# Valida o bind configuravel contra Docker de verdade:
#   1. RHP escutando em 0.0.0.0 dentro do container;
#   2. acesso via IP interno do container, a partir de OUTRO container;
#   3. acesso via reverse proxy (nginx) em container separado;
#   4. port forwarding do Docker (host -> proxy -> RHP);
#   5. controle negativo: bind padrao (127.0.0.1) e inalancavel entre containers.
#
# Uso: ./tools/docker-http-proof.sh [binario-rhp]
#   (padrao: ./target/release/rhp; precisa de `cargo build --release` antes)
#
# Requer: docker, rede bridge disponivel, imagens debian:trixie-slim e
# nginx:alpine (baixadas se ausentes). Nao executa nada como root no host
# alem do proprio docker. Limpa containers/rede ao sair (trap).
set -u
RHP="${1:-./target/release/rhp}"
test -x "$RHP" || { echo "binario nao encontrado: $RHP" >&2; exit 2; }
command -v docker >/dev/null || { echo "docker nao disponivel" >&2; exit 2; }

NET="rhp-proof-$$"
SRV="rhp-proof-srv-$$"
PROXY="rhp-proof-proxy-$$"
TMP="$(mktemp -d)"
cleanup() {
  docker rm -f "$SRV" "$PROXY" >/dev/null 2>&1
  docker network rm "$NET" >/dev/null 2>&1
  rm -rf "$TMP"
}
trap cleanup EXIT

pass=0; fail=0
check() { # check <nome> <esperado> <obteve>
  if [ "$2" = "$3" ]; then pass=$((pass+1)); echo "ok: $1";
  else fail=$((fail+1)); echo "FALHOU: $1 (esperado [$2], obteve [$3])"; fi
}

cat > "$TMP/api.rhp" <<'EOF'
fn app(req) {
    return "docker-ok:" + req["caminho"];
}
http_serve(8081, "app", {"host": "0.0.0.0", "max_requisicoes": 0});
EOF
cat > "$TMP/nginx.conf" <<'EOF'
events {}
http { server { listen 80; location / { proxy_pass http://SRVNAME:8081; } } }
EOF
sed -i "s/SRVNAME/$SRV/" "$TMP/nginx.conf"

docker network create "$NET" >/dev/null
docker run -d --name "$SRV" --network "$NET" \
  -v "$TMP/api.rhp:/app/api.rhp:ro" -v "$PWD/$RHP:/usr/local/bin/rhp:ro" \
  debian:trixie-slim /usr/local/bin/rhp run /app/api.rhp >/dev/null
sleep 3
ANUNCIO="$(docker logs "$SRV" 2>&1 | head -1)"
check "anuncio 0.0.0.0" "rhp: ouvindo em 0.0.0.0:8081" "$ANUNCIO"
IP="$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$SRV")"
R1="$(docker run --rm --network "$NET" curlimages/curl -s -m 10 "http://$IP:8081/interno" 2>/dev/null || echo CURL_FALHOU)"
check "container->container via IP interno" "docker-ok:/interno" "$R1"

docker run -d --name "$PROXY" --network "$NET" \
  -v "$TMP/nginx.conf:/etc/nginx/nginx.conf:ro" -p 18081:80 nginx:alpine >/dev/null
sleep 3
R2="$(docker run --rm --network "$NET" curlimages/curl -s -m 10 "http://$PROXY/via-proxy" 2>/dev/null || echo CURL_FALHOU)"
check "reverse proxy em container separado" "docker-ok:/via-proxy" "$R2"
R3="$(curl -s -m 10 "http://127.0.0.1:18081/via-porta" 2>/dev/null || echo CURL_FALHOU)"
check "port forwarding do Docker" "docker-ok:/via-porta" "$R3"

# Controle negativo: bind padrao nao atravessa containers.
cat > "$TMP/local.rhp" <<'EOF'
fn app(req) {
    return "local";
}
http_serve(8082, "app", {"max_requisicoes": 1});
EOF
docker rm -f "$SRV" >/dev/null
docker run -d --name "$SRV" --network "$NET" \
  -v "$TMP/local.rhp:/app/a.rhp:ro" -v "$PWD/$RHP:/usr/local/bin/rhp:ro" \
  debian:trixie-slim /usr/local/bin/rhp run /app/a.rhp >/dev/null
sleep 2
IP2="$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$SRV")"
if docker run --rm --network "$NET" curlimages/curl -s -m 8 "http://$IP2:8082/x" >/dev/null 2>&1; then
  NEG="alcancado (ERRO: default nao deveria alcancar)"
else
  NEG="recusado"
fi
check "controle negativo (127.0.0.1 entre containers)" "recusado" "$NEG"

echo "---"
echo "pass=$pass fail=$fail"
test "$fail" -eq 0
