//! The page's built-in levels (`levels/page.json`): each one is what it says, the bench's suites tidy into its first
//! levels, and its levels are what generating and tidying them gives.

use puzzle_core::levels::{self, Entry};
use puzzle_core::*;

fn suite(file: &str) -> Vec<Entry> {
    let path = format!("{}/../../bench/puzzle/{file}", env!("CARGO_MANIFEST_DIR"));
    levels::read_suite(&std::fs::read_to_string(&path).expect(&path))
}

#[test]
fn every_world_and_tier_has_25_built_in_levels() {
    let page = levels::page();
    for world in World::ALL {
        for tier in 1..=4 {
            assert_eq!(page.iter().filter(|l| l.world == world && l.tier == tier).count(), 25, "{world:?} tier {tier}");
        }
    }
}

#[test]
fn every_built_in_level_is_solved_by_its_solution_in_its_moves() {
    for l in levels::page() {
        let what = format!("{:?} tier {} seed {}", l.world, l.tier, l.seed);
        assert_eq!(l.level.fingerprint(), l.fingerprint, "{what}");
        assert_eq!(l.solution.len(), l.moves, "{what}");
        let keys: String = l.solution.iter().map(|k| format!("{k:?}")).collect();
        let mut play = Play::new(l.level);
        assert_eq!(play.apply(&keys).stop, None, "{what}");
        assert!(play.solved(), "{what}");
    }
}

/// While the page and the bench share a generator, tidying a suite level gives the built-in level for its seed. Tiers
/// 1 and 2 take seconds; 3 and 4 take over an hour in release, in the ignored test below.
#[test]
fn tidying_a_suite_level_gives_the_built_in_level_tiers_1_and_2() {
    tidies_to_built_in(&[1, 2]);
}

/// `bash scripts/native.sh test -p puzzle-core --release -- --ignored`
#[test]
#[ignore]
fn tidying_a_suite_level_gives_the_built_in_level_tiers_3_and_4() {
    tidies_to_built_in(&[3, 4]);
}

fn tidies_to_built_in(tiers: &[u8]) {
    let page = levels::page();
    for l in suite("suite-v1.json").into_iter().chain(suite("suite-v2.json")).filter(|l| tiers.contains(&l.tier)) {
        let what = format!("{:?} tier {} seed {}", l.world, l.tier, l.seed);
        let built_in = page.iter().find(|p| (p.world, p.tier, p.seed) == (l.world, l.tier, l.seed)).expect(&what);
        let tidied = tidy(&l.level, l.solution, l.world, &TIERS[l.tier as usize - 1]);
        assert_eq!(tidied, (built_in.level.clone(), built_in.solution.clone()), "{what}");
    }
}

/// A built-in level is exactly what generating and tidying it gives, so the page shows the same level for a seed with
/// or without the built-in set. Tier 1 takes a second; the rest take over an hour in release, in the ignored test below.
#[test]
fn built_in_levels_regenerate_tier_1() {
    regenerates(&[1]);
}

/// `bash scripts/native.sh test -p puzzle-core --release -- --ignored`
#[test]
#[ignore]
fn built_in_levels_regenerate_tiers_2_to_4() {
    regenerates(&[2, 3, 4]);
}

fn regenerates(tiers: &[u8]) {
    for l in levels::page().into_iter().filter(|l| tiers.contains(&l.tier)) {
        let tier = &TIERS[l.tier as usize - 1];
        let g = generate(l.seed, l.world, tier).unwrap();
        let tidied = tidy(&g.level, g.solution, l.world, tier);
        assert_eq!(tidied, (l.level, l.solution), "{:?} tier {} seed {}", l.world, l.tier, l.seed);
    }
}

#[test]
fn past_the_built_in_levels_pick_seals_the_generated_level() {
    let picked = levels::pick(World::Classic, 1, 26).unwrap();
    let g = generate(picked.seed, World::Classic, &TIERS[0]).unwrap();
    assert_eq!((picked.level, picked.solution), (seal(&g.level), g.solution));
}

#[test]
fn pick_gives_the_built_in_level_and_its_seed() {
    let l = &levels::page()[0];
    let picked = levels::pick(l.world, l.tier, l.seed).unwrap();
    assert_eq!((picked.seed, &picked.level, &picked.solution), (l.seed, &l.level, &l.solution));
}
