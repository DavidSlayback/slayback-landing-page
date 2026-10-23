"""The results table for a benchmark version: one row per sweep in bench/results with that `bench_version` (v0
sweeps have none) and 5 seeds x 3 trials. `python bench/table.py [version]` prints Markdown."""

import glob
import json
import os
import statistics as st
import sys

here = os.path.dirname(os.path.abspath(__file__))
version = int(sys.argv[1]) if len(sys.argv) > 1 else 1
rows = []
for f in sorted(glob.glob(os.path.join(here, "results", "*", "results.json"))):
    r = json.load(open(f, encoding="utf-8"))
    if r.get("bench_version", 0) != version or r.get("trials") != 3 or len(r.get("seeds", [])) != 5:
        continue
    a = [x["analysis"] for x in r["runs"] if x.get("analysis") and x["analysis"]["verified"]]
    if not a:
        continue
    native = not r.get("claude_version") or r["model"] in ("brawler", "random") or r["model"].startswith("policy:")
    s = r["summary"]
    escaped = sum(x["replayed"]["escapes"] > 0 for x in a)
    rows.append({
        "agent": r["model"].replace("policy:", "policy "),
        "verified": f"{len(a)}/{len(r['runs'])}",
        "escaped": f"{escaped}/{len(a)}",
        "pass_k": f"{s['pass_k']:.0%}",
        "score": st.mean(x.get("score", 0.0) for x in a),
        "kills": st.mean(x["replayed"]["kills"] for x in a),
        "secs": st.mean(x["game_seconds"] for x in a),
        "pile": st.mean(x["best_pile"] for x in a),
        "calls": None if native else st.mean(x["calls"] for x in a),
        "cost": sum(x.get("reported_cost_usd", 0) for x in r["runs"]),
        "dir": os.path.basename(os.path.dirname(f)),
    })
rows.sort(key=lambda x: (-int(x["escaped"].split("/")[0]), -x["score"]))
print("| Agent | Verified | Escaped | pass^3 | Progress score (mean) | Kills | Survived (s) | Best pile (share of rim) | Calls |")
print("|---|---|---|---|---|---|---|---|---|")
for x in rows:
    calls = "n/a" if x["calls"] is None else f"{x['calls']:.0f}"
    print(f"| {x['agent']} | {x['verified']} | {x['escaped']} | {x['pass_k']} | {x['score']:.1f} | {x['kills']:.1f} | "
          f"{x['secs']:.0f} | {x['pile']:.0%} | {calls} |")
print()
for x in rows:
    print(f"{x['agent']}: {x['dir']}, reported cost ${x['cost']:.2f}")
