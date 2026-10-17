//! The 3D game's colours: Rumble Box's own (decision Q75, read off its gameplay video,
//! `docs/research/rumble-box-reference.md`). The fighters wear the same colours in every box: the player an orange
//! head, a blue body and red fists; the fodder magenta with a peach head and brown legs; the thugs (Rumble Box's
//! heavies) dark grey with red fists. Each box is a [`Stage`]: pale green tiles under a sky-blue background, then an
//! orange checker, a grey one, a dark blue one, round again. Debris is a pale wash of its part's colour toward the
//! box's floor, so the dead read apart from the living.
//!
//! Colours are sRGB triples, as a face lit from straight above shows them; the game turns them into materials.

use crate::enemy::Kind;

/// An sRGB colour, 0..=1 per channel.
pub type Rgb = [f32; 3];

/// Who wears a figure's colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Fighter {
    Player,
    Enemy(Kind),
}

/// A live fighter's colours: its body (torso and upper arms), its ball head, its fists (or forearms), and its trim
/// (legs, and the player's visor).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Figure {
    pub body: Rgb,
    pub head: Rgb,
    pub fist: Rgb,
    pub trim: Rgb,
}

/// One box's colours.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stage {
    pub key: &'static str,
    /// Behind the box.
    pub sky: Rgb,
    /// The floor's and walls' tiles, alternating; the same twice for plain tiles.
    pub tiles: [Rgb; 2],
    /// The thin lines between tiles (the rim's thick edges are black in every box).
    pub line: Rgb,
}

const ORANGE: Rgb = [0.97, 0.65, 0.3];
const BLUE: Rgb = [0.1, 0.36, 0.8];
const RED: Rgb = [0.84, 0.24, 0.2];
const MAGENTA: Rgb = [0.85, 0.33, 0.85];
const PEACH: Rgb = [0.98, 0.79, 0.57];
const BROWN: Rgb = [0.42, 0.21, 0.04];
const GREY: Rgb = [0.27, 0.27, 0.3];

/// The boxes' looks, in order; box `n` takes `STAGES[(n - 1) % len]`.
pub const STAGES: [Stage; 4] = [
    // Box 1: pale green tiles, dark green lines, a sky-blue background.
    Stage { key: "green", sky: [0.36, 0.76, 0.91], tiles: [[0.59, 0.84, 0.55]; 2], line: [0.08, 0.24, 0.04] },
    // Box 2 ("Stage Two"): an orange checker on orange.
    Stage {
        key: "orange",
        sky: [1.0, 0.62, 0.25],
        tiles: [[1.0, 0.72, 0.43], [0.9, 0.53, 0.26]],
        line: [0.3, 0.14, 0.03],
    },
    // The "Frenzy" bonus stage's grey checker.
    Stage {
        key: "grey",
        sky: [0.45, 0.45, 0.48],
        tiles: [[0.93, 0.93, 0.93], [0.7, 0.7, 0.72]],
        line: [0.15, 0.15, 0.17],
    },
    // The dark blue arena after box 2, on black.
    Stage {
        key: "night",
        sky: [0.0, 0.0, 0.02],
        tiles: [[0.08, 0.14, 0.5], [0.05, 0.09, 0.36]],
        line: [0.0, 0.0, 0.0],
    },
];

/// Box `n`'s stage (`n` counts from 0).
pub fn stage(n: usize) -> &'static Stage {
    &STAGES[n % STAGES.len()]
}

/// A live fighter's colours. Kinds also differ in shape (Q35).
pub fn fighter(who: Fighter) -> Figure {
    match who {
        Fighter::Player => Figure { body: BLUE, head: ORANGE, fist: RED, trim: [0.16, 0.16, 0.2] },
        Fighter::Enemy(Kind::Fodder) => Figure { body: MAGENTA, head: PEACH, fist: BLUE, trim: BROWN },
        Fighter::Enemy(Kind::Thug) => {
            Figure { body: GREY, head: [0.36, 0.36, 0.4], fist: RED, trim: [0.12, 0.12, 0.14] }
        }
        // Rumble Box's blue enemies.
        Fighter::Enemy(Kind::Ninja) => Figure { body: BLUE, head: PEACH, fist: BLUE, trim: BROWN },
    }
}

/// `a` with a fraction `k` of `b` mixed in.
fn mix(a: Rgb, b: Rgb, k: f32) -> Rgb {
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * k)
}

/// A debris piece that was `part` coloured, in `stage`: washed toward the floor and paled, as Rumble Box's heaps
/// are (mint and pale peach on box 1's green, near-white on the grey checker).
pub fn debris(stage: &Stage, part: Rgb) -> Rgb {
    mix(mix(part, stage.tiles[0], 0.4), [1.0; 3], 0.25)
}

/// A closed door: the wall's tile, darkened, so the doors read as openings.
pub fn door(stage: &Stage) -> Rgb {
    stage.tiles[1].map(|c| c * 0.5)
}

/// Door warnings, impact bursts, score popups, bomb fuses: Rumble Box's red (its combo stars and score).
pub const SIGNAL: Rgb = [1.0, 0.18, 0.1];
/// The player's floor ring (Q60): cyan, which reads on every box's floor.
pub const RING: Rgb = [0.1, 0.95, 1.0];
/// The health bar: Rumble Box's green health pips.
pub const HEALTH: Rgb = [0.12, 0.78, 0.12];

#[cfg(test)]
mod tests {
    use super::*;

    const FIGHTERS: [Fighter; 4] =
        [Fighter::Player, Fighter::Enemy(Kind::Fodder), Fighter::Enemy(Kind::Thug), Fighter::Enemy(Kind::Ninja)];

    /// Colourfulness: the spread between the strongest and weakest channel.
    fn chroma(c: Rgb) -> f32 {
        c.iter().cloned().fold(f32::MIN, f32::max) - c.iter().cloned().fold(f32::MAX, f32::min)
    }

    /// Brightness: the strongest channel (HSV value).
    fn value(c: Rgb) -> f32 {
        c.iter().cloned().fold(f32::MIN, f32::max)
    }

    fn distance(a: Rgb, b: Rgb) -> f32 {
        a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum::<f32>().sqrt()
    }

    fn parts(f: Figure) -> [Rgb; 4] {
        [f.body, f.head, f.fist, f.trim]
    }

    #[test]
    fn the_player_reads_by_contrast_between_its_parts() {
        let p = fighter(Fighter::Player);
        for (i, a) in parts(p).into_iter().enumerate() {
            for b in parts(p).into_iter().skip(i + 1) {
                assert!(distance(a, b) > 0.4, "{a:?} vs {b:?}");
            }
        }
    }

    #[test]
    fn the_kinds_read_apart_from_the_player_and_each_other() {
        for (i, &a) in FIGHTERS.iter().enumerate() {
            for &b in &FIGHTERS[i + 1..] {
                let d: f32 = parts(fighter(a)).iter().zip(parts(fighter(b))).map(|(x, y)| distance(*x, y)).sum();
                assert!(d > 0.6, "{a:?} vs {b:?}: {d}");
            }
        }
    }

    #[test]
    fn debris_is_paler_than_the_living_and_still_reads_against_the_floor() {
        for s in &STAGES {
            for f in FIGHTERS {
                for part in parts(fighter(f)) {
                    let d = debris(s, part);
                    assert!(chroma(d) < chroma(part) || chroma(part) < 0.1, "{}: {f:?} {part:?}", s.key);
                    assert!(distance(d, s.tiles[0]) > 0.08, "{}: {f:?} {part:?} vanishes into the floor", s.key);
                }
            }
        }
    }

    #[test]
    fn doors_lines_and_the_ring_read_against_each_box() {
        for s in &STAGES {
            for tile in s.tiles {
                assert!(value(tile) - value(door(s)) > 0.15 || s.key == "night", "{}: door", s.key);
                assert!(distance(tile, s.line) > 0.3, "{}: tile lines", s.key);
                assert!(distance(tile, RING) > 0.4, "{}: the ring", s.key);
            }
        }
    }

    #[test]
    fn the_boxes_cycle_through_the_stages() {
        assert_eq!(stage(0).key, "green");
        assert_eq!(stage(1).key, "orange");
        assert_eq!(stage(STAGES.len()).key, "green");
    }
}
