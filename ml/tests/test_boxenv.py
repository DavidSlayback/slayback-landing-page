"""The vector environment over box-vec: shapes, determinism, and episodes that end and restart."""

import numpy as np

from boxenv import VecEnv


def rollout(seed: int, steps: int, n: int = 4) -> list:
    rng = np.random.default_rng(seed)
    out = []
    with VecEnv(n=n, seed=seed, max_steps=200) as env:
        assert env.first_obs.shape == (n, env.flat) and env.actions == 45
        for _ in range(steps):
            out.append(env.step(rng.integers(0, env.actions, size=n)))
    return out


def test_shapes_and_finite_values():
    steps = rollout(1, 20)
    s = steps[-1]
    assert s.obs.shape[0] == 4 and s.obs.dtype == np.float32
    assert all(np.isfinite(x.obs).all() and np.isfinite(x.reward).all() for x in steps)


def test_the_same_seed_and_actions_give_the_same_stream():
    a, b = rollout(3, 50), rollout(3, 50)
    assert all(np.array_equal(x.obs, y.obs) and np.array_equal(x.reward, y.reward) for x, y in zip(a, b))


def test_episodes_cut_off_at_the_step_cap_report_and_restart():
    steps = rollout(2, 230)
    ended = [s for s in steps if s.done.any()]
    assert ended, "every env reaches its 200-step cap within 230 steps unless it died first"
    s = ended[0]
    i = int(np.argmax(s.done))
    assert s.length[i] > 0 and np.isfinite(s.score[i])
    assert all(s.length[j] == 0 for j in range(len(s.done)) if not s.done[j])
