//! Cracked floor (`~`, M2 #119): a cracked tile is solid until whatever stands directly on it leaves, crates included,
//! and then it is gone for good (P28, P29).

mod common;

use common::*;
use puzzle_core::*;

fn after(before: &str, ks: &str) -> String {
    let mut p = Play::new(level(before));
    let a = p.apply(ks);
    assert_eq!(a.stop, None, "{ks} in\n{}", grid(before));
    p.show()
}

#[test]
fn cracked_tiles_read_and_write_as_tildes_and_hold_you_up() {
    let text = grid(
        "
        #>..E#
        #~~###",
    );
    let l = level(&text);
    assert_eq!(l.to_string(), text);
    assert!(l.start.cracked.get(6 + 1));
    assert_eq!(after(&text, "R"), grid("#.>.E#\n#.~###"));
}

#[test]
fn walking_off_a_cracked_tile_breaks_it() {
    assert_eq!(after("#>..E#\n#~####", "R"), grid("#.>.E#\n#.####"));
}

#[test]
fn turning_on_a_cracked_tile_does_not_break_it() {
    assert_eq!(after("#.>.E#\n#.~###", "L"), grid("#.<.E#\n#.~###"));
}

#[test]
fn climbing_off_a_cracked_tile_breaks_it() {
    assert_eq!(after("#....E#\n#>#####\n#~#####", "U"), grid("#.>..E#\n#.#####\n#.#####"));
}

#[test]
fn lifting_a_crate_off_a_cracked_tile_breaks_it() {
    assert_eq!(after("#....#\n#>B.E#\n##~###", "D"), grid("#B...#\n#>..E#\n##.###"));
}

#[test]
fn a_crate_set_down_on_a_cracked_tile_breaks_it_when_lifted_again() {
    let start = "#B...#\n#>..E#\n##~###";
    assert_eq!(after(start, "D"), grid("#....#\n#>B.E#\n##~###"));
    assert_eq!(after(start, "DD"), grid("#B...#\n#>..E#\n##.###"));
}

#[test]
fn standing_on_a_crate_on_a_cracked_tile_does_not_break_it() {
    // Only what stands directly on the tile counts: here that is the crate, and it stays.
    assert_eq!(after("#>...#\n#B..E#\n#~####", "R"), grid("#....#\n#B>.E#\n#~####"));
}

#[test]
fn a_broken_tile_leaves_a_hole_you_fall_into() {
    assert_eq!(after("#>..#\n#~.E#\n#####", "RLL"), grid("#...#\n#<.E#\n#####"));
}

#[test]
fn undo_restores_a_broken_tile() {
    let start = grid("#>..E#\n#~####");
    let mut p = Play::new(level(&start));
    p.apply("R");
    p.undo(1);
    assert_eq!(p.show(), start);
}

#[test]
fn random_play_with_cracked_tiles_never_leaves_anything_floating() {
    let mut rng = Rng(0x000c_4ac7_ed0f_1007);
    for _ in 0..500 {
        // Crack some of a random level's walls.
        let text: String = random_level(&mut rng)
            .to_string()
            .chars()
            .map(|c| if c == '#' && rng.below(3) == 0 { '~' } else { c })
            .collect();
        let Ok(l) = Level::parse(&text) else { continue };
        let mut s = l.start;
        for _ in 0..200 {
            let k = [Key::L, Key::R, Key::U, Key::D][rng.below(4)];
            if let Ok(n) = step(&l, &s, k) {
                assert!((0..MAX_W * MAX_H).all(|i| !n.cracked.get(i) || s.cracked.get(i)), "a tile came back");
                s = n;
            }
            if l.solved(&s) {
                break;
            }
            let shown = l.show(&s);
            let back = Level::parse(&shown).unwrap_or_else(|e| panic!("{e:?} after {k:?} in\n{shown}"));
            assert_eq!(back.start, s, "in\n{shown}");
        }
    }
}
