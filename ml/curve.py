"""A run's learning curve at a glance: `uv run python curve.py runs/<name> [points]`. Prints evenly spaced updates
from metrics.jsonl: step, steps/s, the last 100 episodes' progress score, escape rate and kills, and the policy's
entropy."""

import json
import sys
from pathlib import Path


def main() -> None:
    run = Path(sys.argv[1])
    points = int(sys.argv[2]) if len(sys.argv) > 2 else 8
    rows = [json.loads(line) for line in (run / "metrics.jsonl").read_text().splitlines() if line.strip()]
    if not rows:
        print("no updates yet")
        return
    picks = sorted({round(i * (len(rows) - 1) / max(1, points - 1)) for i in range(points)})
    print(f"{'update':>6} {'step':>10} {'sps':>6} {'score':>7} {'escape':>7} {'kills':>6} {'entropy':>7}")
    for i in picks:
        r = rows[i]
        f = lambda k, fmt: format(r[k], fmt) if r.get(k) is not None else "-"  # noqa: E731
        print(f"{r['update']:>6} {r['step']:>10,} {r['sps']:>6.0f} {f('score', '7.2f'):>7} "
              f"{f('escape_rate', '7.0%'):>7} {f('kills', '6.1f'):>6} {f('entropy', '7.2f'):>7}")


if __name__ == "__main__":
    main()
