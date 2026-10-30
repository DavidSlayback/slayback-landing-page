#!/usr/bin/env bash
# Bench v1's regression gate: today's bench crates (bench-core, box-bench) re-analyse the published v1 sweeps to their
# published numbers. The sweeps' traces replay only on the game they were recorded on, so this builds a worktree at
# fbc84d8 (the commit that published them; the game is the same at each sweep's own commit), drops today's bench
# crates in, and runs crates/box-bench/tests/v1.rs there. Run it on Windows, where they were recorded: on Linux the
# game's floats drift and most runs don't replay. Three shims bridge what the bench crates use and that
# commit lacks, none of them gameplay: box-mcp's TOOL_NAMES made public, box-env re-exporting the game's FPS, and
# box-bench's main reading the old INSTRUCTIONS constant.
#
#   bash scripts/bench-v1-gate.sh [worktree dir]   (a cold release build of the game: about ten minutes)
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
dir=${1:-$(mktemp -d)/bench-v1-gate}
git -C "$repo" worktree add -q --detach "$dir" fbc84d8
trap 'git -C "$repo" worktree remove --force "$dir"' EXIT
cp -r "$repo/crates/bench-core" "$dir/crates/"
rm -rf "$dir/crates/box-bench/src" "$dir/crates/box-bench/tests"
cp -r "$repo/crates/box-bench/src" "$repo/crates/box-bench/tests" "$repo/crates/box-bench/Cargo.toml" "$dir/crates/box-bench/"
sed -i 's#"crates/box-bench"\]#"crates/box-bench", "crates/bench-core"]#' "$dir/Cargo.toml"
sed -i 's/^const TOOL_NAMES/pub const TOOL_NAMES/' "$dir/crates/box-mcp/src/lib.rs"
sed -i 's/^pub use game_core::agent::{ACTIONS, Action, FLAT};/&\npub use game3d::headless::FPS;/' "$dir/crates/box-env/src/lib.rs"
sed -i 's/use box_mcp::instructions;/use box_mcp::INSTRUCTIONS;/; s/        instructions()$/        INSTRUCTIONS/' \
  "$dir/crates/box-bench/src/main.rs"
cd "$dir"
bash scripts/native.sh test -q -p box-bench --no-default-features --release --test v1 -- --ignored
