#!/usr/bin/env bash
# Build the bidder wallet (crates/sealforge-wallet) for the browser wallet (task 4.7).
# Output: apps/web/public/wallet/{sealforge_wallet.js, sealforge_wallet_bg.wasm, BUILD.json}
#
# Requires: the toolchain from rust-toolchain.toml and wasm-bindgen-cli at the
# same version as the wasm-bindgen crate in Cargo.lock, and an LLVM clang with the wasm32
# target for the secp256k1 C code the transaction parser depends on
# (macOS: brew install llvm).
set -euo pipefail
cd "$(dirname "$0")/.."

want=$(grep -A1 'name = "wasm-bindgen"$' Cargo.lock | grep version | head -1 | cut -d'"' -f2)
have=$(wasm-bindgen --version 2>/dev/null | awk '{print $2}' || true)
if [[ "$have" != "$want" ]]; then
  echo "wasm-bindgen-cli $want is required (found: ${have:-none})." >&2
  echo "Install it with: cargo install wasm-bindgen-cli --version $want --locked" >&2
  exit 1
fi

out=apps/web/public/wallet
LLVM=${LLVM_BIN:-$(brew --prefix llvm 2>/dev/null)/bin}
export CC_wasm32_unknown_unknown="$LLVM/clang" AR_wasm32_unknown_unknown="$LLVM/llvm-ar"
# Strip local paths from the binary so builds don't depend on where the repo lives.
RUSTFLAGS="--cfg getrandom_backend=\"wasm_js\" --remap-path-prefix=$(pwd)=/sealforge --remap-path-prefix=$HOME/.cargo=/cargo" \
  cargo build --release -p sealforge-wallet --lib --target wasm32-unknown-unknown --features wasm
rm -rf "$out"
wasm-bindgen --target web --no-typescript --out-dir "$out" \
  target/wasm32-unknown-unknown/release/sealforge_wallet.wasm

cat > "$out/BUILD.json" <<JSON
{
  "crate": "sealforge-wallet",
  "version": "$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)",
  "git_commit": "$(git rev-parse HEAD)",
  "source_dirty": $(git diff --quiet HEAD -- crates Cargo.toml Cargo.lock && echo false || echo true),
  "rustc": "$(rustc --version)",
  "wasm_bindgen": "$want",
  "wasm_sha256": "$(shasum -a 256 "$out/sealforge_wallet_bg.wasm" | cut -d' ' -f1)"
}
JSON
echo "Built $out ($(du -h "$out/sealforge_wallet_bg.wasm" | cut -f1))"
