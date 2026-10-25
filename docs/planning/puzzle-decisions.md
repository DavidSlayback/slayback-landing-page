# *Out of the Box*: decision log

A turn-based puzzle on the site that is also an LLM and RL benchmark. It comes from a brainstorm on 2026-10-25 about what the site's next game should show that *Escape the Box* doesn't. In bench v1 the RL policy escaped 12 times out of 15 and the best LLM 3. A turn-based game removes reaction speed and tests planning alone, so LLMs, RL and the solver compete on fairer terms. David: "Spec the turn-based puzzle game as an LLM/RL benchmark on the site: rules crate, MCP server, bench, browser front end. Grill me on the open choices first."

Every row below was picked by David in the grilling of 2026-10-25, unless its source says otherwise. The spec is `docs/planning/puzzle-spec.md`.

## Round 1: the game and the bench

| # | Decision | Answer | Notes |
|---|---|---|---|
| P1 | Puzzle family | **Climb-out gravity, in the style of Block Dude** | Escape the Box as a turn-based puzzle: carry and stack blocks to climb out. Not a standard benchmark, so LLMs have no memorised solutions. Rejected: Sokoban (contaminated, but comparable with Boxoban) and rule rewriting in the style of Baba Is You (hardest to generate). |
| P2 | Where levels come from | **Both**: a seeded generator checked by a solver for the bench and RL, plus a curated set for humans taken from the generator | |
| P3 | How an agent acts | **Move strings plus undo**: `move("LLUR")` takes any number of steps per call, plus `undo` and `restart` | In box-bench, one move per call cost Haiku ~395k input tokens over 40 calls. Moves and calls are counted separately. |
| P4 | What the LLM sees | **ASCII grid plus legend** | JSON with coordinates stays available as a flag, for the RL encoder and debugging. |
| P5 | Headline metric | **Solve rate per difficulty tier**, within the move budget | Secondary: moves over the shortest solution, calls, tokens. |
| P6 | Rows beside the LLMs | **Exact solver, PPO policy, random/greedy** | Humans are deferred: a human row needs a submission backend, as with Escape the Box's human board. |
| P7 | Browser build | **Hero-style tiny wasm**: C-ABI exports with no wasm-bindgen, Rust draws into a pixel buffer, a thin JS file blits it | Loads in well under 1 MB and builds in seconds. No Bevy, so no shader-link stalls. |

## Round 2: rules, budgets, release

| # | Decision | Answer | Notes |
|---|---|---|---|
| P8 | Mechanics | **Classic Block Dude**: walk left or right, climb a 1-tile step, carry one block, set it down in front. Gravity acts on blocks and on you. Pressing the other direction turns you in place first | No jumping, no pushing. |
| P9 | Escaping | **An exit tile** placed by the generator | David overrode the recommendation (over either pit wall). Difficulty is easier to aim with one goal. |
| P10 | Move budget | **3× the shortest solution; undo is free** | A move counts if it is on the board at the end; undo removes it. Calls have their own budget (P21). |
| P11 | What agents are told | **The full rules, no worked example** | The same text for every agent, so the result measures reasoning, not imitation. |
| P12 | Bench v1 size | **4 tiers × 10 levels × Haiku, Sonnet, Opus, 1 trial**: 120 sessions | pass^k comes later, on a subset. |
| P13 | Seeds and contamination | **A fixed public v1 suite, plus fresh seeds each run** | The fresh seeds are drawn at run time, kept in the record and never published in advance. A gap between the two scores flags contamination. |
| P14 | Placement and look | **`/puzzle`, in the hero's neon look** | David overrode the recommendation (Escape the Box's look). Linked from the lab page and the site nav. |
| P15 | Release | **Independent of Escape the Box's gate (Q38)** | Ships to production once David has played the rc build and v1 results exist. |

## Round 3: tiers, players, feel

| # | Decision | Answer | Notes |
|---|---|---|---|
| P16 | Difficulty tiers | **By shortest solution**: tier 1 ≤15 moves, 2: 16–40, 3: 41–80, 4: 81–150, on grids up to 20×12 | The solver measures each level. |
| P17 | What humans get | **Curated campaign, daily seed, agent replays** | Endless play by tier was not picked. |
| P18 | What "neon" means | **Glowing outlines in the site's colour scheme, plus `sim` sparks** when a block lands and on exit | `sim` joins the wasm; it is small. |
| P19 | Human controls | **Keyboard, touch and gamepad** | |

## Round 4: names and numbers

| # | Decision | Answer | Notes |
|---|---|---|---|
| P20 | Name | ***Out of the Box*** | |
| P21 | Call budget | **30 MCP calls per level** | A move string can carry a whole tier-4 solution. |
| P22 | Daily tier | **Rotates by weekday**: Monday tier 1 to Thursday tier 4; Friday to Sunday repeat tiers 2 to 4 | |
| P23 | Bench code | **Generalise box-bench** so its runner isn't tied to one game | Both benches then share the billing guard, the CLAUDE.md isolation and the rule that a result counts only after it replays. Box-bench v1 must still reproduce afterwards. |

## Engineering defaults (agent choice, confirmed by David)

Proposed at the end of the grilling. David: "Yes, write it".

- **Crates:** `puzzle-core` (rules, generator, solver; no dependencies), `puzzle-wasm` (C-ABI exports, renderer, `sim` sparks; budget ~300 KB gzipped), `puzzle-mcp` (tools `look`, `move`, `undo`, `restart`, `status`), the generalised `box-bench`, and an RL env protocol modelled on `box-vec`.
- **Rules:** Block Dude's exact rules, written as tests first.
- **Generator:** random pit layouts, kept only if an A* search over (position, facing, carried block, blocks) proves them solvable within a tier. **Risk:** tier 4 search time is unmeasured. Measure it before committing to 20×12.
- **PPO:** the existing PyTorch/CUDA trainer, observations one-hot encoded and padded to 20×12, a curriculum by tier, and training seeds kept separate from bench seeds.
- **Proof:** a session counts only once its trace replays against `puzzle-core`.
- **No backend:** the daily seed is a hash of the date computed in the browser. Agent replays ship as static JSON from the bench records.
- **Tracking:** this log, `puzzle-spec.md`, and GitHub Issues as with Escape the Box (Q10).

## Built in OB3 (agent choice, 2026-10-25)

- **Tier 1's band is 8–15 moves,** not 1–15. A 3-move walk tests nothing.
- **Grids per tier:** 8×6, 12×8, 16×10 and 20×12.
- **Levels are climbed, not filtered:** random pits never needed more than 40 moves, so filtering couldn't fill tier 4.
- **Expansion cap of 200,000 per candidate.** At that cap tier 4 filled 30 of 30 seeds at 20×12, so its grid was not shrunk (the spec's "Measured").
- **Seed ranges:** below 10^6 for the bench, 10^6 to 2^32 for RL training, 2^32 and up for fresh seeds.
- **Suite v1** is seeds 1–10 of each tier, in `bench/puzzle/suite-v1.json` with each level's text, fingerprint and solution.
