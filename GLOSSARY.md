# Glossary

The game's domain words, as the code uses them. Decisions live in `docs/planning/game-decisions.md`; this file names things.

## The frame

- **Frame**: the order of one game frame, declared once in `game3d/src/lib.rs`. **Decide** (the player's control, the brains, the doors), **Act** (the carry and the bombs), **Resolve** (swings and strikes land, in `Combat`, then debris, the run and the score follow) and **Show** (the look, sound, shake, HUD and screens, plus the bookkeeping that runs on every screen). The first three run only while playing.

## Fighting

- **Blow**: anything that lands on a fighter. It might be a swing (punch, kick, drop-kick, an enemy's smash, slash or jab), a thrown thing, or a bomb blast. A blow is just numbers: damage, stun, launch, hit-stop, which armour shrugs it off, whether it reaches what the player holds, and its feel. Where it came from only decides those numbers. `game_core::combat::Blow`; built by `Blow::swing`, `grab::blow` and `bomb::blow`.
- **Land**: what a blow does to the fighter it reaches, the same for every source (`game_core::combat::land`):
  - nothing on the defeated;
  - nothing on what the player holds, unless the blow reaches held things (only blasts do, and then without launching it);
  - armour shrugs a blow off by taking its damage without stun or launch;
  - a finishing blow freezes for at least the finisher's hit-stop.
- **Strike**: a blow on its way to one fighter, with where it shows and which way it pushes (`game3d::fighter::Strike`). Swings, throws and blasts send strikes, and `fighter::land` applies them all.
- **Hit**: a blow that landed, raised for the burst, the shake and the sound (`game3d::fighter::Hit`). A blast's victims raise no hit, because the blast itself is the spectacle (`Exploded`).
- **Feel**: how a blow reads, meaning its star burst, its screen shake and whether it sounds heavy (`game_core::impact::Feel`). It's set from what struck: a move's own, while a thrown thing reads as a kick.
- **Carry**: the player holding something, from the grab to the throw's release (`game3d::carry::Carry`). It is in one of two phases: **Hold** (out in front in the fists) or **Whirl** (the throw, aimed when the attack starts it, going once round before letting go along that heading). `carry.rs` is the only writer of the carry, of the held thing's **Carried** marker and of the thrown thing's **Thrown** flight.
- **Look**: one fighter's figure in one frame, decided by `pose::look` from its **FigureState** (facing, speed, swing, carry, flash, wind-up, squash) and its **Motion** (walk phase, which arm swings next, the throw's follow-through). It holds the turn, lift, lean and squash, each limb's pose, and the **Tint** (Flash, then Windup, then Own: the part's own paint).
- **Kind** and **Style**: an enemy's kind (fodder, thug, ninja) and everything it is, in one table (`game_core::enemy::Style`): name, capsule radius and height, health, reach, wind-up, walk speed, blow and armour. Only its figure's pieces (`debris::solid::figure`) and colours (the palette) live elsewhere.
- **InBox**: belongs to the current box and goes when it ends (`game3d::run::InBox`): every enemy, debris piece and lit bomb, from its bundle.
- **Armour**: a kind's resistance to being staggered. A thug's armour shrugs off punches but not kicks, throws or blasts.

## Out of the Box (the puzzle)

The words of `crates/puzzle-core`; decisions are in `docs/planning/puzzle-decisions.md`.

- **Level**: the fixed part of a puzzle (walls, the exit, the grid's size) plus where play starts. Its **text form** is one character per cell: `#` wall, `B` block, `E` exit, `.` empty, `>`/`<` the player facing that way, `b` a block on the exit.
- **State**: what changes during play: the player's cell and **facing**, whether they are **carrying**, and the set of block cells. The carried block is not in that set.
- **Key**: one of `L R U D`. `L`/`R` walk the way you face, or turn you to face it; `U` climbs; `D` lifts the block in front or puts the carried one down. A turn is a move.
- **Refusal**: why a key changed nothing (`Blocked`, `CarryBlocked`, `NothingToClimb`, `NoHeadroom`, `NoBlock`, `Covered`, `NoRoom`, `NotAKey`). A refused key is not a move.
- **Pick**: which level a world, tier and seed play (`puzzle_core::levels::pick`): the **built-in** level for that seed if `levels/page.json` has it (the first 25 per tier in every world, made offline by the same generator and **tidied**), otherwise a generated one, **sealed**; a seed that makes no level hands over to the next.
- **Tidy**: take out of a generated level every wall cell, tile and block whose removal keeps it what its world and tier want with as long a shortest solution, then **seal** it (`puzzle_core::tidy`). Seconds per tier-4 level, so only the built-in levels get it.
- **Seal**: fill every cell the player can never reach with wall (`puzzle_core::seal`); the level plays exactly as before. The page, the MCP server and the bench all pick through it.
- **Play**: a level being played, holding every state since the start. Its **moves** are the ones on the board, so **undo** and **restart** cost nothing. `apply` plays a **move string** until it runs out, solves the level or reaches a refusal; a **Stop** says where and why it ended early.
- **Motion**: every timeline of a level on the page, in `crates/puzzle-wasm/src/motion.rs`: the intro, a key's **tween** (or, for a move that plays out over several rounds, its **trace** of the rules' rounds), the win, the lean into a turn and the refusal flash. It says where the climber and each moving crate are now, the climber's pose, and which **events** (landing, dust, exit, crumble, break) fall due; `Game` sets off their effects and draws. Reduced motion is decided there once: every answer is the end state.
