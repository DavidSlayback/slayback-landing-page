# ML track: the box as an RL environment, an agent benchmark, and a lab

**Goal:** turn *Escape the Box* into public evidence that can be checked: an RL agent trained on the game and playing it in the browser, a benchmark of LLM agents trying to escape, a page the site writes from its own test results, the game exposed to browser agents through WebMCP, and a measured training-step write-up.

David, 2026-10-19, after research into what gets ML engineers hired: "Ooh, good idea. I like 1, 2, 4 (but don't pay), 5 and 6". The research is summarized in the conversation record below; the career gap it targets (no recent public empirical ML work, no public non-proprietary agent artifact, no public GPU-performance result) is from david-brain's `docs/career/ml-ai-career-market-map-2026-08.md`.

## Status

```text
As of: 2026-10-19. State: planned; spec #85 and tickets #86-#95 filed. Repo made public 2026-10-19 (approved; history scanned first: 1,626 blobs, 0 secret hits, commit emails are GitHub noreply, no third-party media).
Next: M1 (headless env core). M7 (lab page) can start in parallel.
Parked: every real training or profiling run (David: "I don't intend to train on the machine just yet"). Resume when David says where compute comes from. Code, tests and seconds-long CPU smoke runs proceed.
Needs David later: a WebMCP origin-trial token (M8); the go-ahead to run the benchmark on his subscription past the smoke cap (M6).
```

## Settled (David, 2026-10-19)

| Decision | Choice |
|---|---|
| Ideas | 1 RL agent ("ghost"), 2 LLM agent benchmark, 4 training-step write-up (no paid compute), 5 lab page, 6 WebMCP. Not 3 (gradient-descent hero mode). |
| Where the code lives | This repo, made public (option "Make portfolio repo public"). No LICENSE yet: readable, not reusable; David's call later. |
| RL stack | "Jax and PyTorch both. I don't intend to train on the machine just yet." The env stays Rust with Python bindings; one PPO trainer in each framework, same hyperparameters and metrics, so they can be compared. |
| LLM spend | "Use my Claude subscription": agents run as headless Claude Code (`claude -p`), no API key, no extra spend. Results measure Claude Code as the agent, and runs draw on the plan's usage limits. |
| #4 target | "Both": nanoGPT-124M's training step and the PPO update step (in both frameworks). |

## Defaults (recorded, not asked)

- **Publication:** everything is built and shown on rc-5. Lab, agent and bench pages go public with the game's release (#29's playtest gate), since they show the game.
- **Human leaderboard:** deferred. v1 publishes RL and LLM results as static files; a human track needs a free Cloudflare Worker to accept and re-simulate submissions.
- **Agent in the browser:** v1 is a watch mode (the policy plays its own run, `?agent=<checkpoint>`), not a second fighter beside the player.
- **No client JS for the lab page:** it is generated as static HTML with inline SVG charts at build time.

## Architecture

```text
game3d (Bevy, headless App)                       <- one builder, shared with tests/scenarios.rs
  └─ box-env (Rust): Env { reset(seed), step(Action) -> obs, reward, done }, Trace, replay()
       ├─ box-env-py (PyO3/maturin): VecEnv(n) stepping n envs in Rust, numpy out, gymnasium wrapper
       │    └─ ml/ (uv): ppo_torch.py, ppo_jax.py -> runs/*/metrics.jsonl, export.py -> policy.bin
       │         └─ crates/policy: tiny MLP forward pass (no deps) -> game3d `?agent=` watch mode (wasm)
       ├─ box-mcp (stdio MCP server): look / move / punch / kick / dash / grab / wait, turn-based
       │    └─ bench/: drives `claude -p` over seeds x k trials, keeps traces, re-simulates them
       └─ WebMCP on /play: the same tool schema, registered in the page, driving the wasm game
lab generator (build time): tests, scenarios, sim bench, wasm sizes, commit -> dist/lab/ (+ RL curves, bench table)
```

**Seams reused:**
- **`input::Actions`** (`crates/game3d/src/input.rs`): direction plus verbs, the single way into the game. Demo mode already plays through it. The env writes it each step.
- **The scenarios' headless App** (`crates/game3d/tests/scenarios.rs`: `MinimalPlugins`, `TransformPlugin`, `game3d::plugin`, `TimeUpdateStrategy::ManualDuration`).
  - It moves into `game3d` as a public builder, so tests and the env build the same game.
- **Determinism:**
  - Enemy spawning seeds itself per App (`enemy.rs`, a `Local<u32>`). It becomes a seed resource.
  - The only wall-clock read is `score.rs`'s save date, which isn't game state.

**Action spaces:**
- **RL:** discrete, 9 directions × {none, dash, punch, kick, grab}, 45 actions, repeated for `frame_skip` (default 4) frames.
- **LLM agents:** macro actions that each advance the game a bounded time, with the game frozen between calls (turn-based).
  - A model can't act at 60 Hz. The macro layer is part of the benchmark's definition and is documented with it.

**Observation:** one struct with two encodings, `flat() -> Vec<f32>` for RL and `json()` for LLMs. It holds:
- the player's position, velocity, facing, health, carry and stun;
- the nearest K enemies, relative position, kind, health and state;
- a coarse heightmap of the pile over the floor;
- the rim height, box number, timer and door warnings.

**Reward (RL):**
- **Gains:** a kill bonus, rise in the pile under the player toward the rim, and a large escape bonus.
- **Penalties:** damage taken, and a small cost per step.
- The write-up must cover reward hacking (for example, farming kills without building a climbable pile).

**Benchmark metrics:**
- escape rate;
- pass^k (all k trials of a seed escape);
- boxes reached, turns and wall time;
- the cost `claude -p` reports (an estimate under a subscription);
- a rule-based failure taxonomy over traces: died, stalled at a wall, spread the pile instead of stacking it, looped, ran out of turns.
- Every result ships with its trace, and a result counts only if replaying the trace reproduces it.

## Risks

- **Cross-platform determinism.** A run recorded in the browser (wasm) may not replay identically natively; float results can differ. v1 keeps all verified traces native (RL and bench both run natively). Before any human track: test wasm-to-native replay, or turn on avian's `enhanced-determinism`.
- **Env speed decides everything about RL.** M1 measures steps/s per core first. If a full Bevy App is too slow, strip render-only systems from the headless build, or train on a game-core-only sim.
- **JAX on Windows:** the CPU wheels should cover tests; CUDA JAX needs WSL. That doesn't matter until runs are unparked.
- **Subscription usage:** each benchmark run is many turns. M6 has a hard cap (smoke: 1 seed × 1 trial, ≤ 40 turns) until David approves a full sweep.
- **WebMCP is an origin trial** (Chrome M149–M156, approved on blink-dev 2026-05-18). The entry point is reportedly moving from `navigator.modelContext` to `document.modelContext`, per secondary sources: feature-detect both. Few agents consume it yet.

## Tickets

Spec [#85](https://github.com/DavidSlayback/slayback-landing-page/issues/85), with tickets labelled `track:ml`:

| Ticket | Issue | Blocked by |
|---|---|---|
| M1 headless env core | #86 | — |
| M2 Python bindings, vector env | #87 | #86 |
| M3 PPO in PyTorch and JAX | #88 | #87 |
| M4 policy in the browser (`?agent=`) | #89 | #88 |
| M5 box-mcp server | #90 | #86 |
| M6 bench runner (`claude -p`) | #91 | #90 |
| M7 lab page | #92 | — |
| M8 WebMCP on /play | #93 | #90 |
| M9 profiling harness | #94 | #88 (PPO part) |
| M10 write-ups | #95 | #88, #91, #94, unparked runs |

Start with M1 and M7, which can run in parallel.
