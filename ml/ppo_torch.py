"""PPO for Escape the Box in PyTorch (single file, CleanRL's shape): `uv run python ppo_torch.py --steps 5000000`.

Discrete PPO with GAE, a clipped objective, an entropy bonus and an annealed learning rate, over `boxenv.VecEnv`
(the game in `box-vec` processes, the mini box by default). The networks run on CUDA when it's there. Every update
appends a line of metrics to `runs/<name>/metrics.jsonl` (the same fields as `ppo_jax.py`), and every `--save-every`
updates, and at the end, the actor goes to `runs/<name>/policy.bin` (`policy.py`) and both networks to `model.pt`.
The reward is the environment's: an episode's summed reward is its progress score.
"""

from __future__ import annotations

import argparse
import json
import time
from dataclasses import asdict, dataclass
from pathlib import Path

import numpy as np
import torch
from torch import nn

from boxenv import MINI_RIM, VecEnv
from policy import HIDDEN, write_policy


@dataclass
class Config:
    name: str = "torch"
    steps: int = 5_000_000
    envs: int = 16
    workers: int = 16
    rollout: int = 128
    epochs: int = 4
    minibatches: int = 4
    lr: float = 2.5e-4
    gamma: float = 0.99
    lam: float = 0.95
    clip: float = 0.2
    entropy: float = 0.01
    value: float = 0.5
    max_grad: float = 0.5
    seed: int = 1
    rim: float = MINI_RIM
    max_steps: int = 3000
    save_every: int = 50
    device: str = "cuda" if torch.cuda.is_available() else "cpu"


def mlp(inp: int, out: int, gain: float) -> nn.Sequential:
    def layer(i: int, o: int, g: float) -> nn.Linear:
        lin = nn.Linear(i, o)
        nn.init.orthogonal_(lin.weight, g)
        nn.init.zeros_(lin.bias)
        return lin

    return nn.Sequential(layer(inp, HIDDEN, 2**0.5), nn.Tanh(), layer(HIDDEN, HIDDEN, 2**0.5), nn.Tanh(),
                         layer(HIDDEN, out, gain))


class Agent(nn.Module):
    def __init__(self, flat: int, actions: int):
        super().__init__()
        self.actor = mlp(flat, actions, 0.01)
        self.critic = mlp(flat, 1, 1.0)

    def value(self, x: torch.Tensor) -> torch.Tensor:
        return self.critic(x).squeeze(-1)

    def act(self, x: torch.Tensor, action: torch.Tensor | None = None):
        dist = torch.distributions.Categorical(logits=self.actor(x))
        if action is None:
            action = dist.sample()
        return action, dist.log_prob(action), dist.entropy(), self.value(x)

    def actor_layers(self) -> list[tuple[np.ndarray, np.ndarray]]:
        return [(m.weight.detach().cpu().numpy(), m.bias.detach().cpu().numpy())
                for m in self.actor if isinstance(m, nn.Linear)]


def train(cfg: Config, out: Path) -> dict:
    out.mkdir(parents=True, exist_ok=True)
    (out / "config.json").write_text(json.dumps({**asdict(cfg), "framework": "torch"}, indent=2) + "\n")
    torch.manual_seed(cfg.seed)
    np.random.seed(cfg.seed)
    dev = torch.device(cfg.device)
    env = VecEnv(n=cfg.envs, seed=cfg.seed * 10_000, rim=cfg.rim, max_steps=cfg.max_steps, workers=cfg.workers)
    agent = Agent(env.flat, env.actions).to(dev)
    opt = torch.optim.Adam(agent.parameters(), lr=cfg.lr, eps=1e-5)

    n, t = cfg.envs, cfg.rollout
    obs = torch.zeros((t, n, env.flat), device=dev)
    actions = torch.zeros((t, n), device=dev, dtype=torch.long)
    logps, rewards, dones, values = (torch.zeros((t, n), device=dev) for _ in range(4))
    next_obs = torch.tensor(env.first_obs, device=dev)
    next_done = torch.zeros(n, device=dev)
    updates = max(1, cfg.steps // (n * t))
    batch, mini = n * t, (n * t) // cfg.minibatches
    metrics = open(out / "metrics.jsonl", "w")
    finished: list[tuple[float, int, int, int]] = []
    start, step = time.perf_counter(), 0
    last = {}
    for update in range(1, updates + 1):
        for g in opt.param_groups:
            g["lr"] = cfg.lr * (1.0 - (update - 1) / updates)
        for i in range(t):
            step += n
            obs[i], dones[i] = next_obs, next_done
            with torch.no_grad():
                a, lp, _, v = agent.act(next_obs)
            actions[i], logps[i], values[i] = a, lp, v
            s = env.step(a.cpu().numpy())
            rewards[i] = torch.tensor(s.reward, device=dev)
            next_obs = torch.tensor(s.obs, device=dev)
            next_done = torch.tensor(s.done, device=dev, dtype=torch.float32)
            for j in np.flatnonzero(s.done):
                finished.append((float(s.score[j]), int(s.escapes[j]), int(s.kills[j]), int(s.length[j])))
        # GAE. A done marks the step that ended an episode; the obs after it is the next episode's first.
        with torch.no_grad():
            next_value = agent.value(next_obs)
            adv = torch.zeros_like(rewards)
            gae = torch.zeros(n, device=dev)
            for i in reversed(range(t)):
                nonterminal = 1.0 - (next_done if i == t - 1 else dones[i + 1])
                nv = next_value if i == t - 1 else values[i + 1]
                delta = rewards[i] + cfg.gamma * nv * nonterminal - values[i]
                gae = delta + cfg.gamma * cfg.lam * nonterminal * gae
                adv[i] = gae
            returns = adv + values
        b_obs, b_act = obs.reshape(batch, env.flat), actions.reshape(batch)
        b_lp, b_adv, b_ret, b_val = logps.reshape(batch), adv.reshape(batch), returns.reshape(batch), values.reshape(batch)
        stats = {"policy_loss": [], "value_loss": [], "entropy": [], "approx_kl": [], "clipfrac": [], "grad_norm": []}
        for _ in range(cfg.epochs):
            for idx in torch.randperm(batch, device=dev).split(mini):
                _, lp, ent, v = agent.act(b_obs[idx], b_act[idx])
                ratio = (lp - b_lp[idx]).exp()
                a_ = b_adv[idx]
                a_ = (a_ - a_.mean()) / (a_.std() + 1e-8)
                pg = torch.max(-a_ * ratio, -a_ * ratio.clamp(1 - cfg.clip, 1 + cfg.clip)).mean()
                vl = 0.5 * ((v - b_ret[idx]) ** 2).mean()
                loss = pg - cfg.entropy * ent.mean() + cfg.value * vl
                opt.zero_grad()
                loss.backward()
                gn = nn.utils.clip_grad_norm_(agent.parameters(), cfg.max_grad)
                opt.step()
                with torch.no_grad():
                    stats["policy_loss"].append(pg.item())
                    stats["value_loss"].append(vl.item())
                    stats["entropy"].append(ent.mean().item())
                    stats["approx_kl"].append(((ratio - 1) - (lp - b_lp[idx])).mean().item())
                    stats["clipfrac"].append(((ratio - 1).abs() > cfg.clip).float().mean().item())
                    stats["grad_norm"].append(float(gn))
        recent = finished[-100:]
        last = {
            "update": update,
            "step": step,
            "sps": step / (time.perf_counter() - start),
            "episodes": len(finished),
            "score": float(np.mean([e[0] for e in recent])) if recent else None,
            "escape_rate": float(np.mean([e[1] > 0 for e in recent])) if recent else None,
            "kills": float(np.mean([e[2] for e in recent])) if recent else None,
            "length": float(np.mean([e[3] for e in recent])) if recent else None,
            "lr": opt.param_groups[0]["lr"],
            **{k: float(np.mean(v)) for k, v in stats.items()},
        }
        metrics.write(json.dumps(last) + "\n")
        metrics.flush()
        if update % cfg.save_every == 0 or update == updates:
            write_policy(out / "policy.bin", agent.actor_layers())
            torch.save(agent.state_dict(), out / "model.pt")
    metrics.close()
    env.close()
    return last


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    for f, v in asdict(Config()).items():
        p.add_argument(f"--{f.replace('_', '-')}", type=type(v), default=v)
    p.add_argument("--out", type=Path, default=None)
    a = vars(p.parse_args())
    out = a.pop("out")
    cfg = Config(**a)
    final = train(cfg, out or Path("runs") / cfg.name)
    print(json.dumps(final))


if __name__ == "__main__":
    main()
