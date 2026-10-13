#!/usr/bin/env bash
# B2 spike: the same post-link steps as scripts/build.sh for one raw cargo wasm, then raw and gzip -9 sizes.
# usage: bash ship.sh <input.wasm> <out-dir> <out-name>
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
in="$1" out="$2" name="$3"
mkdir -p "$out"
wasm-bindgen --target web --no-typescript --out-dir "$out" --out-name "$name" "$in"
wasm-opt -Oz --enable-simd --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
  --enable-mutable-globals --enable-reference-types --enable-multivalue "$out/${name}_bg.wasm" -o "$out/${name}_bg.wasm"
echo "$name raw $(wc -c < "$out/${name}_bg.wasm") B, gzip -9 $(gzip -9 -c "$out/${name}_bg.wasm" | wc -c) B"
