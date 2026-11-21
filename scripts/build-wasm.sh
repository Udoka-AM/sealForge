#!/usr/bin/env bash
# Build the browser WebAssembly modules reproducibly, inside the pinned image
# in infra/wasm-build (issue #61). Same bytes on any machine with Docker.
#
#   scripts/build-wasm.sh                 # all four modules
#   scripts/build-wasm.sh signer wallet   # some of them
#
# Commit your source changes first: BUILD.json records the commit and whether
# the tree was clean, and CI rejects a module built from a dirty tree or one
# that doesn't match a fresh build of the source.
#
# WASM_BUILD_IMAGE: use an image you already built (CI does).
set -euo pipefail
cd "$(dirname "$0")/.."

modules=("$@")
[[ ${#modules[@]} -eq 0 ]] && modules=(auditor signer scanner wallet)
for m in "${modules[@]}"; do
  [[ -x "scripts/build-$m-wasm.sh" ]] || { echo "unknown module: $m" >&2; exit 1; }
done

image=${WASM_BUILD_IMAGE:-}
if [[ -z "$image" ]]; then
  tag=$( (cat infra/wasm-build/Dockerfile) | git hash-object --stdin | cut -c1-12)
  image="sealforge-wasm-build:$tag"
  docker image inspect "$image" >/dev/null 2>&1 \
    || docker build -t "$image" infra/wasm-build
fi

# Caches live outside the repository and the host target/: Docker volumes, or
# host directories under WASM_CACHE_DIR (CI caches those between runs).
if [[ -n "${WASM_CACHE_DIR:-}" ]]; then
  mkdir -p "$WASM_CACHE_DIR/registry" "$WASM_CACHE_DIR/target"
  registry="$WASM_CACHE_DIR/registry" target="$WASM_CACHE_DIR/target"
else
  registry=sealforge-wasm-registry target=sealforge-wasm-target
fi
cmd=""
for m in "${modules[@]}"; do cmd+="scripts/build-$m-wasm.sh && "; done
docker run --rm \
  -v "$PWD":/sealforge \
  -v "$registry":/usr/local/cargo/registry \
  -v "$target":/target \
  "$image" bash -c "${cmd}true"
