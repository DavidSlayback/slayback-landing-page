"""How fast `VecEnv` steps from Python, and how much memory its `box-vec` processes hold: `uv run python bench_env.py
[n] [steps] [workers]`. Random actions in the mini box."""

import os
import subprocess
import sys
import time

import numpy as np

from boxenv import VecEnv


def rss_mb(pid: int) -> float:
    if os.name != "nt":
        return float(open(f"/proc/{pid}/status").read().split("VmRSS:")[1].split()[0]) / 1024
    out = subprocess.run(["tasklist", "/fi", f"PID eq {pid}", "/fo", "csv", "/nh"], capture_output=True, text=True).stdout
    return float(out.strip().split('","')[-1].strip('"').replace(" K", "").replace(",", "")) / 1024


def main() -> None:
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 8
    steps = int(sys.argv[2]) if len(sys.argv) > 2 else 1000
    workers = int(sys.argv[3]) if len(sys.argv) > 3 else n
    rng = np.random.default_rng(0)
    with VecEnv(n=n, seed=0, workers=workers) as env:
        start = time.perf_counter()
        episodes = 0
        for _ in range(steps):
            s = env.step(rng.integers(0, env.actions, size=n))
            episodes += int(s.done.sum())
        secs = time.perf_counter() - start
        mb = sum(rss_mb(pid) for pid in env.pids())
        print(f"{n} envs on {workers} workers x {steps} steps: {n * steps / secs:,.0f} env steps/s, "
              f"{episodes} episodes ended, box-vec holds {mb:,.0f} MB")


if __name__ == "__main__":
    main()
