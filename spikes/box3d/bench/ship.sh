#!/usr/bin/env bash
# 3D spike: wasm-bindgen (+ wasm-opt -Oz for the size build, as scripts/build.sh does) into web/, then sizes.
# usage: bash bench/ship.sh <release|game>   (run from spikes/box3d after `cargo build --profile <p> --target wasm32-unknown-unknown`)
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
cd "$(dirname "$0")/.."
profile="$1"
in="target/wasm32-unknown-unknown/$profile/box3d.wasm"
out="web"
[ "$profile" = game ] && out="target/size"
mkdir -p "$out"
wasm-bindgen --target web --no-typescript --out-dir "$out" --out-name box3d "$in"
if [ "$profile" = game ]; then
  wasm-opt -Oz --enable-simd --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
    --enable-mutable-globals --enable-reference-types --enable-multivalue "$out/box3d_bg.wasm" -o "$out/box3d_bg.wasm"
fi
echo "$profile: box3d_bg.wasm raw $(wc -c < "$out/box3d_bg.wasm") B, gzip -9 $(gzip -9 -c "$out/box3d_bg.wasm" | wc -c) B"
