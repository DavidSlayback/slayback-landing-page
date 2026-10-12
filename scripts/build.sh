#!/usr/bin/env bash
# Builds the deployable site into dist/: static files from site/ plus hero.wasm from crates/hero-wasm.
# Preview with any static server, e.g. `python -m http.server -d dist` (WASM cannot load from file://).
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
cargo build -p hero-wasm --release --target wasm32-unknown-unknown --quiet
rm -rf dist && mkdir -p dist
cp -r site/. dist/
cp target/wasm32-unknown-unknown/release/hero_wasm.wasm dist/hero.wasm
if command -v wasm-opt >/dev/null; then wasm-opt -O3 --enable-simd dist/hero.wasm -o dist/hero.wasm; fi
echo "dist/: hero.wasm $(( $(wc -c < dist/hero.wasm) / 1024 )) KB"
