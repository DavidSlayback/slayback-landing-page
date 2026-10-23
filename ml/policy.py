"""The policy's shape and its file, shared by both trainers and the game.

The actor and the critic are separate MLPs over the flat observation: two hidden layers of `HIDDEN` with tanh, then 45
action logits (the actor) or one value (the critic). `policy.bin` holds the actor alone, for the game's Rust forward
pass (M4): little-endian, `b"BOXP"`, version 1 (u32), the layer count (u32), then per layer its input and output
sizes (u32 each), its weights `out x in` row-major and its biases (f32). Tanh between layers, none after the last.
"""

from __future__ import annotations

import struct
from pathlib import Path

import numpy as np

HIDDEN = 256
MAGIC = b"BOXP"


def write_policy(path: Path, layers: list[tuple[np.ndarray, np.ndarray]]) -> None:
    """Writes the actor's `(weight[out, in], bias[out])` layers."""
    with open(path, "wb") as f:
        f.write(MAGIC + struct.pack("<II", 1, len(layers)))
        for w, b in layers:
            w = np.asarray(w, dtype="<f4")
            b = np.asarray(b, dtype="<f4")
            out, inp = w.shape
            assert b.shape == (out,)
            f.write(struct.pack("<II", inp, out))
            f.write(w.tobytes(order="C"))
            f.write(b.tobytes())


def read_policy(path: Path) -> list[tuple[np.ndarray, np.ndarray]]:
    data = Path(path).read_bytes()
    assert data[:4] == MAGIC, "not a policy file"
    version, count = struct.unpack_from("<II", data, 4)
    assert version == 1
    at, layers = 12, []
    for _ in range(count):
        inp, out = struct.unpack_from("<II", data, at)
        at += 8
        w = np.frombuffer(data, dtype="<f4", count=out * inp, offset=at).reshape(out, inp)
        at += 4 * out * inp
        b = np.frombuffer(data, dtype="<f4", count=out, offset=at)
        at += 4 * out
        layers.append((w.copy(), b.copy()))
    return layers


def forward(layers: list[tuple[np.ndarray, np.ndarray]], obs: np.ndarray) -> np.ndarray:
    """The actor's logits in numpy: the reference both trainers and the game's forward pass must match."""
    x = np.asarray(obs, dtype=np.float32)
    for i, (w, b) in enumerate(layers):
        x = x @ w.T + b
        if i < len(layers) - 1:
            x = np.tanh(x)
    return x
