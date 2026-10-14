"""Generates the game's sound effects: short retro sounds synthesised from scratch, so we own them outright.

Run from the repo root: `python tools/sfx/generate.py`. It writes 8-bit mono 22.05 kHz WAVs to
`crates/game3d/assets/sfx/`, which the game embeds. Each effect is fully described by its entry in EFFECTS below and a
fixed noise seed, so regenerating gives byte-identical files. Standard library only.
"""

import math
import pathlib
import random
import wave

RATE = 22050
OUT = pathlib.Path(__file__).resolve().parents[2] / "crates" / "game3d" / "assets" / "sfx"

# name: (seconds, start Hz, end Hz, tone mix (0 = all noise, 1 = all tone), attack s, decay curve, wave, gain)
EFFECTS = {
    # A punch landing: a short, bright square-wave snap with a little grit.
    "hit": (0.09, 520.0, 180.0, 0.65, 0.002, 3.0, "square", 0.55),
    # A kick, drop-kick or throw: lower, longer, with more crunch.
    "heavy": (0.16, 300.0, 70.0, 0.5, 0.002, 2.5, "square", 0.7),
    # A figure breaking apart: a noisy clatter that falls in pitch.
    "shatter": (0.32, 900.0, 120.0, 0.2, 0.004, 2.0, "triangle", 0.6),
    # A bomb going off: a deep noise boom with a long tail.
    "boom": (0.7, 160.0, 30.0, 0.15, 0.005, 1.6, "sine", 0.9),
    # A door starting to warn: a short rising two-step beep. Appended last so the others keep their seeds.
    "door": (0.2, 330.0, 660.0, 0.85, 0.01, 1.2, "square", 0.4),
}


def osc(kind, phase):
    s = math.sin(phase)
    if kind == "square":
        return 1.0 if s >= 0 else -1.0
    if kind == "triangle":
        return 2.0 / math.pi * math.asin(s)
    return s


def synth(seconds, f0, f1, tone, attack, curve, kind, gain, seed):
    rng = random.Random(seed)
    n = int(seconds * RATE)
    phase, held, out = 0.0, 0.0, []
    for i in range(n):
        t = i / n
        freq = f0 * (f1 / f0) ** t  # exponential sweep
        phase += 2 * math.pi * freq / RATE
        # Sample-and-hold noise at the sweep frequency gives pitched, 8-bit-style grit.
        if i % max(1, int(RATE / (freq * 2))) == 0:
            held = rng.uniform(-1.0, 1.0)
        env = min(1.0, (i / RATE) / attack) * (1.0 - t) ** curve
        out.append(gain * env * (tone * osc(kind, phase) + (1.0 - tone) * held))
    return out


def write(name, samples):
    OUT.mkdir(parents=True, exist_ok=True)
    with wave.open(str(OUT / f"{name}.wav"), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(1)  # 8-bit unsigned
        w.setframerate(RATE)
        w.writeframes(bytes(max(0, min(255, int(128 + 127 * s))) for s in samples))


if __name__ == "__main__":
    for i, (name, params) in enumerate(EFFECTS.items()):
        write(name, synth(*params, seed=1000 + i))
        print(f"{name}.wav  {(OUT / f'{name}.wav').stat().st_size} bytes")
