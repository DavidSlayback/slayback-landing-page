#!/usr/bin/env bash
# Quiet-machine wrapper around bench.cjs (S1/S2 spikes). Another build on the machine makes GPU/CPU timings invalid,
# so each variant runs only after 60 s with no cargo/rustc process, and a variant during which any cargo/rustc
# process appeared goes to <out>.invalid.jsonl and is retried (up to 3 tries). The empty-box baseline runs first and
# again every 4 variants; above 5.5 ms settled avg (the clean 2026-10-13 baseline was 4.3 ms) it waits and retries.
# usage: bash bench/measure.sh <out.jsonl> <variant>...   (from spikes/box3d, with web/ served on $PORT, default 8094)
set -uo pipefail
cd "$(dirname "$0")/.."
out="$1"; shift
port="${PORT:-8094}"
base="http://127.0.0.1:$port"
export NODE_PATH="${NODE_PATH:-/c/Users/david/repos/portfolio/tests/e2e/node_modules}"
tmp="$(mktemp -d)"
compiles() { tasklist 2>/dev/null | grep -ciE '^(cargo|rustc)\.exe' ; }
wait_quiet() {  # 6 x 10 s with no compile, up to ~30 min
  local q=0 i
  for i in $(seq 1 180); do
    if [ "$(compiles)" -eq 0 ]; then q=$((q + 1)); else q=0; fi
    [ $q -ge 6 ] && return 0
    sleep 10
  done
  echo "GAVE UP waiting for a quiet machine $(date +%T)"; exit 2
}
# run_one <variant>: 0 = clean (row appended to $out), 1 = a compile overlapped (row to $out.invalid.jsonl)
run_one() {
  local v="$1" log="$tmp/watch" row="$tmp/row.jsonl"
  : > "$log"; rm -f "$row"
  ( while true; do c=$(compiles); [ "$c" -gt 0 ] && echo "$(date +%T) $c" >> "$log"; sleep 3; done ) &
  local w=$!
  node bench/bench.cjs "$base" gpu "$row" "$v" | grep -E "interval|TIMEOUT|ERR" | sed "s/^/  /"
  kill $w 2>/dev/null; wait $w 2>/dev/null
  if [ -s "$log" ]; then
    tail -n +2 "$row" | sed 's/^{/{"invalid":"compile overlapped",/' >> "${out%.jsonl}.invalid.jsonl"
    echo "  INVALID (compile seen $(head -1 "$log"))"; return 1
  fi
  head -1 "$row" >> "$out.renderer"; tail -n +2 "$row" >> "$out"; return 0
}
settled_avg() {  # settled avg of the last row of $out
  python -X utf8 -c "import json,sys; r=[json.loads(l) for l in open(sys.argv[1],encoding='utf-8')][-1]; print(next(x for x in r['reports'] if x.get('phase')=='settled')['interval_ms']['avg'])" "$out"
}
measure() {  # measure <variant> [baseline]: retry until clean (3 tries)
  local t
  for t in 1 2 3; do
    wait_quiet
    echo "$(date +%T) $1"
    if run_one "$1"; then
      if [ "${2:-}" = baseline ]; then
        local a; a=$(settled_avg)
        echo "  BASELINE settled avg $a ms"
        python -c "import sys; sys.exit(0 if float(sys.argv[1]) <= 5.5 else 1)" "$a" && return 0
        echo "  baseline too slow, waiting 3 min"; sleep 180; continue
      fi
      return 0
    fi
  done
  echo "  FAILED after 3 tries: $1"
}
echo "START $(date +%T)"
n=0
measure "bench=1&burst=1&cubes=0&tag=baseline" baseline
for v in "$@"; do
  measure "$v"
  n=$((n + 1))
  [ $((n % 4)) -eq 0 ] && measure "bench=1&burst=1&cubes=0&tag=baseline" baseline
done
measure "bench=1&burst=1&cubes=0&tag=baseline" baseline
echo "END $(date +%T)"
