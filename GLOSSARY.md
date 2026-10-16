# Glossary

The game's domain words, as the code uses them. Decisions live in `docs/planning/game-decisions.md`; this file names things.

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
- **Armour**: a kind's resistance to being staggered. A thug's armour shrugs off punches but not kicks, throws or blasts.
