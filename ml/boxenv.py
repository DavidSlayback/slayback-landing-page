"""Escape the Box as a vector environment for Python trainers.

`VecEnv` runs the Rust `box-vec` binary (crates/box-env/src/bin/box-vec.rs) and speaks its binary protocol over
stdin and stdout: `n` environments step together, each resetting itself with the next seed when its episode ends.
Observations are the game's flat encoding (`game_core::agent::Obs::flat`), actions its 45 discrete ones.
"""

from __future__ import annotations

import os
import subprocess
from dataclasses import dataclass
from pathlib import Path

import numpy as np

# The mini box's rim, metres (box_env::MINI_RIM).
MINI_RIM = 0.7


def default_binary() -> Path:
    """`box-vec` from the workspace's release build (BOX_VEC overrides)."""
    if "BOX_VEC" in os.environ:
        return Path(os.environ["BOX_VEC"])
    exe = "box-vec.exe" if os.name == "nt" else "box-vec"
    here = Path(__file__).resolve().parent
    for root in (here.parent, here.parents[3] if len(here.parents) > 3 else here.parent):
        candidate = root / "target" / "release" / exe
        if candidate.exists():
            return candidate
    target = os.environ.get("CARGO_TARGET_DIR")
    if target:
        return Path(target) / "release" / exe
    return here.parent / "target" / "release" / exe


@dataclass
class Step:
    obs: np.ndarray  # (n, flat) float32
    reward: np.ndarray  # (n,) float32
    done: np.ndarray  # (n,) bool: the episode ended (died or cut off); obs is the next episode's first
    truncated: np.ndarray  # (n,) bool: cut off at the step cap rather than ended
    score: np.ndarray  # (n,) float32: the finished episode's progress score, where done
    escapes: np.ndarray  # (n,) uint32
    kills: np.ndarray  # (n,) uint32
    length: np.ndarray  # (n,) uint32: steps


class _Worker:
    """One `box-vec` process and its share of the environments."""

    def __init__(self, args: list[str], n: int):
        self.n = n
        self.proc = subprocess.Popen(args, stdin=subprocess.PIPE, stdout=subprocess.PIPE, bufsize=0)
        header = np.frombuffer(self.read(12), dtype="<u4")
        assert int(header[0]) == n
        self.flat, self.actions = int(header[1]), int(header[2])
        self.first_obs = self.floats(n * self.flat).reshape(n, self.flat)

    def read(self, size: int) -> bytes:
        buf = bytearray()
        while len(buf) < size:
            chunk = self.proc.stdout.read(size - len(buf))
            if not chunk:
                raise RuntimeError(f"box-vec closed its output (exit {self.proc.poll()})")
            buf += chunk
        return bytes(buf)

    def floats(self, count: int) -> np.ndarray:
        return np.frombuffer(self.read(4 * count), dtype="<f4").copy()

    def u32(self, count: int) -> np.ndarray:
        return np.frombuffer(self.read(4 * count), dtype="<u4").copy()

    def send(self, actions: np.ndarray) -> None:
        self.proc.stdin.write(actions.astype(np.uint8).tobytes())
        self.proc.stdin.flush()

    def receive(self) -> Step:
        n = self.n
        obs = self.floats(n * self.flat).reshape(n, self.flat)
        reward = self.floats(n)
        done = np.frombuffer(self.read(n), dtype=np.uint8).astype(bool)
        truncated = np.frombuffer(self.read(n), dtype=np.uint8).astype(bool)
        score = self.floats(n)
        escapes, kills, length = self.u32(n), self.u32(n), self.u32(n)
        return Step(obs, reward, done, truncated, score, escapes, kills, length)

    def close(self) -> None:
        if self.proc.poll() is None:
            self.proc.stdin.close()
            self.proc.wait(timeout=10)


class VecEnv:
    """`n` environments of the game, spread over `workers` `box-vec` processes that step in parallel (each steps its
    share one after another; the game is single-threaded). Seeds: worker `k` starts at `seed + k * 1_000_000`."""

    def __init__(
        self,
        n: int = 8,
        seed: int = 0,
        frame_skip: int = 4,
        max_steps: int = 3000,
        rim: float | None = MINI_RIM,
        binary: Path | None = None,
        workers: int | None = None,
    ):
        workers = max(1, min(n, workers or n))
        shares = [n // workers + (1 if k < n % workers else 0) for k in range(workers)]
        self._workers = []
        for k, share in enumerate(shares):
            args = [str(binary or default_binary()), "--n", str(share), "--seed", str(seed + k * 1_000_000),
                    "--frame-skip", str(frame_skip), "--max-steps", str(max_steps)]
            if rim is not None:
                args += ["--rim", str(rim)]
            self._workers.append(_Worker(args, share))
        self.n = n
        self.flat, self.actions = self._workers[0].flat, self._workers[0].actions
        self.first_obs = np.concatenate([w.first_obs for w in self._workers])

    def step(self, actions: np.ndarray) -> Step:
        a = np.asarray(actions, dtype=np.uint8).reshape(self.n)
        at = 0
        for w in self._workers:
            w.send(a[at : at + w.n])
            at += w.n
        parts = [w.receive() for w in self._workers]
        return Step(*(np.concatenate([getattr(p, f) for p in parts]) for f in Step.__dataclass_fields__))

    def close(self) -> None:
        for w in self._workers:
            w.close()

    def pids(self) -> list[int]:
        return [w.proc.pid for w in self._workers]

    def __enter__(self) -> VecEnv:
        return self

    def __exit__(self, *_) -> None:
        self.close()
