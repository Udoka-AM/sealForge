#!/usr/bin/env bash
# Build the trustee signer (crates/sealforge-signer-wasm) for the browser.
# Output: apps/web/public/signer/{sealforge_signer_wasm.js, sealforge_signer_wasm_bg.wasm, BUILD.json}
#
# Requires: the toolchain from rust-toolchain.toml, wasm-bindgen-cli at the
# same version as the wasm-bindgen crate in Cargo.lock, and an LLVM clang with the
# wasm32 target for the C code the PCZT crates pull in (secp256k1)
# (macOS: brew install llvm).
set -euo pipefail
sha256() { if command -v sha256sum >/dev/null; then sha256sum "$@"; else shasum -a 256 "$@"; fi; }
cd "$(dirname "$0")/.."

want=$(grep -A1 'name = "wasm-bindgen"$' Cargo.lock | grep version | head -1 | cut -d'"' -f2)
have=$(wasm-bindgen --version 2>/dev/null | awk '{print $2}' || true)
if [[ "$have" != "$want" ]]; then
  echo "wasm-bindgen-cli $want is required (found: ${have:-none})." >&2
  echo "Install it with: cargo install wasm-bindgen-cli --version $want --locked" >&2
  exit 1
fi

LLVM=${LLVM_BIN:-$(brew --prefix llvm 2>/dev/null)/bin}
export CC_wasm32_unknown_unknown="$LLVM/clang" AR_wasm32_unknown_unknown="$LLVM/llvm-ar"

out=apps/web/public/signer
# Strip local paths from the binary so builds don't depend on where the repo lives.
RUSTFLAGS="--remap-path-prefix=$(pwd)=/sealforge --remap-path-prefix=${CARGO_HOME:-$HOME/.cargo}=/cargo" \
  cargo build --release -p sealforge-signer-wasm --lib --target wasm32-unknown-unknown
rm -rf "$out"
wasm-bindgen --target web --no-typescript --out-dir "$out" \
  "${CARGO_TARGET_DIR:-target}"/wasm32-unknown-unknown/release/sealforge_signer_wasm.wasm

cat > "$out/BUILD.json" <<JSON
{
  "crate": "sealforge-signer-wasm",
  "version": "$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)",
  "git_commit": "$(git rev-parse HEAD)",
  "source_dirty": $(git diff --quiet HEAD -- crates Cargo.toml Cargo.lock && echo false || echo true),
  "rustc": "$(rustc --version)",
  "clang": "$("$LLVM/clang" --version | head -1)",
  "wasm_bindgen": "$want",
  "wasm_sha256": "$(sha256 "$out/sealforge_signer_wasm_bg.wasm" | cut -d' ' -f1)"
}
JSON
echo "Built $out ($(du -h "$out/sealforge_signer_wasm_bg.wasm" | cut -f1))"
