"""Synthesises the game's two effects that aren't recordings (decision Q79): the whoosh of something flying (a burst's
pieces, a throw) and a bomb's blast. The wooden knocks are recordings (`tools/sfx/import.sh`).

Run from the repo root: `python tools/sfx/generate.py`. It writes 16-bit mono 44.1 kHz WAVs to
`crates/game3d/assets/sfx/`, which the game embeds. A fixed noise seed per effect makes regenerating byte-identical.
Standard library only.
"""

import math
import pathlib
import random
import struct
import wave

RATE = 44100
OUT = pathlib.Path(__file__).resolve().parents[2] / "crates" / "game3d" / "assets" / "sfx"


def bandpass(samples, centre, q):
    """A resonant band-pass (RBJ biquad) whose centre frequency follows `centre(t)`, t from 0 to 1."""
    out, x1, x2, y1, y2 = [], 0.0, 0.0, 0.0, 0.0
    n = len(samples)
    for i, x in enumerate(samples):
        w = 2 * math.pi * centre(i / n) / RATE
        alpha = math.sin(w) / (2 * q)
        b0, b2, a0, a1, a2 = alpha, -alpha, 1 + alpha, -2 * math.cos(w), 1 - alpha
        y = (b0 * x + b2 * x2 - a1 * y1 - a2 * y2) / a0
        out.append(y)
        x2, x1, y2, y1 = x1, x, y1, y
    return out


def lowpass(samples, cutoff):
    """A one-pole low-pass whose cutoff follows `cutoff(t)`."""
    out, y = [], 0.0
    n = len(samples)
    for i, x in enumerate(samples):
        k = 1 - math.exp(-2 * math.pi * cutoff(i / n) / RATE)
        y += k * (x - y)
        out.append(y)
    return out


def normalise(samples, peak):
    top = max(abs(s) for s in samples) or 1.0
    return [s * peak / top for s in samples]


def whoosh(seed):
    """Air rushing past: noise through a band-pass that sweeps up and back down, swelling and fading, 0.32 s."""
    rng = random.Random(seed)
    n = int(0.32 * RATE)
    noise = [rng.uniform(-1, 1) for _ in range(n)]
    swept = bandpass(noise, lambda t: 500 + 1700 * math.sin(math.pi * t) ** 2, 1.4)
    env = [math.sin(math.pi * min(1.0, (i / n) / 0.35)) ** 2 if i / n < 0.35 else (1 - (i / n - 0.35) / 0.65) ** 2 for i in range(n)]
    return normalise([s * e for s, e in zip(swept, env)], 0.7)


def boom(seed):
    """A blast: a deep thump (a sine falling from 90 to 35 Hz) under a burst of rumbling noise that darkens as it
    fades, 1.1 s."""
    rng = random.Random(seed)
    n = int(1.1 * RATE)
    phase, thump = 0.0, []
    for i in range(n):
        t = i / RATE
        phase += 2 * math.pi * (35 + 55 * math.exp(-t * 9)) / RATE
        thump.append(math.sin(phase) * math.exp(-t * 4.5))
    noise = [rng.uniform(-1, 1) for _ in range(n)]
    rumble = lowpass(noise, lambda t: 2400 * math.exp(-t * 5) + 120)
    rumble = [r * min(1.0, (i / RATE) / 0.004) * math.exp(-(i / RATE) * 3.2) for i, r in enumerate(rumble)]
    rumble = normalise(rumble, 1.0)
    return normalise([0.75 * a + 0.55 * b for a, b in zip(thump, rumble)], 0.9)


EFFECTS = {"whoosh": whoosh, "boom": boom}


def write(name, samples):
    OUT.mkdir(parents=True, exist_ok=True)
    with wave.open(str(OUT / f"{name}.wav"), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)  # 16-bit signed
        w.setframerate(RATE)
        w.writeframes(b"".join(struct.pack("<h", max(-32767, min(32767, int(32767 * s)))) for s in samples))


if __name__ == "__main__":
    for i, (name, make) in enumerate(EFFECTS.items()):
        write(name, make(seed=2000 + i))
        print(f"{name}.wav  {(OUT / f'{name}.wav').stat().st_size} bytes")
