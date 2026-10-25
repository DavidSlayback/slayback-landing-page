# Spec: *Out of the Box* (v1)

Sources: `docs/planning/puzzle-decisions.md` (P1–P23 and the engineering defaults), `docs/planning/ml-track.md` (box-env, box-mcp, box-bench, the PPO trainer), `bench/README.md` (bench v1's protocol and results).

## Problem

The site's only game, *Escape the Box*, is real time. That makes it a poor LLM benchmark: an agent that thinks between frozen turns still loses to its own slowness at reading a 3D scene, and bench v1 measured the wrong thing as much as the right one (RL 12/15 escapes, Opus 3/15, every other agent 0/15). The ML track also lacks a result where language models, RL and an exact solver can be ranked on the same task with a ceiling known in advance.

## Solution

A turn-based puzzle in the style of Block Dude. You stand in a pit of walls and loose blocks, carry blocks one at a time, stack them into steps and climb to an exit. Every level comes from a seed. A solver proves each level solvable and measures its shortest solution, which sets its difficulty tier and move budget.

The same rules crate serves four front ends:
- **Browser:** `/puzzle` on the site, in the hero's neon look with `sim` sparks, for keyboard, touch and gamepad.
- **MCP server:** LLM agents play a level through text tools.
- **RL env:** a PPO policy trains on generated levels and is tested on seeds it never saw.
- **Bench:** the generalised `box-bench` runs LLMs, PPO, the solver and the simple baselines on one seed suite. A result counts only once its record replays.

## Rules (`puzzle-core`)

The grid is at most 20 wide and 12 tall; y grows downward. Each cell holds empty, wall, block or exit. The state is the player's cell, their facing, whether they carry a block, and the set of block cells. Blocks are identical, so the state hashes as a bitset (240 cells fit in four `u64`s).

| Key | Effect | Refused when |
|---|---|---|
| `L` / `R` | If facing that way: step one cell, then fall. Otherwise: turn in place (this counts as a move) | The target cell is not empty, or you carry a block and the cell above the target is not empty |
| `U` | Climb onto the cell in front: up one, forward one | The cell in front is empty, the cell above it is filled, the cell above you is filled (apart from the block you carry), or you carry a block and the cell two above the one in front is filled |
| `D` | Not carrying: lift the block in front onto your head. Carrying: put it in the cell in front at head height, where it falls until it rests | Lift: there is no block in front at your level, something sits on it, or the cell above you is filled. Put: the cell in front at head height is filled |

- **Gravity:** after every move, the player (with any carried block) falls until supported. A put-down block falls until it rests. Nothing else moves.
- **Exit:** the level is solved when a move ends with you on the exit tile; falling through it on the way down doesn't count. Keys after that are ignored.
- **Refused moves** change nothing and do not count. In a move string, the first refusal stops the string; the reply names the key, its position in the string and the reason.
- **Moves that count:** those on the board at the end. `undo` takes the last one back, `restart` clears them all. Neither costs a move (P10).
- **Text form** (P4), one line per row:

  ```text
  #.........#
  #......E..#
  #...#..##.#
  #>..##.B..#
  ###########
  # wall  B block  E exit  . empty  > or < you, facing that way
  ```

  A carried block is drawn as `B` in the cell above you. A block on the exit, resting or carried, is drawn as `b`. The same characters parse back into a level, so tests and fixtures are plain text. Parsing refuses anything floating, since the rules never leave anything there.

These rules are written as tests before the code, one per row of the table, refusals included (TDD).

## Solver and generator

- **Solver:** A* over states. The heuristic is `max(|dx|, rise)` to the exit, where `rise` is how far the exit is above you. It is admissible because each move changes x by at most 1 and climbs at most 1. It returns the shortest solution and the number of states it expanded. It is the oracle row in the bench and the source of each level's tier and budget.
- **Generator** (`puzzle_core::generate`, built in OB3):
  - **Why it climbs:** random pits almost never need more than about 40 moves (OB2's measurement), so drawing and filtering can't reach tier 4. Instead it climbs.
  - **How it climbs:** from a seed it builds a pit (a rolling floor, a few blocks, the exit and the player in different columns), then changes one thing at a time: a wall cell, a block added to or taken from a column, where the exit or the player drops.
  - **What survives:** a change is kept when the level stays solvable within the cap and scores no worse. From tier 2 up, the score is first whether a block is needed, then the length of the shortest solution.
  - **When it stops:** once the length is in the band and, from tier 2 up, the level can't be solved with every block turned to wall.
  - **Bounds:** at most 3,000 changes per climb and 8 climbs per seed.
- **Tiers** (`TIERS`): tier 1 8–15 moves on 8×6, tier 2 16–40 on 12×8, tier 3 41–80 on 16×10, tier 4 81–150 on 20×12. Tier 1's floor of 8 moves is an agent choice: P16 said only "≤15", and a 3-move walk tests nothing.
- **Cap** (`CAP`): 200,000 expansions per candidate, about 0.3 s at worst. A candidate that hits it is thrown away as too costly to verify. A solution found under the cap is still a shortest one.
- **Determinism:** integers only, one splitmix-seeded xorshift per (seed, tier), so the same seed gives the same level everywhere. `bench/puzzle/suite-v1.json` is the seed-to-level table; `tests/generate.rs` regenerates it and fails if any level's fingerprint (FNV-1a of its text form) or solution length changes. Wasm and Python parity is checked when those front ends exist (OB7, OB8).
- **Seed ranges:** below 1,000,000 for the bench (suite v1 uses 1–10 per tier); 1,000,000 up to 2^32 for RL training; 2^32 up for a run's fresh seeds.
- **Risk to measure first:** how long the solver and generator take on tier 4. If search on 20×12 is too slow, shrink tier 4's grid or bound its expansions, and record that in the decision log.
- **Measured (OB2, 2026-10-25):**
  - **Setup:** `examples/solve.rs`, 50 random settled levels per size and block density, release build on David's laptop. These are random pits, not generator levels.
  - **Solvable levels are cheap:** the slowest took 13 ms (34k expansions), and the median at every size is under 0.01 ms. None needed more than 40 moves, so tier-4 cost (81–150 moves) is still unmeasured.
  - **Unsolvable levels have a heavy tail:** the search must exhaust every reachable state. At 20×12 with blocks on 15% of cells, the median was 38 expansions, but one level took **67 s and 46 M expansions** (about 0.7 M expansions/s, several GB of memory). At 12×8 to 16×10 the worst cases were 0.1–0.24 M expansions, under 0.1 s.
  - **Consequence for OB3:** the generator needs an expansion cap and must discard any level that hits it ("too costly to verify"). The cap, and whether tier 4 still fills at 20×12 under it, is OB3's gate.
- **Measured (OB3, 2026-10-25), the gate passed:**
  - **Setup:** `examples/generate.rs`, release build, cap 200k.
  - **Results:**

    | Tier | Grid | Seeds filled | Median | Worst | Shortest solutions |
    |---|---|---|---|---|---|
    | 1 | 8×6 | 10/10 | <1 ms | 2 ms | 8–12 |
    | 2 | 12×8 | 10/10 | 2 ms | 11 ms | 17–32 |
    | 3 | 16×10 | 10/10 | 0.17 s | 0.44 s | 41–63 |
    | 4 | 20×12 | **30/30** | 0.76 s | 11.4 s | 81–116 |

  - **Tier 4 fills at 20×12; no shrink needed.**
  - **Solutions cluster at the low end of each band,** because the climb stops on entering it. Tier 4 never went past 116 moves, so 117–150 is empty in practice.
  - **They are block puzzles, not mazes:** tier-4 solutions use 24–26 lifts and put-downs and 11–14 climbs.
  - **Known issue for the human campaign (OB9):** wall toggling leaves floating wall cells and piles in blocks (up to 39 in a tier-4 level), so levels look cluttered rather than like a pit. The bench doesn't care. The campaign should either pick the cleanest levels or add a tidiness term to the score.

## Agents through MCP (`puzzle-mcp`)

A hand-rolled MCP server over stdio, as in `box-mcp`. One session plays one level.

| Tool | Does | Returns |
|---|---|---|
| `look` | Nothing | The grid, facing, carrying, moves used / budget, calls used / budget, tier |
| `move(keys)` | Plays a string of `L R U D` until it ends, solves or hits a refusal | The same as `look`, plus how many keys played and any refusal |
| `undo(n = 1)` | Takes back n moves | As `look` |
| `restart` | Back to the start | As `look` |
| `status` | Nothing | The counters only, without the grid |

- **Instructions:** the server's instructions give the full rules and legend, with no worked example (P11), and say that every action returns the new grid. In box-bench, agents called `look` before nearly every action because nothing told them they didn't need to.
- **Budgets:** moves 3× the shortest solution (P10), 30 calls (P21). Running out of either ends the session unsolved.
- **Record:** `--trace FILE` writes the session's record: seed, tier, every call with its arguments and reply, and the outcome. The bench replays it against `puzzle-core` before counting it.

## The bench (generalised `box-bench`)

- **The seam (P23):** a `Game` trait in `box-bench`: its name, the MCP server to launch and its tool names, a `replay(record) -> Outcome`, the per-record analysis, and its native players. Escape the Box and *Out of the Box* each implement it. The billing guard (strip `ANTHROPIC_API_KEY`, stop on an API-key source), the CLAUDE.md isolation, the tool allowlist and replay-before-count stay shared and unchanged.
- **Regression gate:** after the refactor, box-bench's v1 records replay and summarise to the same numbers as `bench/README.md`.
- **Suite v1 (P13):** 10 seeds per tier, found by scanning upward from a fixed starting seed per tier and published in `bench/puzzle/suite-v1.json` with each level's hash and shortest solution. Each LLM run also plays 1 fresh seed per tier, drawn from OS randomness at run time and kept in the record. Fresh seeds scoring clearly above the suite's would point to contamination.
- **Rows:** Haiku, Sonnet and Opus through `claude -p` (P12: 120 sessions plus 12 fresh-seed sessions); the solver; the PPO policy; `random` (uniform keys); `greedy` (moves toward the exit and climbs when it can, otherwise takes a random legal move).
- **Numbers:** headline is solve rate per tier (P5). Secondary numbers on solved levels: moves over the shortest solution. For every session: calls, input and output tokens, refused-move rate (a direct measure of whether the agent understood the rules), and undo and restart counts.
- **Output:** `bench/puzzle/results/<stamp>-<model>/` with the summary, each record, the transcript and the MCP config, as in box-bench. `bench/puzzle/README.md` holds the results table and how to reproduce it.

## RL (`puzzle-env`, `ml/`)

- **Env:** `reset(seed, tier)` / `step(key)` straight over `puzzle-core`, with no Bevy. Steps should run in the millions per second, a different regime from box-env's ~2,000.
- **Actions:** `L R U D`. RL plays without undo; the episode ends at the move budget.
- **Observation:** a one-hot grid padded to 20×12 (wall, block, exit, player facing left, player facing right, carried block), plus the moves left.
- **Reward:** +1 on solve, −0.01 per move. If PPO can't learn from that sparse signal, add a shaping term for distance to the exit and record the change.
- **Binding:** the binary stdio protocol of `box-vec`, batched in Rust. Trainer: `ml/ppo_torch.py` on CUDA, with a curriculum by tier. Training seeds are drawn from a range kept apart from the suite's.
- **Bench row:** a Rust forward pass of the trained policy, as `box_env::policy` does, so the bench plays it natively. Whether it generalises to tiers 3 and 4 is a finding either way.

## Browser (`puzzle-wasm`, `site/puzzle/`)

- **Build (P7):** C-ABI exports with no wasm-bindgen and no imports, like `hero-wasm`.
  - Rust owns the rules, the level, the animation and the drawing, into an RGBA buffer in wasm memory.
  - `site/puzzle/puzzle.js` uploads the buffer to a canvas, reads input, polls the Gamepad API and keeps `localStorage`.
  - Size budget: ~300 KB gzipped, checked in CI like the hero's.
- **Look (P14, P18):**
  - Dark ground with glowing outlines: walls dim, blocks bright, the player and exit in the scheme's accent colour.
  - Colours come from the site's scheme in `localStorage` (`slayback-scheme`, written by `site/hero.js`).
  - Moves tween in about 80 ms and falls ease in, so the game stays turn-based but never snaps.
  - `sim` throws sparks when a block lands and bursts on exit.
- **Comfort:** reduced motion drops the sparks and tweens. The player, block, wall and exit differ in shape as well as colour.
- **Controls (P19):**
  - Keyboard: arrows or WASD; Down or S lifts and puts down; Z or Backspace undoes; R restarts.
  - Touch: four large buttons, plus undo and restart.
  - Gamepad: d-pad or stick to move; up climbs, down lifts and puts down; B undoes; Start restarts.
- **Modes (P17):**
  - **Campaign:** about 20 generated levels, picked by hand to ramp up, with progress and best moves per level kept in `localStorage`.
  - **Daily:** a level from a seed hashed from the UTC date, so everyone gets the same one. Its tier rotates by weekday (P22). Your moves are shown against the solver's.
  - **Watch the agents:** on suite levels, replay how Opus, Sonnet, Haiku, PPO and the solver played. The build copies the bench records into `dist/puzzle/replays/` as action lists, and the page replays them with the same rules. A caption gives the outcome, moves against the shortest solution, and calls.
- **Placement:** `/puzzle`, linked from the site nav and the lab page (M7). It releases independently of Escape the Box's gate (P15): production once David has played the rc build and bench v1 results exist.

## Testing and evidence

- `puzzle-core`: a test per rule and per refusal; solver optimality on hand-written levels whose shortest solution is known; the seed-to-level hash table; a check that the gravity invariant (nothing floats) holds after every move in random play.
- `puzzle-mcp`: protocol tests (initialize, tools/list, each tool), refusal and budget replies, and a recorded session that replays.
- `box-bench`: Escape the Box v1 reproduces; *Out of the Box* runs end to end with the native players before any LLM run; one smoke session per model before the 120-session sweep.
- Browser: Playwright on `dist/`. The page loads, a tier-1 level is solved by key presses, undo and restart work, the daily level is stable across reloads, a replay plays to its recorded outcome, and the size budget holds.
- Measurements to record: generator and solver time per tier, env steps per second, wasm size.

## Build order

Each step ships on its own and unblocks the next. The spec issue is #101; each step's ticket is labelled `track:puzzle`.

1. (OB1, #102) **Rules:** `puzzle-core` rules, text form, tests.
2. (OB2, #103) **Solver**, with measured search times. *Blocked by 1.*
3. (OB3, #104) **Generator, tiers, suite v1**, with the tier-4 time gate. *Blocked by 2.*
4. (OB4, #105) **`puzzle-mcp`** and a smoke session. *Blocked by 3.*
5. (OB5, #106) **Generalise `box-bench`**, check that Escape the Box reproduces, and plug in the puzzle with the native players. *Blocked by 4.*
6. (OB6, #107) **Bench v1 sweep** and `bench/puzzle/README.md`. *Blocked by 5; the PPO row joins after 7.*
7. (OB7, #108) **`puzzle-env`, PPO training, the policy's Rust forward pass.** *Blocked by 3.*
8. (OB8, #109) **`puzzle-wasm` and `/puzzle`:** one level playable with keys, the neon look, sparks. *Blocked by 3.*
9. (OB9, #110) **Campaign, daily level, touch, gamepad, reduced motion.** *Blocked by 8.*
10. (OB10, #111) **Agent replays on `/puzzle`.** *Blocked by 6 and 8.*
11. (OB11, #112) **Lab page row, nav link, release** once David has played the rc build. *Blocked by 9 and 10.*

## Out of scope for v1

A human leaderboard (it needs a backend); image observations; pass^k (later, on a subset); endless play by tier; pushing blocks; a level editor; WebMCP.

## Open risks

- **Tier-4 search time**, unmeasured. It could force a smaller tier-4 grid (step 3 gates on it).
- **PPO on tiers 3–4** may not learn from the sparse reward. A failure to generalise gets reported, not hidden.
- **Subscription usage:** 132 LLM sessions of up to 30 calls each. Smoke runs come first, and the runner's sweep guard (`--allow-sweep`) stays on.
