"""PPO for Escape the Box in JAX: `uv run python ppo_jax.py --steps 2000000`.

The same algorithm, network, hyperparameters and metrics as `ppo_torch.py`, written in JAX with optax: a jitted
minibatch update over parameters kept as `(weight[out, in], bias[out])` layers, the layout `policy.py` writes, so the
two trainers' policies are interchangeable. On Windows it runs on the CPU (CUDA JAX needs Linux or WSL).
"""

from __future__ import annotations

import argparse
import json
import time
from dataclasses import asdict, dataclass
from functools import partial
from pathlib import Path

import jax
import jax.numpy as jnp
import numpy as np
import optax

from boxenv import MINI_RIM, VecEnv
from policy import HIDDEN, write_policy


@dataclass
class Config:
    name: str = "jax"
    steps: int = 2_000_000
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


def init_mlp(key, inp: int, out: int, gain: float):
    sizes = [(inp, HIDDEN, 2**0.5), (HIDDEN, HIDDEN, 2**0.5), (HIDDEN, out, gain)]
    layers = []
    for (i, o, g), k in zip(sizes, jax.random.split(key, len(sizes))):
        w = jax.nn.initializers.orthogonal(g)(k, (i, o), jnp.float32).T  # (out, in), as torch keeps it
        layers.append((w, jnp.zeros(o, jnp.float32)))
    return layers


def apply_mlp(layers, x):
    for i, (w, b) in enumerate(layers):
        x = x @ w.T + b
        if i < len(layers) - 1:
            x = jnp.tanh(x)
    return x


def logits_value(params, x):
    return apply_mlp(params["actor"], x), apply_mlp(params["critic"], x)[..., 0]


@jax.jit
def act(params, x, key):
    logits, value = logits_value(params, x)
    action = jax.random.categorical(key, logits)
    logp = jax.nn.log_softmax(logits)[jnp.arange(x.shape[0]), action]
    return action, logp, value


@partial(jax.jit, static_argnames=("cfg_clip", "cfg_entropy", "cfg_value", "opt"))
def update_minibatch(params, opt_state, b, cfg_clip, cfg_entropy, cfg_value, opt):
    def loss_fn(p):
        logits, v = logits_value(p, b["obs"])
        logp_all = jax.nn.log_softmax(logits)
        lp = logp_all[jnp.arange(b["obs"].shape[0]), b["act"]]
        ent = -(jnp.exp(logp_all) * logp_all).sum(-1)
        ratio = jnp.exp(lp - b["logp"])
        a = (b["adv"] - b["adv"].mean()) / (b["adv"].std() + 1e-8)
        pg = jnp.maximum(-a * ratio, -a * jnp.clip(ratio, 1 - cfg_clip, 1 + cfg_clip)).mean()
        vl = 0.5 * ((v - b["ret"]) ** 2).mean()
        loss = pg - cfg_entropy * ent.mean() + cfg_value * vl
        kl = ((ratio - 1) - (lp - b["logp"])).mean()
        clipfrac = (jnp.abs(ratio - 1) > cfg_clip).mean()
        return loss, (pg, vl, ent.mean(), kl, clipfrac)

    (_, aux), grads = jax.value_and_grad(loss_fn, has_aux=True)(params)
    gn = optax.tree.norm(grads)
    updates, opt_state = opt.update(grads, opt_state, params)
    return optax.apply_updates(params, updates), opt_state, (*aux, gn)


def gae(rewards, values, dones, next_value, next_done, gamma, lam):
    t = rewards.shape[0]
    adv = np.zeros_like(rewards)
    last = np.zeros_like(next_value)
    for i in reversed(range(t)):
        nonterminal = 1.0 - (next_done if i == t - 1 else dones[i + 1])
        nv = next_value if i == t - 1 else values[i + 1]
        delta = rewards[i] + gamma * nv * nonterminal - values[i]
        last = delta + gamma * lam * nonterminal * last
        adv[i] = last
    return adv, adv + values


def train(cfg: Config, out: Path) -> dict:
    out.mkdir(parents=True, exist_ok=True)
    (out / "config.json").write_text(json.dumps({**asdict(cfg), "framework": "jax"}, indent=2) + "\n")
    key = jax.random.PRNGKey(cfg.seed)
    env = VecEnv(n=cfg.envs, seed=cfg.seed * 10_000, rim=cfg.rim, max_steps=cfg.max_steps, workers=cfg.workers)
    ka, kc, key = jax.random.split(key, 3)
    params = {"actor": init_mlp(ka, env.flat, env.actions, 0.01), "critic": init_mlp(kc, env.flat, 1, 1.0)}
    n, t = cfg.envs, cfg.rollout
    updates = max(1, cfg.steps // (n * t))
    schedule = optax.linear_schedule(cfg.lr, 0.0, updates * cfg.epochs * cfg.minibatches)
    opt = optax.chain(optax.clip_by_global_norm(cfg.max_grad), optax.adam(schedule, eps=1e-5))
    opt_state = opt.init(params)
    batch, mini = n * t, (n * t) // cfg.minibatches
    obs = np.zeros((t, n, env.flat), np.float32)
    actions = np.zeros((t, n), np.int32)
    logps, rewards, dones, values = (np.zeros((t, n), np.float32) for _ in range(4))
    next_obs, next_done = env.first_obs.astype(np.float32), np.zeros(n, np.float32)
    metrics = open(out / "metrics.jsonl", "w")
    finished: list[tuple[float, int, int, int]] = []
    rng = np.random.default_rng(cfg.seed)
    start, step, last = time.perf_counter(), 0, {}
    for update in range(1, updates + 1):
        for i in range(t):
            step += n
            obs[i], dones[i] = next_obs, next_done
            key, k = jax.random.split(key)
            a, lp, v = act(params, next_obs, k)
            actions[i], logps[i], values[i] = np.asarray(a), np.asarray(lp), np.asarray(v)
            s = env.step(actions[i])
            rewards[i], next_obs, next_done = s.reward, s.obs.astype(np.float32), s.done.astype(np.float32)
            for j in np.flatnonzero(s.done):
                finished.append((float(s.score[j]), int(s.escapes[j]), int(s.kills[j]), int(s.length[j])))
        next_value = np.asarray(logits_value(params, next_obs)[1])
        adv, ret = gae(rewards, values, dones, next_value, next_done, cfg.gamma, cfg.lam)
        flat = {"obs": obs.reshape(batch, env.flat), "act": actions.reshape(batch), "logp": logps.reshape(batch),
                "adv": adv.reshape(batch), "ret": ret.reshape(batch)}
        stats = []
        for _ in range(cfg.epochs):
            perm = rng.permutation(batch)
            for s0 in range(0, batch, mini):
                idx = perm[s0 : s0 + mini]
                b = {k: jnp.asarray(v[idx]) for k, v in flat.items()}
                params, opt_state, aux = update_minibatch(params, opt_state, b, cfg.clip, cfg.entropy, cfg.value, opt)
                stats.append([float(x) for x in aux])
        st = np.mean(stats, axis=0)
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
            "lr": float(schedule(update * cfg.epochs * cfg.minibatches)),
            "policy_loss": float(st[0]), "value_loss": float(st[1]), "entropy": float(st[2]),
            "approx_kl": float(st[3]), "clipfrac": float(st[4]), "grad_norm": float(st[5]),
        }
        metrics.write(json.dumps(last) + "\n")
        metrics.flush()
        if update % cfg.save_every == 0 or update == updates:
            write_policy(out / "policy.bin", [(np.asarray(w), np.asarray(b)) for w, b in params["actor"]])
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
    print(json.dumps(train(cfg, out or Path("runs") / cfg.name)))


if __name__ == "__main__":
    main()
