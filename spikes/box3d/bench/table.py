"""Markdown tables from bench.cjs JSONL output: python -X utf8 bench/table.py results/*.jsonl"""

import json
import sys

for path in sys.argv[1:]:
    print("##", path)
    print("| variant | phase | interval avg / p95 / max (ms) | >16.7 ms frames | cpu avg / p95 | phys ms per step | awake avg | events |")
    print("|---|---|---|---|---|---|---|---|")
    for line in open(path, encoding="utf-8"):
        r = json.loads(line)
        if "variant" not in r:
            print(f"<!-- {r['renderer']} Chrome {r['version']} -->")
            continue
        ev = [x for x in r["reports"] if "event" in x]
        evs = "; ".join(
            f"{e['event']} {e.get('after_s', e.get('secs'))}s awake {e.get('awake', e.get('awake_max'))}/{e['total']}"
            for e in ev
        )
        for x in r["reports"]:
            if "phase" not in x:
                continue
            i, c, p = x["interval_ms"], x["cpu_ms"], x["phys_ms_per_frame"]
            per_step = p["avg"] * x["frames"] / max(x["steps"], 1)
            print(
                f"| {r['variant']} | {x['phase']} | {i['avg']:.1f} / {i['p95']:.1f} / {i['max']:.1f} | {i['over_16_7']} of {x['frames']} "
                f"| {c['avg']:.1f} / {c['p95']:.1f} | {per_step:.2f} | {x['awake_avg']:.0f} | {evs} |"
            )
