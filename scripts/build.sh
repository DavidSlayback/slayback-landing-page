#!/usr/bin/env bash
# Builds the deployable site into dist/: static files from site/, hero.wasm from crates/hero-wasm, and the game
# (Bevy) at dist/play/ via wasm-bindgen (pinned to the version in Cargo.lock).
# Preview with any static server, e.g. `python -m http.server -d dist` (WASM cannot load from file://).
#   SKIP_GAME=1     hero-only build: no /play (what deploys ship)
#   GAME=game3d     which game crate /play serves: `game` (the 2.5D game, the default) or `game3d` (the 3D port,
#                   spec #58). Both ship as dist/play/game.js + game_bg.wasm, so the host page is the same.
#   GAME_RELEASE=1  the game's release switch: Play nav item, hero call to action, easter egg and the touch-device
#                   clip page. Off by default; it needs the game, so it refuses SKIP_GAME=1. Deploys never set it.
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
target="${CARGO_TARGET_DIR:-target}"
release="${GAME_RELEASE:-0}"
game="${GAME:-game}"
case "$game" in
  game | game3d) ;;
  *) echo "GAME must be game or game3d, not '$game'" >&2; exit 1 ;;
esac
if [ "$release" = 1 ] && [ "${SKIP_GAME:-0}" = 1 ]; then
  echo "GAME_RELEASE=1 needs the game in the build: unset SKIP_GAME" >&2
  exit 1
fi
cargo build -p hero-wasm --release --target wasm32-unknown-unknown --quiet
rm -rf dist && mkdir -p dist
cp -r site/. dist/
cp "$target/wasm32-unknown-unknown/release/hero_wasm.wasm" dist/hero.wasm
if command -v wasm-opt >/dev/null; then wasm-opt -O3 --enable-simd --enable-bulk-memory --enable-sign-ext --enable-mutable-globals dist/hero.wasm -o dist/hero.wasm; fi
echo "dist/: hero.wasm $(( $(wc -c < dist/hero.wasm) / 1024 )) KB"

# Release switch. site/ marks switch-only content with paired comments, either alone on the lines around a block
# or inline within one line: <!-- GAME_RELEASE --> ... <!-- /GAME_RELEASE --> (/* GAME_RELEASE */ in CSS and JS).
# Off deletes the marked content, so the output is the pre-switch site byte for byte; on deletes only the markers.
# Comment markers keep site/ plain, valid HTML, CSS and JS: no template engine, no generated files.
open='(?:<!--|/\*) GAME_RELEASE (?:-->|\*/)' close='(?:<!--|/\*) /GAME_RELEASE (?:-->|\*/)'
for f in $(grep -rlI 'GAME_RELEASE' dist || true); do
  if [ "$release" = 1 ]; then
    perl -0777 -pi -e "s{^[ \t]*(?:$open|$close)[ \t]*\r?\n}{}gm; s{$open|$close}{}g" "$f"
  else
    perl -0777 -pi -e "s{^[ \t]*$open[ \t]*\r?\n.*?^[ \t]*$close[ \t]*\r?\n}{}gms; s{$open.*?$close}{}g" "$f"
  fi
done
[ "$release" = 1 ] || rm -rf dist/play/watch   # the touch-device clip page
echo "dist/: game release switch $([ "$release" = 1 ] && echo ON || echo off)"

# The game (/play): Bevy → wasm-bindgen glue. Skip with SKIP_GAME=1 for hero-only builds, which also drop the
# /play host page: deploys are hero-only until the game's release switch, and a page with no game would hang.
if [ "${SKIP_GAME:-0}" = 1 ]; then
  rm -rf dist/play
else
  cargo build -p "$game" --profile game --target wasm32-unknown-unknown --quiet
  wasm-bindgen --target web --no-typescript --out-dir dist/play --out-name game "$target/wasm32-unknown-unknown/game/$game.wasm"
  # The workspace builds with simd128, so wasm-opt must be told which post-MVP features the module already uses.
  if command -v wasm-opt >/dev/null; then
    wasm-opt -Oz --enable-simd --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext       --enable-mutable-globals --enable-reference-types --enable-multivalue dist/play/game_bg.wasm -o dist/play/game_bg.wasm
  fi
  echo "dist/play/ ($game): game_bg.wasm $(( $(wc -c < dist/play/game_bg.wasm) / 1024 )) KB ($(( $(gzip -9 -c dist/play/game_bg.wasm | wc -c) / 1024 )) KB gzipped)"
fi
