#!/usr/bin/env bash
# Local regtest chain helper. See infra/regtest/README.md.
set -euo pipefail
cd "$(dirname "$0")/.."

IMAGE=${ZEBRA_REGTEST_IMAGE:-zfnd/zebra:7.0.0-rc.0}
# Zaino publishes amd64 images only; Docker emulates them on Apple silicon.
ZAINO_IMAGE=${ZAINO_REGTEST_IMAGE:-zingodevops/zaino:0.10.2-no-tls}
NAME=sealforge-regtest
ZAINO=sealforge-regtest-zaino
NET=sealforge-regtest-net
RPC=${ZEBRA_RPC:-http://127.0.0.1:18232}

rpc() {
  curl -sf -H 'content-type: application/json' \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"$1\",\"params\":${2:-[]}}" "$RPC"
}

case "${1:-}" in
  up)
    address=${2:?usage: regtest.sh up <miner-address>}
    config=$(mktemp)
    sed "s|MINER_ADDRESS|$address|" infra/regtest/zebrad.toml > "$config"
    docker rm -f "$NAME" "$ZAINO" >/dev/null 2>&1 || true
    docker network create "$NET" >/dev/null 2>&1 || true
    docker run -d --name "$NAME" --network "$NET" -p 127.0.0.1:18232:18232 \
      -v sealforge-regtest-state:/home/zebra/.cache/zebra \
      -v "$config:/etc/zebrad/zebrad.toml:ro" \
      --entrypoint zebrad "$IMAGE" -c /etc/zebrad/zebrad.toml start >/dev/null
    for _ in $(seq 1 60); do rpc getblockcount >/dev/null 2>&1 && break; sleep 1; done
    echo "Zebra regtest up at $RPC (height $(rpc getblockcount | sed 's/.*"result":\([0-9]*\).*/\1/'))"
    docker run -d --name "$ZAINO" --network "$NET" --platform linux/amd64 -p 127.0.0.1:8137:8137 \
      -v "$(pwd)/infra/regtest/zainod.toml:/app/config/zainod.toml:ro" \
      "$ZAINO_IMAGE" start >/dev/null
    echo "Zaino (lightwallet gRPC) starting at 127.0.0.1:8137"
    ;;
  mine)
    rpc generate "[${2:-1}]" | sed 's/.*"result":\[\(.\{0,80\}\).*/mined: \1.../'
    ;;
  height)
    rpc getblockcount
    ;;
  rpc)
    rpc "${2:?method}" "${3:-[]}"
    ;;
  logs)
    docker logs --tail "${2:-50}" "$NAME"
    ;;
  zaino-logs)
    docker logs --tail "${2:-50}" "$ZAINO"
    ;;
  down)
    docker rm -f "$NAME" "$ZAINO" >/dev/null 2>&1 || true
    [[ "${2:-}" == "--wipe" ]] && docker volume rm sealforge-regtest-state >/dev/null 2>&1 || true
    echo "stopped"
    ;;
  *)
    echo "usage: regtest.sh up <miner-address> | mine [n] | height | rpc <method> [params] | logs [n] | zaino-logs [n] | down [--wipe]" >&2
    exit 1
    ;;
esac
