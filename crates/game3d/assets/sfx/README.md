# Sound effects

Embedded into the game by `crates/game3d/src/sound.rs` (decision Q79). The recordings are David's picks, by ear, from
the CC0 packs below. All are 16-bit mono 44.1 kHz WAVs, trimmed and peak-normalised to -1 dBFS.

| Files | What | Source |
|---|---|---|
| `hit_0..2` | A light blow (punch, jab, slash) | 37 hits/punches: `hit11`, `hit12`, `hit15` |
| `heavy_0..2` | A heavy blow (kick, drop-kick, smash, a throw's hit) | 37 hits/punches: `hit21`, `hit19`, `hit35` |
| `crack_0..1` | A figure breaking apart | 75 breaking/falling/hit: `bfh1_breaking_01`, `bfh1_wood_breaking_03` |
| `clack_0..3` | Debris landing | 75 breaking/falling/hit: `bfh1_hit_01`, `bfh1_wood_falling_02`, `bfh1_wood_hit_02`, `bfh1_wood_hit_03` |
| `rubble` | Under a bomb's blast | 75 breaking/falling/hit: `bfh1_falling_01` |
| `door` | A door's warning: knock, knock | Kenney Impact Sounds: `impactWood_medium_001` + `003` |
| `whoosh` | Something flying (a burst, a throw) | Synthesised: `tools/sfx/generate.py` |
| `boom` | A bomb's blast | Synthesised: `tools/sfx/generate.py` |

## Sources (all CC0 1.0, no credit required; credited with thanks)

- **37 hits/punches** by Independent.nu (ljudbank), submitted by qubodup:
  <https://opengameart.org/content/37-hitspunches> (`independent_nu_ljudbank-hits_and_punches.7z`).
- **75 CC0 breaking / falling / hit sfx** by rubberduck: <https://opengameart.org/comment/84931>
  (`sfx_breaking_and_falling.zip`).
- **Impact Sounds** (v1.0) by Kenney: <https://kenney.nl/assets/impact-sounds> (`kenney_impact-sounds.zip`).

License: [CC0 1.0 Universal](https://creativecommons.org/publicdomain/zero/1.0/).

## Rebuilding

Unzip the packs into `punches/`, `breaking/` and `kenney/` under one folder (Windows' own `tar` opens the `.7z`), then
`bash tools/sfx/import.sh <folder>`; `python tools/sfx/generate.py` regenerates the synthesised two.
