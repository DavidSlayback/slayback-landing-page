#!/usr/bin/env bash
# Quiet-machine wrapper around bench.cjs (S1/S2 spikes): refuses to start while cargo/rustc run, gates on the
# empty-box baseline (settled avg <= 5.5 ms; the clean 2026-10-13 baseline was 4.3 ms), runs the variants, re-runs
# the baseline at the end, and reports any cargo/rustc process seen during the run (which invalidates it).
# usage: bash bench/measure.sh <out.jsonl> <variant>...   (from spikes/box3d, with web/ served on $PORT, default 8094)
set -uo pipefail
cd "$(dirname "$0")/.."
out="$1"; shift
port="${PORT:-8094}"
base="http://127.0.0.1:$port"
export NODE_PATH="${NODE_PATH:-/c/Users/david/repos/portfolio/tests/e2e/node_modules}"
compiles() { tasklist 2>/dev/null | grep -ciE '^(cargo|rustc)\.exe' ; }
baseline() {
  node bench/bench.cjs "$base" gpu "$out" "bench=1&burst=1&cubes=0&tag=$1" | grep settled
  python -X utf8 - "$out" "$1" <<'PY'
import json, sys
rows = [json.loads(l) for l in open(sys.argv[1], encoding="utf-8")]
r = [r for r in rows if r.get("variant", "").endswith("tag=" + sys.argv[2])][-1]
avg = next(x for x in r["reports"] if x.get("phase") == "settled")["interval_ms"]["avg"]
print(f"BASELINE {sys.argv[2]} settled avg {avg:.2f} ms")
sys.exit(0 if avg <= 5.5 else 3)
PY
}
[ "$(compiles)" -eq 0 ] || { echo "BUSY: cargo/rustc running, not measuring"; exit 2; }
log="$(mktemp)"
( while true; do c=$(compiles); [ "$c" -gt 0 ] && echo "$(date +%T) $c" >> "$log"; sleep 5; done ) &
watcher=$!
trap 'kill $watcher 2>/dev/null' EXIT
echo "START $(date +%T)"
baseline pre || { echo "BASELINE too slow, not measuring"; exit 3; }
node bench/bench.cjs "$base" gpu "$out" "$@"
baseline post || echo "WARN: post baseline above 5.5 ms"
echo "END $(date +%T)"
if [ -s "$log" ]; then echo "INVALID: compiles seen during the run:"; cat "$log"; exit 4; fi
echo "QUIET: no cargo/rustc seen during the run"
