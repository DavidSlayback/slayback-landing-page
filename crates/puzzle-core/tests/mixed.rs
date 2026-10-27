//! The mixed world (M5 #122): cracked floor, ice and springs in one level, their rules combined. Ice that reaches a
//! spring bounces its own way and keeps sliding; anything that passes over a cracked tile mid-move breaks it when it
//! leaves; a bounced block that lands on ice knocks it.

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
fn ice_that_slides_onto_a_spring_is_thrown_forward_and_keeps_sliding() {
    assert_eq!(after("#I......E#\n#>.......#\n###^######", "D"), grid("#.......E#\n#>......I#\n###^######"));
}

#[test]
fn ice_sliding_off_a_cracked_tile_breaks_it_on_the_way() {
    assert_eq!(after("#I....E#\n#>.....#\n##~#####", "D"), grid("#.....E#\n#>....I#\n##.#####"));
}

#[test]
fn a_block_thrown_by_a_spring_that_lands_on_ice_knocks_it() {
    assert_eq!(
        after("#B.....E#\n#>......#\n##^I....#\n#########", "D"),
        grid("#......E#\n#>......#\n##^B...I#\n#########")
    );
}

#[test]
fn random_play_with_every_mechanic_never_leaves_anything_floating() {
    let mut rng = Rng(0x0000_51ec_ed0b_0b00);
    for _ in 0..500 {
        let text: String = random_level(&mut rng)
            .to_string()
            .chars()
            .map(|c| match (c, rng.below(6)) {
                ('#', 0) => '~',
                ('#', 1) => '^',
                ('B', 0..=2) => 'I',
                (c, _) => c,
            })
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
