# Out of the Box: new mechanics (study)

David, 2026-10-26: "I think we could use more mechanics. Maybe some physics based ones." Then: "Turns is the same. Look at all the low cost mechanics. Also consider balloons and crumbling." This study works out the rules, edge cases and cost of each candidate, so the open choices can be grilled. Nothing here is decided except P27 (turns still cost a move).

## What every mechanic must keep

- **Deterministic and discrete.** A move plays out until everything is at rest, as falling does now.
- **Text first.** One new character per thing, so the LLM's grid and `Level::parse`/`show` stay the source of truth.
- **The solver stays exact.** A* must still prove the shortest solution within the generator's 200,000-expansion cap (OB3).
- **The four keys stay.** No new key, no pushing (P8).

## Two findings that shape all of them

**1. The heuristic is not where the cost is** (measured). Springs and a lift weaken A*'s lower bound, so I re-solved all 40 suite v1 levels with weaker estimates. Halving it (what springs need) cost 1.14–1.26× the expansions; dropping the rise term (what a lift needs) cost 1.03–1.07×. Almost all of the search is over crate arrangements, not over where the player is. The real cost of a mechanic is how much it multiplies the state space.

| Tier | Halved estimate | Climb term dropped |
|---|---|---|
| 1 / 2 | 1.20× / 1.26× | 1.07× / 1.03× |
| 3 / 4 | 1.20× / 1.14× | 1.06× / 1.03× |

**2. Today nothing ever needs to settle.** A crate can only be lifted when nothing is on it, so crates never lose their support, and only a set-down crate falls. Balloons, crumbling floor under crates, and the lift all break that: after a move, things can be left unsupported (or, for balloons, uncapped). They need a shared **settle** pass: repeat "unsupported crates fall one cell, uncapped balloons rise one cell" until nothing moves. The order within a pass must be fixed (crates bottom up, balloons top down) so the result is deterministic. Built once, it serves all three.

## The candidates

### Ice crates (`I`)

- **Rule:** an ice crate you set down slides in the direction you face until the next cell is solid.
- **State:** a second crate set (`Bits`), and what you carry becomes none / crate / ice. That splits crate arrangements by kind, so the state space grows with the number of ice crates.
- **Settle:** none. The slide is part of the set-down, like the drop.
- **What it adds:** where a crate ends up depends on what is beyond it, so a wall far away becomes a tool. Setting ice on top of a stack sends it off the far side.
- **Open choices (David):** at a ledge, does it fall and stop, or fall and keep sliding? Does an ice crate also slide when something lands on it? (Recommend: falls and stops; no.)

### Springs (`^`)

- **Rule:** a spring is a floor tile. Arriving on it, by walking or falling, throws you up 2 cells (less if a ceiling is in the way), then 1 cell in the direction you face if that is clear, then you fall. A crate resting on a spring holds it down, so a crate switches a spring off.
- **State:** none. Springs are part of the level.
- **Settle:** none.
- **Solver:** a move triggers at most one bounce, so one move gains at most 2 columns, and 3 rows (a climb onto a spring). The estimate becomes max(⌈dx/2⌉, ⌈rise/3⌉), close to the halved estimate measured above (1.14–1.26×).
- **What it adds:** a climb of 2 without building stairs, and an obstacle: walking across a spring launches you, so sometimes you must cover it with a crate first.
- **Open choices (David):** fixed height 2, or bounce back as high as you fell? Does a crate set onto a spring bounce too? (Recommend: fixed 2; no, it holds the spring down.)

### Crumbling floor (`~`)

- **Rule:** a cracked floor tile breaks when whatever stands directly on it leaves (you walk or climb off, or the crate on it is lifted). It is then empty for good.
- **State:** one bit per cracked tile, as a third `Bits`.
- **Settle:** none for this rule. What left was the only thing on it, so nothing is left hanging.
- **What it adds:** commitment. Moves can't be taken back, so dead ends appear, which is exactly the planning a benchmark should test. Undo still works because the bits are part of the state.
- **Open choices (David):** break when the thing on it leaves, or when anything lands on it after a fall of 2 or more (impact)? Does a crate count, or only you? (Recommend: when it is left, crates included. The impact version needs settle, because a crate breaking through drops whatever is on it.)

### Balloons (`O`)

- **Rule:** a balloon is a crate that rises instead of falls. You lift it from in front of you like a crate. Set down, it rises from in front of your head until something is above it. It is solid, so you can stand on it.
- **State:** a second set, as for ice, and what you carry gains a third kind.
- **Settle:** needed. Lift a crate that a balloon was pressed against and the balloon rises; that can uncap others.
- **The hard edge case:** a crate falling onto a balloon that rises into it. Either the pair floats where they meet (deterministic, odd-looking), or the crate wins and both fall until the balloon's top is the crate's floor (the balloon behaves as weightless). Recommend: the crate wins.
- **What it adds:** a hanging step under any overhang, the mirror image of a stair.
- **Open choices (David):** the crate-meets-balloon rule above; whether a carried balloon slows your fall. (Recommend: crate wins; no.)

### Counterweight lift (`=`)

- **Rule:** two platforms on one rope, at most one rope per level. After each move, the side carrying more weight (you, crates, a crate you carry) sinks until it hits a stop, and the other side rises by the same amount. Equal weight: no motion.
- **State:** one byte, the rope's position.
- **Settle:** needed. Things on the sinking side ride down, things on the rising side ride up and can hit a ceiling (which stops the lift).
- **Solver:** a move can lift you several rows, so the climb term goes, at the 1.03–1.07× measured above.
- **What it adds:** the clearest physics on screen. Weigh a side with a crate to raise yourself, or stand on one side to raise a crate.
- **Open choices (David):** platform width (1 cell, or up to 3); whether the lift moves all the way or one cell per move. (Recommend: 1 cell wide; all the way, like a fall.)

### Tall crates that topple (`H`)

- **Rule:** without pushing, the trigger is `D`: facing a 1×2 tall crate, `D` tips it away from you into a 2×1 bridge, if there is room.
- **State:** crates stop being identical cells. Each one becomes a body with a position and an orientation, so `blocks` turns into a list and every rule (support, lifting, climbing, setting down) learns about two-cell bodies.
- **Settle:** needed (a bridge is supported if either cell is).
- **What it adds:** bridging a gap, which nothing else does.
- **Cost:** the most code by far. It belongs after the others, if at all.

## Ranking

| Mechanic | Cost | Planning it adds |
|---|---|---|
| Crumbling floor | Low: a bitset, no settle | One-way commitment, dead ends |
| Springs | Low: no state, 1.2× search | Two-cell climbs, launch hazards |
| Ice crates | Low: a second set, no settle | Far walls as stops |
| Balloons | Medium: a second set, needs settle | Hanging steps |
| Counterweight lift | Medium: a byte, needs settle | Weight puzzles, the best visual |
| Tall crates | High: bodies instead of cells | Bridges |

Recommendation (agent): crumbling, springs and ice first, because each is a new rule without new machinery. Then build settle once and add balloons and the lift on it. Leave tall crates out of v2.

## What adding any of them touches

- **`puzzle-core`:** the rule, tests first (as OB1), the legend in `parse`/`show`, and the fingerprint (it hashes the text, so it follows).
- **Solver:** the estimate per level (halved only when a level has springs; no climb term only when it has a lift).
- **Generator:** a mutation that places and removes each new tile. Today's from-tier-2 check ("unsolvable with every crate turned to wall") needs a version per mechanic, so a level can't ignore its world's mechanic.
- **Suite:** v1 stays frozen. Each mechanic gets a world and a v2 suite, so the bench can score by mechanic and show which rule a model misreads (fits OB9's campaign).
- **MCP server and RL env:** the legend in the instructions (P11), and one more observation channel per new tile.
- **Front end:** art and motion per tile (`art.rs`, `game.rs`), using the existing squash, dust and spark moves.
