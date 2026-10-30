# Escape the Box: an agent benchmark

LLM agents play *Escape the Box*, the 3D brawler on this site, through an MCP server, turn by turn. This page covers how runs are made and checked, and the results so far. The plan is `docs/planning/ml-track.md` (M6, issue #91).

## How a run works

- **The agent:** headless Claude Code (`claude -p`), on David's Claude subscription.
- **The game:** the agent's only tools are the game's: `look`, `status`, `move`, `punch`, `kick`, `dash`, `grab` and `wait` (`crates/box-mcp`).
  - The game is frozen between calls, so reaction time isn't tested.
  - Each action advances the game a bounded time: a punch takes about half a second, a move up to 2 s.
- **The task:** one run of the game, scored on how many boxes the agent escapes before it dies.
  - To escape, the agent must heap the debris of defeated enemies under itself until the pile reaches the rim, 1.5 m in box 1.
  - The rules are in the server's instructions, which every model gets word for word.
- **Isolation:** the agent sees no personal instructions (`CLAUDE_CODE_DISABLE_CLAUDE_MDS`) and no tools besides the game's. The runner stops a sweep if a session bills an API key, or has any other tool.
- **Verification:** a result counts only if replaying the session's recorded actions on the same game build reproduces its outcome bit for bit (`box_env::replay`).
  - Every run here was verified.
  - The evidence sits beside each result: `record.json`, `transcript.jsonl` and `mcp.json`.
- **Baselines:** two agents need no model and play natively on the same seeds.
  - `brawler` walks at the nearest enemy and punches it.
  - `random` presses random actions.

Run it: `bash scripts/native.sh run -p box-bench -q -- --model sonnet --seeds 1,2,3,4,5 --trials 3 --max-calls 80 --allow-sweep` (see `crates/box-bench`).

## Results v1: the mini box

**Setup:**
- **Box:** a "mini box": box 1 with a 0.7 m rim (David: "Progress score is a good reward shaping option", and the RL agent "Probably should" be in it).
- **Rules:** escaping needs the pile under you at the rim *against a wall* (Q81).
- **Runs:** 5 seeds × 3 trials per agent. Every agent gets 60 s of game time; LLM runs also stop at 80 tool calls, which ended them at 14–42 s.
- **Scoring:** escapes are the headline. The **progress score** (the episode's summed reward: kills, pile at a wall, escapes, minus harm and death) is the secondary number, and also the RL agent's training objective.
- **Agents:**
  - LLMs: headless Claude Code 2.1.305 with Haiku 5.5, Sonnet 5.5 and Opus 5.5.
  - Model-free, played natively: the RL policy, the scripted brawler, and random.
  - The policy samples its actions as it was trained to, from a generator seeded per seed and trial, so its runs repeat and its trials differ.
- **Verification:** every run below was replayed and reproduced its outcome.

| Agent | Verified | Escaped | pass^3 | Progress score (mean) | Kills | Survived (s) | Best pile (share of rim) | Calls |
|---|---|---|---|---|---|---|---|---|
| **RL policy torch-mini-1** | 15/15 | **12/15** | **40%** | **32.0** | 26.7 | 54 | 74% | n/a |
| Opus 5.5 | 15/15 | 3/15 | 0% | 8.6 | 9.9 | 33 | 72% | 73 |
| brawler (scripted) | 15/15 | 0/15 | 0% | 5.2 | 11.8 | 28 | 56% | n/a |
| Haiku 5.5 | 15/15 | 0/15 | 0% | 0.1 | 7.1 | 24 | 49% | 55 |
| Sonnet 5.5 | 15/15 | 0/15 | 0% | −0.6 | 5.9 | 22 | 38% | 54 |
| random | 15/15 | 0/15 | 0% | −2.2 | 4.7 | 22 | 39% | n/a |

`python bench/table.py 1` regenerates the table from `bench/results/` (sweeps marked `bench_version: 1`).

**The RL policy:**
- **Training:** PPO in PyTorch on the RTX 4060 (`ml/ppo_torch.py`), 10M env steps in the mini box, about 2.4 h at 1,170–1,640 steps/s, 16 envs in 16 `box-vec` processes.
- **Network:** a 2×256 tanh actor and critic over the game's 175-float observation.
- **Files:** `ml/models/torch-mini-1/` holds `policy.bin` (the actor, read by `box_env::policy`), the run's config, and its learning curve, every 25th update.
- **Learning curve:** its training episodes' escape rate rose to 32% at 2M steps, 51% at 4M, and about 70% from 6M on.
- **Evaluation protocol:** the policy is evaluated as it was trained, as a stochastic policy (PPO's sampling is its exploration and part of what it learned), not greedily. Its runs hug the walls: it picked up the wall rule from the climb reward alone.

**What it shows:**
- **The mini box separates the agents.** One LLM escapes at all: Opus, in 3 of 15 runs, with the most kills and the highest piles of the three.
- **A small trained policy does far better.** About 120k parameters for the actor, running in microseconds a step, escapes 12 of 15 at no API cost.
- **The comparison is unequal by design, and that's the point.** The LLMs read rules and act turn by turn from text; the policy has 10M steps of experience. Both are measured on the same seeds, box and time limit, with replays checked.
- **Caveats:**
  - Five seeds, one training run (one seed), no confidence intervals.
  - The LLMs act through coarse tools, which costs them precision in fights; the policy acts every 4 frames.

## Results, 2026-10-21 (v0: the agents' heightmap was blank)

**Read these with a flaw in mind.** The observation's `pile_heights` grid was all zeros in these runs: the pile-height test was given `f32::MAX` as its ceiling, and its downward ray lost all precision. Agents still saw the pile under their own feet (`standing_on_pile`), but not where any heap stood. It is fixed (`debris::ANY_HEIGHT`, tested in the scenarios). These results stand as v0 of the benchmark; a rerun on the fixed build is the next sweep. The rules have changed since too: a pile now only gets you out against a wall (Q81), and agents see their distance to the nearest wall. The baselines don't read the heightmap, so their numbers are unaffected.

**Setup:**
- 5 seeds × 3 trials per agent, at most 80 tool calls per run.
- Game at commit `b589b2e`. The tree was dirty only in `bench/` and `crates/box-bench`, not the game.
- Claude Code 2.1.305; models Haiku 5.5, Sonnet 5.5 and Opus 5.5.
- The whole sweep used about 7% of the plan's five-hour window. Claude Code's own list-price estimate was $24.25: Opus $18.83, Sonnet $5.00, Haiku $0.42.

| Agent | Verified | Escaped | Kills (mean) | Survived (mean, s of play) | Best pile (mean / max, share of rim) | Calls (mean) | Flags |
|---|---|---|---|---|---|---|---|
| Opus 5.5 | 15/15 | 0 | 9.7 | 30.5 | 33% / 45% | 70 | spread the pile 1 |
| Haiku 5.5 | 15/15 | 0 | 7.9 | 24.4 | 31% / 55% | 63 | none |
| Sonnet 5.5 | 15/15 | 0 | 7.1 | 24.2 | 19% / 46% | 50 | looped 1 |
| `brawler` (scripted) | 15/15 | 0 | 10.2 | 25.3 | 42% / 56% | n/a | none |
| `random` | 15/15 | 0 | 3.7 | 20.8 | 16% / 54% | n/a | hugged walls 3 |

**How the runs ended:**
- Every run died in box 1, except 4 Opus runs that ran out of calls.
- The call cap binds only Opus. It plays longest, and its looks and status checks spend calls without advancing the game.

**What it shows:**
- **No agent escapes box 1.** This includes the scripted brawler, which beats every model on kills.
  - The models fight where they stand and don't heap debris toward the rim. Their best piles are incidental: the most any run stood on was 55% of the rim.
  - So the headline metric, escape rate (and pass^k), is at the floor for every agent and can't rank them yet. Kills, survival and best pile do.
- **Opus is the strongest model:** most kills, longest survival. It still doesn't reach the scripted brawler's kill count.
- **Caveats:** one game build, five seeds, and no confidence intervals. The trials of the brawler are identical: it is deterministic. Treat the ranking as provisional.

## Known limits

- Results are tied to the game's commit: a gameplay change can stop old traces from replaying.
- Replays are checked natively, and only reproduce on the platform a run was recorded on: v1 was recorded on Windows, and on Linux at the same commit only 28 of its 75 runs replay to their outcome (the game's floats drift). A run recorded in the browser (wasm) isn't yet known to replay identically. `crates/box-bench/tests/v1.rs` checks on any machine that the published runs read back and summarise to these numbers; `scripts/bench-v1-gate.sh` replays them, on Windows.
- The agent is Claude Code with its own system prompt, not a bare model. The numbers measure that harness too.
