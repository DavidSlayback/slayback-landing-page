"""Writes the policy fixture the Rust forward pass is checked against (crates/box-env/tests/fixtures): a random actor
in policy.bin, a few observations, and the logits `policy.forward` gives for them. `uv run python make_fixture.py`."""

import json
from pathlib import Path

import numpy as np
import torch

from policy import forward, write_policy
from ppo_torch import Agent

FLAT, ACTIONS = 175, 45  # game_core::agent::FLAT, ACTIONS


def main() -> None:
    out = Path(__file__).resolve().parent.parent / "crates" / "box-env" / "tests" / "fixtures"
    out.mkdir(parents=True, exist_ok=True)
    torch.manual_seed(7)
    agent = Agent(FLAT, ACTIONS)
    # Bigger last-layer weights than the trainer's init, so the logits differ enough to test.
    with torch.no_grad():
        agent.actor[-1].weight.mul_(100.0)
    layers = agent.actor_layers()
    write_policy(out / "policy.bin", layers)
    obs = np.random.default_rng(7).uniform(-1, 1, (3, FLAT)).astype(np.float32)
    logits = forward(layers, obs)
    (out / "policy.json").write_text(json.dumps({"obs": obs.tolist(), "logits": logits.tolist()}) + "\n")
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
