//! Springs (`^`, M4 #121; P30-P33): a spring is a floor tile. Arriving on it, by walking or falling, throws you up as
//! high as you fell (at least 1), stopping at a ceiling, then a cell forward if that is clear, then you fall; landing
//! on another spring bounces again. Blocks bounce too, forward the way you faced when you set them down. A bounce whose
//! forward cell is filled comes back down onto its spring and stays.

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
fn springs_read_and_write_as_carets_and_hold_you_up() {
    let text = "#>.^E#\n######";
    assert_eq!(level(text).to_string(), grid(text));
    assert!(level("#>..E#\n##^###").springs.get(6 + 2));
}

#[test]
fn walking_onto_a_spring_throws_you_up_one_and_over_a_low_wall() {
    let before = "#.....E#\n#......#\n#.>.#..#\n###^####";
    assert_eq!(after(before, "R"), grid("#.....E#\n#...>..#\n#...#..#\n###^####"));
}

#[test]
fn falling_onto_a_spring_throws_you_as_high_as_you_fell() {
    let before = "#>.....#\n##.#...#\n#..#..E#\n##^#####";
    assert_eq!(after(before, "R"), grid("#..>...#\n##.#...#\n#..#..E#\n##^#####"));
}

#[test]
fn a_ceiling_cuts_a_bounce_short() {
    // A walk-on throws you 1, but a wall sits right over the spring: no rise, just the hop forward.
    assert_eq!(after("#####E#\n#>....#\n##^####", "R"), grid("#####E#\n#..>..#\n##^####"));
}

#[test]
fn a_bounce_with_its_way_forward_filled_comes_back_down_and_stays() {
    assert_eq!(after("#..#E#\n#>.#.#\n##^###", "R"), grid("#..#E#\n#.>#.#\n##^###"));
}

#[test]
fn landing_on_another_spring_bounces_again() {
    assert_eq!(after("#.....E#\n#......#\n#>.....#\n##^^####", "R"), grid("#.....E#\n#......#\n#...>..#\n##^^####"));
}

#[test]
fn a_block_set_down_onto_a_spring_bounces_forward() {
    assert_eq!(after("#B...E#\n#>....#\n##^####", "D"), grid("#....E#\n#>.B..#\n##^####"));
}

#[test]
fn turning_on_a_spring_does_not_bounce() {
    assert_eq!(after("#.>..E#\n##^####", "L"), grid("#.<..E#\n##^####"));
}

#[test]
fn undo_takes_a_bounce_back() {
    let start = grid("#>..#.E#\n##^#####");
    let mut p = Play::new(level(&start));
    p.apply("R");
    assert_eq!(p.show(), grid("#..>#.E#\n##^#####"), "the top edge is a ceiling too");
    p.undo(1);
    assert_eq!(p.show(), start);
}

#[test]
fn the_solver_finds_a_shortest_way_that_needs_a_spring() {
    // The exit is up a 2-high wall; a 2-cell drop onto the spring throws you over it.
    let l = level("#>.......#\n##.......#\n#..#.....#\n#..#.....#\n##^#....E#\n##########");
    let found = solve(&l).solution.expect("solvable with the spring");
    assert!(Play::new(l.clone()).apply(&found.iter().map(|k| format!("{k:?}")).collect::<String>()).solved);
}

#[test]
fn random_play_with_springs_never_leaves_anything_floating() {
    let mut rng = Rng(0x0005_9c1d_7e57_ab1e);
    for _ in 0..500 {
        let text: String = random_level(&mut rng)
            .to_string()
            .chars()
            .map(|c| if c == '#' && rng.below(4) == 0 { '^' } else { c })
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
