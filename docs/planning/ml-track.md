# ML track: the box as an RL environment, an agent benchmark, and a lab

**Goal:** turn *Escape the Box* into public evidence that can be checked: an RL agent trained on the game and playing it in the browser, a benchmark of LLM agents trying to escape, a page the site writes from its own test results, the game exposed to browser agents through WebMCP, and a measured training-step write-up.

David, 2026-10-19, after research into what gets ML engineers hired: "Ooh, good idea. I like 1, 2, 4 (but don't pay), 5 and 6". The research is summarized in the conversation record below; the career gap it targets (no recent public empirical ML work, no public non-proprietary agent artifact, no public GPU-performance result) is from david-brain's `docs/career/ml-ai-career-market-map-2026-08.md`.

## Status

```text
As of: Fri 2026-10-23. Bench v1 done, with the RL agent; branch rl-agent.
Done: (1) the progress score is the env's summed reward, the RL objective and the bench's secondary number; (2) bench v1 on the mini box with a 60 s play limit for all: RL policy 12/15 escapes, Opus 3/15, every other agent 0/15 (bench/README.md); (3) M2 #87 as box-vec (a binary protocol over stdio, not PyO3) + ml/boxenv.py, M3 #88 PPO in PyTorch (CUDA) and JAX (CPU); M4 #89's Rust forward pass (box_env::policy) for the bench; (4) torch-mini-1: 10M steps, 2.4 h, ml/models/torch-mini-1/.
Next: the policy in the browser (M4 #89's game side: ?agent= watch mode), the JAX run for the PyTorch-vs-JAX comparison, the lab page (M7 #92), the write-ups (M10 #95), profiling (M9 #94: training throughput fell from 1,640 to 1,170 steps/s as boxes filled).
Waiting on: WebMCP token (M8, later).
Re-arm: nothing scheduled.
Verified: bench/results/2026-10-23-*/results.json, every run verified by replay; ml tests 6 passed; box-env policy parity test.
```

M1 result: crates/box-env (reset/step, reward, Trace/replay bit for bit, JSON obs, scripted `brawler` baseline); game3d::headless shared with the scenarios (68 still pass); Waves.seed. Speed, one core, random actions in a light box: 2,092 steps/s release (140x real time), 1,818 dev; ~16 envs on this 14-core CPU should give ~25k steps/s, before the box fills with debris (unmeasured). The brawler baseline: ~10 kills, dies ~24 s into box 1, never escapes.

M5 result: crates/box-mcp, the MCP server (8 tools in tools.json, turn-based, the record replays); smoke run with headless Claude Code in bench/smoke/.

M6 so far: crates/box-bench runs `claude -p` per seed x trial through box-mcp, and replays every record before it counts.
- **Billing:** it strips ANTHROPIC_API_KEY and stops the sweep if a session reports an API-key source (David: "try to use the max if you can").
- **Isolation:** CLAUDE_CODE_DISABLE_CLAUDE_MDS keeps his CLAUDE.md out. Probed: --system-prompt alone doesn't, and --bare isn't logged in. It allows only the box tools, and checks that the server connected.
- **Output:** endings plus behaviour flags (looped, hugged walls, spread the pile, passive, fumbled) and a summary (escape rate, pass^k, pass-any), written to bench/results/<stamp>-<model>/.
- **First verified result (Haiku 5.5, seed 1, 40 calls):** out of calls after 16 s of play, 5 kills, no escape, best pile 39% of the rim, 41 calls, 39 s wall, ~395k input tokens.
  - It punched in every direction and called look before nearly every action, so the observation costs a call each time.
  - Worth considering: return the observation with each action, which already happens, and tell the agent that.
- **Second result (Sonnet 5.5, seed 1, 40 calls):** died after 17 s of play and 31 calls, 3 kills, no escape, best pile 16% of the rim. It fought in place (punches and kicks in all directions, one grab, never set out to heap a pile). One seed each is no comparison: a sweep is needed before any claim.
- **Not yet done:** the bench page on the lab (M7).

## Findings for the bench (M6), from the M5 smoke run

- **Billing.** David's Windows user environment has `ANTHROPIC_API_KEY` set. A `claude -p` launched from a fresh shell would see it, and headless Claude Code would then bill the API, not the subscription. The runner must remove it from every child's environment, and fail loudly if a run reports an API-key source.
- **Context contamination.** With no built-in tools, a run still starts with ~27k cached input tokens: Claude Code's system prompt, and likely David's user CLAUDE.md, which project-only setting sources don't drop. A benchmark must not be shaped by David's personal instructions.
  - Fix: `--system-prompt` with the bench's own prompt, plus a check of what loaded.
  - `--bare` would skip CLAUDE.md, but it also skips keychain reads, so it may need an API key: test it first.
- **Windows PowerShell 5.1 drops empty arguments.** `--tools ''` vanishes and swallows the next flag. Pass `--tools=`, and give `--allowedTools=` the full tool names (`mcp__box__look,...`).
- **Results are tied to the game's commit.** Any gameplay change can stop an old trace from replaying. Each result must name the commit it was played and replayed on.

## Settled (David, 2026-10-19)

| Decision | Choice |
|---|---|
| Ideas | 1 RL agent ("ghost"), 2 LLM agent benchmark, 4 training-step write-up (no paid compute), 5 lab page, 6 WebMCP. Not 3 (gradient-descent hero mode). |
| Where the code lives | This repo, made public (option "Make portfolio repo public"). No LICENSE yet: readable, not reusable; David's call later. |
| RL stack | "Jax and PyTorch both. I don't intend to train on the machine just yet." The env stays Rust with Python bindings; one PPO trainer in each framework, same hyperparameters and metrics, so they can be compared. |
| LLM spend | "Use my Claude subscription": agents run as headless Claude Code (`claude -p`), no API key, no extra spend. Results measure Claude Code as the agent, and runs draw on the plan's usage limits. |
| #4 target | "Both": nanoGPT-124M's training step and the PPO update step (in both frameworks). |
| Bench v1 (David, 2026-10-22) | Asked "What is the next benchmark design choice?", then: "Are we using an RL agent too? Probably should. Progress score is a good reward shaping option". Settled: the bench moves to a mini box (box 1's rim at 0.7 m, the tour's), where escape rate is the headline; one progress score (kills, survival, pile at a wall, escape, harm) is both every agent's secondary metric and the RL reward; a trained policy is a row in the table beside the LLMs and the baselines. |
| Training compute (David, 2026-10-22) | "Use the GPU if you can, it's the only thing that actually has memory to spare" (15.6 GB RAM, ~2 GB free; RTX 4060 8 GB, ~7.8 GB free). Training is unparked on this machine: PyTorch on CUDA for the policy; the game worlds step on the CPU (Bevy), so about 8 envs, sized by measured memory. JAX: CPU on Windows (CUDA JAX needs WSL). |

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
