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

## Results, 2026-10-21

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
- Replays are checked natively. A run recorded in the browser (wasm) isn't yet known to replay identically.
- The agent is Claude Code with its own system prompt, not a bare model. The numbers measure that harness too.
