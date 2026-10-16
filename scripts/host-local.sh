#!/usr/bin/env bash
# Temporary public hosting from this machine (until a server exists).
#
#   scripts/host-local.sh start | stop | status
#
# Runs, against a public lightwallet server (no node to sync):
#   coordinator   127.0.0.1:8090  (not exposed: the website proxies to it)
#   website       127.0.0.1:3200  -> public https://…trycloudflare.com
#   Envoy         127.0.0.1:8082  -> public https://…trycloudflare.com (gRPC-web, for the browser audit)
# Cloudflare quick tunnels need no account and open no router ports. Their URLs
# change on every start, and nothing is served while this machine sleeps.
#
# Settings: SF_HOST_NETWORK (mainnet | testnet, default mainnet),
#           SF_HOST_LIGHTWALLETD (default zec.rocks / testnet.zec.rocks).
# Fixed hostnames instead of quick tunnels: put these in ~/.sealforge-host/<network>/tunnel.env
#   SF_HOST_TUNNEL=<named tunnel>      (cloudflared tunnel create …; route dns for both hosts)
#   SF_HOST_WEB_HOST=sealforge.example.com
#   SF_HOST_LWD_HOST=sealforge-lwd.example.com
# Test networks only: put SF_MIN_WINDOW_BLOCKS / SF_REFUND_DELAY_SECS in
# ~/.sealforge-host/testnet/demo.env to fit a whole auction into about an hour.
# State lives in ~/.sealforge-host/<network>/ (0700): host.env holds the data
# key and organiser invite. Back up SF_DATA_KEY: escrows can't be spent without it.
set -euo pipefail
cd "$(dirname "$0")/.."

NETWORK=${SF_HOST_NETWORK:-mainnet}
case "$NETWORK" in
  mainnet) LWD=${SF_HOST_LIGHTWALLETD:-zec.rocks} ;;
  testnet) LWD=${SF_HOST_LIGHTWALLETD:-testnet.zec.rocks} ;;
  *) echo "SF_HOST_NETWORK must be mainnet or testnet" >&2; exit 1 ;;
esac
DIR=${SF_HOST_DIR:-$HOME/.sealforge-host}/$NETWORK
ENVFILE=$DIR/host.env
ENVOY=sealforge-host-envoy-$NETWORK
DB=sealforge_$NETWORK
COORD_PORT=8090 WEB_PORT=3200 ENVOY_PORT=8082

mkdir -p "$DIR/logs" "$DIR/pids" "$DIR/wallets"
chmod 700 "${SF_HOST_DIR:-$HOME/.sealforge-host}" "$DIR"

if [[ ! -f "$ENVFILE" ]]; then
  umask 077
  cat > "$ENVFILE" <<EOF
# Created $(date -u +%FT%TZ). Keep a copy of SF_DATA_KEY in your password manager.
SF_DATA_KEY=$(openssl rand -hex 32)
SF_INVITE_CODES=$(openssl rand -hex 8)
EOF
fi
# shellcheck disable=SC1090
source "$ENVFILE"
# shellcheck disable=SC1090
[[ -f "$DIR/tunnel.env" ]] && source "$DIR/tunnel.env"
SF_HOST_TUNNEL=${SF_HOST_TUNNEL:-}
DEMO_ENV=()
if [[ -f "$DIR/demo.env" ]]; then
  [[ "$NETWORK" == mainnet ]] && { echo "demo.env is for test networks only" >&2; exit 1; }
  while IFS= read -r line; do
    [[ "$line" =~ ^(SF_MIN_WINDOW_BLOCKS|SF_REFUND_DELAY_SECS)=[0-9,]+$ ]] && DEMO_ENV+=("$line")
  done < "$DIR/demo.env"
fi

running() { [[ -f "$DIR/pids/$1.pid" ]] && kill -0 "$(cat "$DIR/pids/$1.pid")" 2>/dev/null; }

launch() { # name, then the command
  local name=$1; shift
  running "$name" && { echo "$name already running"; return; }
  nohup "$@" > "$DIR/logs/$name.log" 2>&1 &
  echo $! > "$DIR/pids/$name.pid"
}

tunnel_url() { # wait for cloudflared to print its URL
  for _ in $(seq 1 60); do
    url=$(grep -oE 'https://[a-z0-9-]+\.trycloudflare\.com' "$DIR/logs/$1.log" 2>/dev/null | head -1 || true)
    [[ -n "$url" ]] && { echo "$url"; return; }
    sleep 1
  done
  echo "tunnel $1 did not come up; see $DIR/logs/$1.log" >&2
  return 1
}

start() {
  docker compose -f infra/docker-compose.yml up -d postgres >/dev/null
  for _ in $(seq 1 30); do docker exec sealforge-postgres-1 pg_isready -U sealforge >/dev/null 2>&1 && break; sleep 1; done
  docker exec sealforge-postgres-1 psql -U sealforge -d postgres -tAc \
    "SELECT 1 FROM pg_database WHERE datname = '$DB'" | grep -q 1 ||
    docker exec sealforge-postgres-1 createdb -U sealforge "$DB"

  echo "building coordinator and website…"
  cargo build --release -q -p sealforge-coordinator -p sealforge-cli
  launch coordinator env DATABASE_URL="postgres://sealforge:sealforge@localhost:5432/$DB" \
    SF_LISTEN_ADDR=127.0.0.1:$COORD_PORT SF_NETWORK="$NETWORK" SF_ZAINO_URL="https://$LWD:443" \
    SF_WALLET_DIR="$DIR/wallets" SF_DATA_KEY="$SF_DATA_KEY" SF_INVITE_CODES="$SF_INVITE_CODES" \
    ${DEMO_ENV[@]+"${DEMO_ENV[@]}"} RUST_LOG=info ./target/release/sealforge-coordinator

  local envoy_cfg="$DIR/envoy.yaml"
  sed "s|UPSTREAM_HOST|$LWD|g" infra/envoy-tls.yaml > "$envoy_cfg"
  docker rm -f "$ENVOY" >/dev/null 2>&1 || true
  docker run -d --name "$ENVOY" -p 127.0.0.1:$ENVOY_PORT:8081 \
    -v "$envoy_cfg:/etc/envoy/envoy.yaml:ro" envoyproxy/envoy:v1.31-latest >/dev/null

  local web_url grpc_url
  if [[ -n "$SF_HOST_TUNNEL" ]]; then
    # Named tunnel: fixed hostnames, one cloudflared process.
    cat > "$DIR/cloudflared.yml" <<EOF
tunnel: $SF_HOST_TUNNEL
ingress:
  - hostname: $SF_HOST_WEB_HOST
    service: http://127.0.0.1:$WEB_PORT
  - hostname: $SF_HOST_LWD_HOST
    service: http://127.0.0.1:$ENVOY_PORT
  - service: http_status:404
EOF
    launch tunnel-web cloudflared tunnel --no-autoupdate --config "$DIR/cloudflared.yml" run "$SF_HOST_TUNNEL"
    web_url="https://$SF_HOST_WEB_HOST"
    grpc_url="https://$SF_HOST_LWD_HOST"
  else
    launch tunnel-web cloudflared tunnel --no-autoupdate --url "http://127.0.0.1:$WEB_PORT"
    launch tunnel-grpc cloudflared tunnel --no-autoupdate --url "http://127.0.0.1:$ENVOY_PORT"
    web_url=$(tunnel_url tunnel-web)
    grpc_url=$(tunnel_url tunnel-grpc)
  fi

  pnpm --filter @sealforge/web build >/dev/null
  launch web env COORDINATOR_URL="http://127.0.0.1:$COORD_PORT" LIGHTWALLETD_GRPCWEB="$grpc_url" \
    NEXT_PUBLIC_DEFAULT_NETWORK="$NETWORK" \
    pnpm --filter @sealforge/web exec next start -p $WEB_PORT -H 127.0.0.1

  # Keep the machine from idle-sleeping while hosting (macOS; a closed lid on battery still sleeps).
  command -v caffeinate >/dev/null && launch caffeinate caffeinate -ims
  printf 'WEB_URL=%s\nGRPC_WEB_URL=%s\n' "$web_url" "$grpc_url" > "$DIR/urls"
  echo
  echo "sealForge ($NETWORK) is up:"
  echo "  website      $web_url"
  echo "  gRPC-web     $grpc_url  (browser audit)"
  echo "  coordinator  http://127.0.0.1:$COORD_PORT (local only)"
  echo "Register as organiser:  ./target/release/sealforge --coordinator http://127.0.0.1:$COORD_PORT init --invite <SF_INVITE_CODES in $ENVFILE> --name \"<your name>\""
}

stop() {
  for name in web tunnel-web tunnel-grpc coordinator caffeinate; do
    if running "$name"; then kill "$(cat "$DIR/pids/$name.pid")" && echo "stopped $name"; fi
    rm -f "$DIR/pids/$name.pid"
  done
  docker rm -f "$ENVOY" >/dev/null 2>&1 && echo "stopped envoy" || true
}

status() {
  for name in coordinator web tunnel-web tunnel-grpc caffeinate; do
    running "$name" && echo "running  $name" || echo "stopped  $name"
  done
  docker ps --format '{{.Names}}' | grep -qx "$ENVOY" && echo "running  envoy" || echo "stopped  envoy"
  [[ -f "$DIR/urls" ]] && cat "$DIR/urls"
  curl -s "http://127.0.0.1:$COORD_PORT/readyz" && echo
}

case "${1:-}" in
  start) start ;;
  stop) stop ;;
  status) status ;;
  *) echo "usage: scripts/host-local.sh start | stop | status" >&2; exit 1 ;;
esac
