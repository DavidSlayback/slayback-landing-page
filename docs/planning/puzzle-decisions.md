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

## Round 5: reach and look (2026-10-26)

David: "Can we make it easier to get to the puzzle page from the main page? Also, I'd like a more interesting art style and maybe some animation."

| # | Decision | Answer | Notes |
|---|---|---|---|
| P24 | Where the homepage link shows | **Production now**: a "Puzzle" nav item and a "Try the puzzle" call to action, and `/puzzle` itself, on slayback.pages.dev | **Overrides P15**, which waited for David's rc play and bench v1. The game's own link stays behind its switch. |
| P25 | Art direction | **Neon arcade, richer**: a backdrop with depth (a faint grid, drifting embers), walls with bevels and brick courses, blocks as glowing crates, CRT scanlines and bloom | Keeps the hero's neon (P14) and adds a world around it. |
| P26 | Animation | **All four**: character life (walk cycle, climb pose, idle breath and blink, a lean on turning); weight (squash and stretch, a small shake when a block lands, dust); a living backdrop (embers, the doorway shimmering); a level intro (blocks drop in) and a win (through the door, the crates break apart) | Reduced motion turns all of it off. |

## Round 6: mechanics (2026-10-26)

David found that tier 1 level 1's shortest count (10) includes its turns, then: "I think we could use more mechanics. Maybe some physics based ones." The candidates, their rules and their cost are in `puzzle-mechanics.md`. David: "Recommendation" picked crumbling floor, springs and ice crates to grill first; balloons, the lift and tall crates are not decided.

| # | Decision | Answer | Notes |
|---|---|---|---|
| P27 | Turning | **Still costs a move** (David: "Turns is the same") | Keeps P8; every shortest count stays comparable with suite v1. |
| P28 | What breaks a cracked tile | **Being left**: it breaks when whatever stands directly on it leaves | No settle pass needed for this rule. |
| P29 | Who breaks it | **Crates count**: lifting a crate off it, or a crate sliding or bouncing off it, breaks it too | |
| P30 | Spring height | **As high as you fell, at least 1** | A walk-on (a fall of 0) throws 1. Rejected: a fixed 2. |
| P31 | Walking onto a spring | **Launches you** | So a spring is also a hazard you may cover. |
| P32 | Spring chains | **Chain**: a bounce that ends on another spring bounces again | Every bounce moves a cell forward, so a chain ends. |
| P33 | Crates on springs | **Crates bounce too**, forward the way you faced when you set them down | Rejected: a crate holding the spring down. |
| P34 | Ice at a ledge | **Keeps sliding**: it falls, then slides on the same way | Rejected: falls and stops. |
| P35 | Landing on ice | **Anything landing on it knocks it sliding**, you included, the way the lander was going (your facing, for you); the lander drops into the gap | Rejected: slides only when set down. |
| P36 | Ice hits ice | **Passes it on**: the slider stops and the one it hit slides on | Hitting a normal crate or a wall just stops it. |
| P37 | Riders on pushed ice | **Ride along** when the ice is pushed from the side (P36). A lander from above still drops (P35) | David confirmed the split: "Yes, record it". |
| P38 | Worlds | **One mechanic each, then mixed**: Crumble, Ice and Springs worlds, and a final world that mixes all three | The bench scores each mechanic on its own. |
| P39 | Suite v2 | **5 levels per tier per new world** (80 levels); suite v1's 40 classic levels stay frozen | Keeps a full run's API cost near v1's. |

Agent defaults that follow from P28-P39 (agent choice):

- **A bounce stops at a ceiling.** If its forward cell is blocked, the bouncer comes down onto the spring and stays (no re-trigger), so nothing bounces forever.
- **A crate with no direction** (one that drops because ice slid out from under it) bounces straight up and so stays on its spring.
- **In the mixed world the rules simply combine:** ice that lands on a spring bounces the way it was sliding and keeps sliding after.
- **M2's "mechanic matters" check:** a Crumble level must change its shortest solution when every cracked tile is made wall (unsolvable, shorter or longer), not only become unsolvable, so levels where breaking costs you a path count too.
- **M3, ice (agent choices):** a landing needs a fall of at least a cell (walking or climbing onto ice doesn't knock it); a lander with no way of its own just lands; a rider that would hit something stays behind and falls. You can never ride ice (everything you set moving moves away from your column), so the solver keeps its estimate in the Ice world, unlike the Search default below. Stacks fall together in one round.
- **M1 builds settle only; crate kinds come with ice (M3 #120).** A kind enum with one member would be dead code until then.
- **Search:** with chains, one move can carry you any distance, so the Springs and mixed worlds solve with no estimate (plain shortest-path search). Ice doesn't need that after all (M3 above).

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
