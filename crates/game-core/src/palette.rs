//! The 3D game's colour rules (decision Q50): in each of the site's schemes ([`crate::scheme`]), the arena and the
//! debris take a muted tone and live fighters its vivid accents, so the dead read apart from the living (as in Rumble
//! Box, research §1). The player's ring is the main accent (Q60).
//!
//! Colours are sRGB triples; the game turns them into materials. A muted tone is a grey with a little of the main
//! accent mixed in, so each scheme's arena keeps its hue without competing with the fighters.

use crate::enemy::Kind;
use crate::scheme::Scheme;

/// An sRGB colour, 0..=1 per channel.
pub type Rgb = [f32; 3];

/// Who wears a figure's colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fighter {
    Player,
    Enemy(Kind),
}

/// A live fighter's colours: a vivid body and a dark trim (the visor and other marks).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Figure {
    pub body: Rgb,
    pub trim: Rgb,
}

/// The box's colours.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arena {
    pub floor: Rgb,
    pub wall: Rgb,
    /// The bright line along the rim.
    pub edge: Rgb,
    /// The glass front wall's rails and posts: dimmer than the rim, so the pane reads as glass.
    pub glass: Rgb,
}

/// `grey` with a fraction `k` of `accent` mixed in.
fn tint(grey: f32, accent: Rgb, k: f32) -> Rgb {
    accent.map(|c| grey + (c - grey) * k)
}

/// How much of the main accent the muted tones carry.
const MUTE: f32 = 0.16;

/// The arena in `scheme`'s muted tone: a mid floor, lighter walls (so the walls' faces read against the floor), a
/// near-white rim and dimmer glass rails.
pub fn arena_tone(scheme: &Scheme) -> Arena {
    let a = scheme.accents[0];
    Arena {
        floor: tint(0.46, a, MUTE),
        wall: tint(0.6, a, MUTE),
        edge: tint(0.86, a, 0.12),
        glass: tint(0.66, a, 0.12),
    }
}

/// Shades a heap cycles through, so neighbouring pieces read apart under the outlines.
const DEBRIS_SHADES: [f32; 3] = [0.62, 0.68, 0.74];

/// A debris piece in `scheme`'s muted tone; `piece` (any number, e.g. the part's index) picks one of a few shades.
/// Lighter than the floor, so a heap reads against it.
pub fn debris_tone(scheme: &Scheme, piece: usize) -> Rgb {
    tint(DEBRIS_SHADES[piece % DEBRIS_SHADES.len()], scheme.accents[0], MUTE)
}

/// A live fighter's colours: the player wears the main accent, and each enemy kind one of the others (as in the 2.5D
/// game). Colour is a bonus; the kinds also differ in shape (Q35).
pub fn fighter_colour(scheme: &Scheme, who: Fighter) -> Figure {
    let body = scheme.accents[match who {
        Fighter::Player => 0,
        Fighter::Enemy(Kind::Fodder) => 2,
        Fighter::Enemy(Kind::Thug) => 1,
        Fighter::Enemy(Kind::Ninja) => 3,
    }];
    Figure { body, trim: body.map(|c| c * 0.22) }
}

/// The player's floor ring (Q60): the main accent.
pub fn ring(scheme: &Scheme) -> Rgb {
    scheme.accents[0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheme::SCHEMES;

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

    /// Every muted colour in a scheme: the arena's and a heap's worth of debris.
    fn muted(s: &Scheme) -> Vec<Rgb> {
        let a = arena_tone(s);
        let mut out = vec![a.floor, a.wall, a.edge, a.glass];
        out.extend((0..DEBRIS_SHADES.len()).map(|i| debris_tone(s, i)));
        out
    }

    #[test]
    fn live_fighters_are_more_colourful_and_brighter_than_the_arena_and_debris() {
        for s in &SCHEMES {
            let dullest = FIGHTERS.map(|f| chroma(fighter_colour(s, f).body)).into_iter().fold(f32::MAX, f32::min);
            let dimmest = FIGHTERS.map(|f| value(fighter_colour(s, f).body)).into_iter().fold(f32::MAX, f32::min);
            for m in muted(s) {
                assert!(dullest > 2.0 * chroma(m), "{}: fighter chroma {dullest} vs muted {m:?}", s.key);
                assert!(dimmest > value(m) + 0.05, "{}: fighter value {dimmest} vs muted {m:?}", s.key);
            }
        }
    }

    #[test]
    fn the_muted_tones_keep_the_schemes_hue() {
        for s in &SCHEMES {
            let floor = arena_tone(s).floor;
            assert!(chroma(floor) > 0.08 && chroma(floor) < 0.2, "{}: tinted but muted: {floor:?}", s.key);
            let [ar, ag, ab] = s.accents[0];
            let [r, g, b] = floor;
            // The floor's strongest channel is the accent's.
            let strongest = |x: f32, y: f32, z: f32| {
                if x >= y && x >= z {
                    0
                } else if y >= z {
                    1
                } else {
                    2
                }
            };
            assert_eq!(strongest(r, g, b), strongest(ar, ag, ab), "{}", s.key);
        }
    }

    #[test]
    fn debris_reads_against_the_floor_and_neighbouring_pieces_differ() {
        for s in &SCHEMES {
            let floor = arena_tone(s).floor;
            for i in 0..DEBRIS_SHADES.len() {
                assert!(value(debris_tone(s, i)) > value(floor) + 0.1, "{}: piece {i}", s.key);
                assert_ne!(debris_tone(s, i), debris_tone(s, i + 1), "{}: piece {i}", s.key);
            }
        }
    }

    #[test]
    fn enemy_kinds_wear_distinct_colours() {
        let kinds = [Kind::Fodder, Kind::Thug, Kind::Ninja];
        for s in &SCHEMES {
            for (i, &a) in kinds.iter().enumerate() {
                for &b in &kinds[i + 1..] {
                    let d =
                        distance(fighter_colour(s, Fighter::Enemy(a)).body, fighter_colour(s, Fighter::Enemy(b)).body);
                    assert!(d > 0.3, "{}: {a:?} vs {b:?}: {d}", s.key);
                }
                let d = distance(fighter_colour(s, Fighter::Enemy(a)).body, fighter_colour(s, Fighter::Player).body);
                assert!(d > 0.2, "{}: {a:?} vs the player: {d}", s.key);
            }
        }
    }

    #[test]
    fn the_trim_stands_out_on_the_body_and_the_ring_is_the_main_accent() {
        for s in &SCHEMES {
            for f in FIGHTERS {
                let Figure { body, trim } = fighter_colour(s, f);
                assert!(value(body) - value(trim) > 0.5, "{}: {f:?}", s.key);
            }
            assert_eq!(ring(s), s.accents[0]);
        }
    }
}
