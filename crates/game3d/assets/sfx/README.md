# Sound effects

Embedded into the game by `crates/game3d/src/sound.rs` (decision Q79: wooden toy blocks). All are 16-bit mono 44.1 kHz WAVs.

| Files | What | Source |
|---|---|---|
| `hit_0..2` | A light blow (punch, jab, slash) | Kenney, Impact Sounds: `impactWood_medium_000/002/004` |
| `heavy_0..2` | A heavy blow (kick, drop-kick, smash, a throw's hit) | `impactWood_heavy_000/002/004` |
| `clack_0..3` | Debris landing | `impactWood_light_000/002/004`, `impactGeneric_light_001` |
| `crack` | A figure breaking apart | `impactPlank_medium_001` |
| `door` | A door's warning: knock, knock | `impactWood_medium_001` + `003` |
| `whoosh` | Something flying (a burst, a throw) | Synthesised: `tools/sfx/generate.py` |
| `boom` | A bomb's blast | Synthesised: `tools/sfx/generate.py` |

The recordings are from [Kenney's "Impact Sounds"](https://kenney.nl/assets/impact-sounds) (v1.0, 2019), licensed
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/): free to use, credit not required. Thanks, Kenney.
`bash tools/sfx/import.sh <unzipped pack dir>` re-imports them (trimmed, mono, 44.1 kHz, faded out at the end), and
`python tools/sfx/generate.py` regenerates the synthesised two.
