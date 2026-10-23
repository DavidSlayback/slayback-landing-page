"""Both trainers run end to end on a tiny budget, write the same metrics, and agree on what a policy computes."""

import json

import jax
import jax.numpy as jnp
import numpy as np
import torch

import ppo_jax
import ppo_torch
from policy import forward, read_policy, write_policy

FIELDS = {"update", "step", "sps", "episodes", "score", "escape_rate", "kills", "length", "lr", "policy_loss",
          "value_loss", "entropy", "approx_kl", "clipfrac", "grad_norm"}


def tiny(cfg):
    cfg.steps, cfg.envs, cfg.workers, cfg.rollout, cfg.epochs, cfg.minibatches, cfg.max_steps = 2 * 4 * 32, 4, 2, 32, 1, 2, 40
    return cfg


def check_run(out):
    lines = [json.loads(line) for line in (out / "metrics.jsonl").read_text().splitlines()]
    assert len(lines) == 2 and FIELDS <= set(lines[-1])
    assert lines[-1]["step"] == 2 * 4 * 32
    assert all(np.isfinite(lines[-1][k]) for k in ("policy_loss", "value_loss", "entropy", "grad_norm"))
    assert lines[-1]["episodes"] > 0, "40-step episodes end within the run"
    layers = read_policy(out / "policy.bin")
    assert [w.shape for w, _ in layers][0][1] > 0 and layers[-1][0].shape[0] == 45


def test_the_torch_trainer_runs_end_to_end_on_the_cpu(tmp_path):
    cfg = tiny(ppo_torch.Config(device="cpu"))
    ppo_torch.train(cfg, tmp_path)
    check_run(tmp_path)


def test_the_jax_trainer_runs_end_to_end(tmp_path):
    ppo_jax.train(tiny(ppo_jax.Config()), tmp_path)
    check_run(tmp_path)


def test_the_same_weights_give_the_same_logits_in_torch_jax_and_the_file(tmp_path):
    agent = ppo_torch.Agent(80, 45)
    obs = np.random.default_rng(0).standard_normal((5, 80)).astype(np.float32)
    want = agent.actor(torch.tensor(obs)).detach().numpy()
    layers = agent.actor_layers()
    got_jax = np.asarray(ppo_jax.apply_mlp([(jnp.asarray(w), jnp.asarray(b)) for w, b in layers], jnp.asarray(obs)))
    assert np.allclose(got_jax, want, atol=1e-5)
    write_policy(tmp_path / "p.bin", layers)
    assert np.allclose(forward(read_policy(tmp_path / "p.bin"), obs), want, atol=1e-5), "the file round-trips"
    assert jax.devices()[0].platform in ("cpu", "gpu")
