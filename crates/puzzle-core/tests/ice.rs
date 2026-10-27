//! Ice crates (`I`, M3 #120; P34-P37): set down, an ice crate slides away from you until it hits something, and keeps
//! sliding after a ledge. Anything that falls onto resting ice knocks it sliding the way the lander was going, and the
//! lander drops into the gap. Sliding ice that hits resting ice stops and passes its motion on, and what sits on the
//! ice it hit rides along.

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
fn ice_reads_and_writes_as_i_carried_and_on_the_exit() {
    for text in ["#.....E#\n#>.I...#\n########", "#I....E#\n#>.....#\n########", "#>.i#\n#####"] {
        assert_eq!(level(text).to_string(), grid(text));
    }
    let carried = level("#I....E#\n#>.....#\n########");
    assert!(carried.start.carrying && carried.start.carrying_ice);
    assert!(level("#>.I.E#\n#######").start.ice.get(3));
}

#[test]
fn ice_set_down_slides_away_until_it_hits_something() {
    assert_eq!(after("#I....E#\n#>.....#\n########", "D"), grid("#.....E#\n#>....I#\n########"));
}

#[test]
fn a_plain_crate_set_down_stays_put() {
    assert_eq!(after("#B....E#\n#>.....#\n########", "D"), grid("#.....E#\n#>B....#\n########"));
}

#[test]
fn sliding_ice_falls_off_a_ledge_and_keeps_sliding() {
    let before = "#I.......E#\n#>........#\n###.#######\n#.........#\n###########";
    let want = "#........E#\n#>........#\n###.#######\n#........I#\n###########";
    assert_eq!(after(before, "D"), grid(want));
}

#[test]
fn falling_onto_resting_ice_knocks_it_away_and_you_drop_into_the_gap() {
    assert_eq!(after("#>....E#\n##.....#\n##I....#\n########", "R"), grid("#.....E#\n##.....#\n##>...I#\n########"));
}

#[test]
fn a_crate_set_down_onto_resting_ice_knocks_it_the_way_you_face() {
    assert_eq!(after("#B....E#\n#>.....#\n##I....#\n########", "D"), grid("#.....E#\n#>.....#\n##B...I#\n########"));
}

#[test]
fn walking_onto_ice_at_the_same_level_does_not_knock_it() {
    assert_eq!(after("#.....E#\n#>.....#\n##I....#\n########", "R"), grid("#.....E#\n#.>....#\n##I....#\n########"));
}

#[test]
fn sliding_ice_that_hits_resting_ice_passes_its_motion_on() {
    assert_eq!(after("#I....E#\n#>..I..#\n########", "D"), grid("#.....E#\n#>.I..I#\n########"));
}

#[test]
fn what_sits_on_ice_that_is_passed_motion_rides_along() {
    let before = "#I..B....#\n#>..I....#\n#########E";
    assert_eq!(after(before, "D"), grid("#.......B#\n#>.I....I#\n#########E"));
}

#[test]
fn ice_can_be_lifted_and_set_down_again() {
    let start = "#.....E#\n#>I....#\n########";
    assert_eq!(after(start, "D"), grid("#I....E#\n#>.....#\n########"));
    assert_eq!(after(start, "DD"), grid("#.....E#\n#>....I#\n########"));
}

#[test]
fn undo_puts_slid_ice_back() {
    let start = grid("#I....E#\n#>.....#\n########");
    let mut p = Play::new(level(&start));
    p.apply("D");
    p.undo(1);
    assert_eq!(p.show(), start);
}

#[test]
fn random_play_with_ice_never_leaves_anything_floating() {
    let mut rng = Rng(0x0001_ce1c_e0ff_5eed);
    for _ in 0..500 {
        // Make some of a random level's crates ice.
        let text: String = random_level(&mut rng)
            .to_string()
            .chars()
            .map(|c| if c == 'B' && rng.below(2) == 0 { 'I' } else { c })
            .collect();
        let Ok(l) = Level::parse(&text) else { continue };
        let mut s = l.start;
        for _ in 0..200 {
            let k = [Key::L, Key::R, Key::U, Key::D][rng.below(4)];
            if let Ok(n) = step(&l, &s, k) {
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
