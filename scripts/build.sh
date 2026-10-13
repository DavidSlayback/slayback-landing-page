#!/usr/bin/env bash
# Builds the deployable site into dist/: static files from site/, hero.wasm from crates/hero-wasm, and the game
# (crates/game, Bevy) at dist/play/ via wasm-bindgen (pinned to the version in Cargo.lock).
# Preview with any static server, e.g. `python -m http.server -d dist` (WASM cannot load from file://).
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
cargo build -p hero-wasm --release --target wasm32-unknown-unknown --quiet
rm -rf dist && mkdir -p dist
cp -r site/. dist/
cp target/wasm32-unknown-unknown/release/hero_wasm.wasm dist/hero.wasm
if command -v wasm-opt >/dev/null; then wasm-opt -O3 --enable-simd --enable-bulk-memory --enable-sign-ext --enable-mutable-globals dist/hero.wasm -o dist/hero.wasm; fi
echo "dist/: hero.wasm $(( $(wc -c < dist/hero.wasm) / 1024 )) KB"

# The game (/play): Bevy → wasm-bindgen glue. Skip with SKIP_GAME=1 for hero-only builds, which also drop the
# /play host page: deploys are hero-only until the game's release switch, and a page with no game would hang.
if [ "${SKIP_GAME:-0}" = 1 ]; then
  rm -rf dist/play
else
  cargo build -p game --profile game --target wasm32-unknown-unknown --quiet
  wasm-bindgen --target web --no-typescript --out-dir dist/play --out-name game target/wasm32-unknown-unknown/game/game.wasm
  # The workspace builds with simd128, so wasm-opt must be told which post-MVP features the module already uses.
  if command -v wasm-opt >/dev/null; then
    wasm-opt -Oz --enable-simd --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext       --enable-mutable-globals --enable-reference-types --enable-multivalue dist/play/game_bg.wasm -o dist/play/game_bg.wasm
  fi
  echo "dist/play/: game_bg.wasm $(( $(wc -c < dist/play/game_bg.wasm) / 1024 )) KB ($(( $(gzip -9 -c dist/play/game_bg.wasm | wc -c) / 1024 )) KB gzipped)"
fi
