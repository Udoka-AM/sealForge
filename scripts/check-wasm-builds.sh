#!/usr/bin/env bash
# Check the committed browser WebAssembly modules (issue #61): each BUILD.json
# records a build from a clean tree, and its wasm_sha256 is the committed
# .wasm. CI then rebuilds them in the pinned container and diffs the bytes.
set -euo pipefail
cd "$(dirname "$0")/.."
sha256() { if command -v sha256sum >/dev/null; then sha256sum "$@"; else shasum -a 256 "$@"; fi; }

fail=0
for m in auditor signer scanner wallet; do
  dir=apps/web/public/$m
  wasm=$(ls "$dir"/*_bg.wasm)
  read -r dirty recorded < <(python3 -c "import json,sys;d=json.load(open(sys.argv[1]));print(str(d['source_dirty']).lower(), d['wasm_sha256'])" "$dir/BUILD.json")
  actual=$(sha256 "$wasm" | cut -d' ' -f1)
  if [[ "$dirty" != false ]]; then
    echo "::error file=$dir/BUILD.json::$m was built from a dirty tree; commit the source and run scripts/build-wasm.sh $m"
    fail=1
  fi
  if [[ "$recorded" != "$actual" ]]; then
    echo "::error file=$dir/BUILD.json::$m: BUILD.json says $recorded, the committed module is $actual"
    fail=1
  fi
  echo "$m: clean=$([[ $dirty == false ]] && echo yes || echo no) sha256=${actual:0:16}"
done
exit $fail
