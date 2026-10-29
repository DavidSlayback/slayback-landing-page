//! Generated levels against what their tier promises (P16): the band, the grid, a needed block, the same level from
//! the same seed.

use puzzle_core::*;

fn keys(ks: &[Key]) -> String {
    ks.iter().map(|k| format!("{k:?}")).collect()
}

/// Checks one generated level against everything its tier promises.
fn check(seed: u64, tier: &Tier) -> Generated {
    let g = generate(seed, World::Classic, tier).unwrap_or_else(|| panic!("tier {} seed {seed}: no level", tier.n));
    let (l, len) = (&g.level, g.solution.len());
    let at = format!("tier {} seed {seed}, {len} moves, in\n{l}", tier.n);
    assert!((tier.moves.0..=tier.moves.1).contains(&len), "outside the band: {at}");
    assert!(l.w <= tier.w && l.h <= tier.h, "too big: {at}");
    assert_eq!(solve(l).solution.map(|s| s.len()), Some(len), "not a shortest solution: {at}");
    assert!(Play::new(l.clone()).apply(&keys(&g.solution)).solved, "doesn't reach the exit: {at}");
    if tier.n >= 2 {
        // With every block turned to wall nothing can be lifted; the level must then be impossible.
        let text = l.to_string();
        assert!(!text.contains('b'), "a block starts on the exit: {at}");
        let fixed = Level::parse(&text.replace('B', "#")).unwrap();
        assert_eq!(solve(&fixed).solution, None, "solvable without lifting a block: {at}");
    }
    g
}

#[test]
fn tier_bands_follow_the_decision_log() {
    let bands: Vec<_> = TIERS.iter().map(|t| (t.n, t.moves, t.w, t.h)).collect();
    assert_eq!(bands, [(1, (8, 15), 8, 6), (2, (16, 40), 12, 8), (3, (41, 80), 16, 10), (4, (81, 150), 20, 12)]);
}

#[test]
fn tier_1_and_2_levels_keep_their_promises() {
    for tier in &TIERS[..2] {
        for seed in 1..=5 {
            check(seed, tier);
        }
    }
}

#[test]
fn tier_3_levels_keep_their_promises() {
    for seed in 1..=2 {
        check(seed, &TIERS[2]);
    }
}

#[test]
fn a_fingerprint_is_fnv1a_of_the_text_form() {
    // Expected value computed outside Rust (Python, FNV-1a 64 over b"#>E#\n####").
    assert_eq!(Level::parse("#>E#\n####").unwrap().fingerprint(), 0xa68c_59b5_6714_9aea);
}

/// Every level of a suite in `bench/puzzle/`.
fn suite(file: &str) -> Vec<levels::Entry> {
    let path = format!("{}/../../bench/puzzle/{file}", env!("CARGO_MANIFEST_DIR"));
    levels::read_suite(&std::fs::read_to_string(&path).expect(&path))
}

fn regenerates(tiers: &[u8]) {
    let levels = suite("suite-v1.json");
    for &t in tiers {
        assert_eq!(levels.iter().filter(|l| l.tier == t).count(), 10, "tier {t} should have 10 levels");
    }
    for l in levels.iter().filter(|l| tiers.contains(&l.tier)) {
        let g = generate(l.seed, World::Classic, &TIERS[l.tier as usize - 1]).unwrap();
        assert_eq!(
            (g.level.fingerprint(), g.solution.len()),
            (l.fingerprint, l.moves),
            "tier {} seed {}",
            l.tier,
            l.seed
        );
    }
}

/// The published suite is the seed-to-level table: a change to the rules or the generator that changes any level
/// fails here, and needs a new suite version rather than a silent edit.
#[test]
fn the_published_suite_still_regenerates() {
    regenerates(&[1, 2, 3]);
}

/// Tier 4 takes about 25 s even in release: `bash scripts/native.sh test -p puzzle-core --release -- --ignored`.
#[test]
#[ignore]
fn the_published_suite_still_regenerates_tier_4() {
    regenerates(&[4]);
}

#[test]
fn the_same_seed_gives_the_same_level() {
    let a = generate(7, World::Classic, &TIERS[1]).unwrap();
    assert_eq!(generate(7, World::Classic, &TIERS[1]), Some(a.clone()));
    assert_ne!(generate(8, World::Classic, &TIERS[1]).map(|g| g.level), Some(a.level), "different seeds should differ");
}

/// A Crumble level keeps its tier's promises, has cracked tiles, and needs them: with every cracked tile made wall
/// the shortest solution changes or disappears (M2).
fn check_crumble(seed: u64, tier: &Tier) {
    let g =
        generate(seed, World::Crumble, tier).unwrap_or_else(|| panic!("crumble tier {} seed {seed}: no level", tier.n));
    let (l, len) = (&g.level, g.solution.len());
    let at = format!("crumble tier {} seed {seed}, {len} moves, in\n{l}", tier.n);
    assert!((tier.moves.0..=tier.moves.1).contains(&len), "outside the band: {at}");
    assert_eq!(solve(l).solution.map(|s| s.len()), Some(len), "not a shortest solution: {at}");
    assert!(Play::new(l.clone()).apply(&keys(&g.solution)).solved, "doesn't reach the exit: {at}");
    let text = l.to_string();
    assert!(text.contains('~'), "no cracked tiles: {at}");
    let walled = Level::parse(&text.replace('~', "#")).unwrap();
    assert_ne!(solve(&walled).solution.map(|s| s.len()), Some(len), "cracked tiles change nothing: {at}");
}

#[test]
fn crumble_levels_keep_their_promises_and_need_their_cracked_tiles() {
    for tier in &TIERS[..2] {
        for seed in 1..=5 {
            check_crumble(seed, tier);
        }
    }
    check_crumble(1, &TIERS[2]);
}

#[test]
fn worlds_give_different_levels_for_the_same_seed() {
    let classic = generate(3, World::Classic, &TIERS[0]).unwrap().level;
    assert_ne!(generate(3, World::Crumble, &TIERS[0]).unwrap().level, classic);
}

/// An Ice level keeps its tier's promises, has ice, and needs it: with every ice block made plain the shortest
/// solution changes or disappears (M3).
fn check_ice(seed: u64, tier: &Tier) {
    let g = generate(seed, World::Ice, tier).unwrap_or_else(|| panic!("ice tier {} seed {seed}: no level", tier.n));
    let (l, len) = (&g.level, g.solution.len());
    let at = format!("ice tier {} seed {seed}, {len} moves, in\n{l}", tier.n);
    assert!((tier.moves.0..=tier.moves.1).contains(&len), "outside the band: {at}");
    assert_eq!(solve(l).solution.map(|s| s.len()), Some(len), "not a shortest solution: {at}");
    assert!(Play::new(l.clone()).apply(&keys(&g.solution)).solved, "doesn't reach the exit: {at}");
    let text = l.to_string();
    assert!(text.contains(['I', 'i']), "no ice: {at}");
    let plain = Level::parse(&text.replace('I', "B").replace('i', "b")).unwrap();
    assert_ne!(solve(&plain).solution.map(|s| s.len()), Some(len), "ice changes nothing: {at}");
}

#[test]
fn ice_levels_keep_their_promises_and_need_their_ice() {
    for tier in &TIERS[..2] {
        for seed in 1..=5 {
            check_ice(seed, tier);
        }
    }
    check_ice(1, &TIERS[2]);
}

/// A Springs level keeps its tier's promises, has springs, and needs them: with every spring made wall the shortest
/// solution changes or disappears (M4).
fn check_springs(seed: u64, tier: &Tier) {
    let g =
        generate(seed, World::Springs, tier).unwrap_or_else(|| panic!("springs tier {} seed {seed}: no level", tier.n));
    let (l, len) = (&g.level, g.solution.len());
    let at = format!("springs tier {} seed {seed}, {len} moves, in\n{l}", tier.n);
    assert!((tier.moves.0..=tier.moves.1).contains(&len), "outside the band: {at}");
    assert_eq!(solve(l).solution.map(|s| s.len()), Some(len), "not a shortest solution: {at}");
    assert!(Play::new(l.clone()).apply(&keys(&g.solution)).solved, "doesn't reach the exit: {at}");
    let text = l.to_string();
    assert!(text.contains('^'), "no springs: {at}");
    let walled = Level::parse(&text.replace('^', "#")).unwrap();
    assert_ne!(solve(&walled).solution.map(|s| s.len()), Some(len), "springs change nothing: {at}");
}

#[test]
fn springs_levels_keep_their_promises_and_need_their_springs() {
    for tier in &TIERS[..2] {
        for seed in 1..=5 {
            check_springs(seed, tier);
        }
    }
    check_springs(1, &TIERS[2]);
}

/// A Mixed level keeps its tier's promises and needs at least two of its three mechanics (M5).
fn check_mixed(seed: u64, tier: &Tier) {
    let g = generate(seed, World::Mixed, tier).unwrap_or_else(|| panic!("mixed tier {} seed {seed}: no level", tier.n));
    let (l, len) = (&g.level, g.solution.len());
    let at = format!("mixed tier {} seed {seed}, {len} moves, in\n{l}", tier.n);
    assert!((tier.moves.0..=tier.moves.1).contains(&len), "outside the band: {at}");
    assert_eq!(solve(l).solution.map(|s| s.len()), Some(len), "not a shortest solution: {at}");
    let text = l.to_string();
    let needed = [text.replace('~', "#"), text.replace('I', "B").replace('i', "b"), text.replace('^', "#")]
        .iter()
        .filter(|t| **t != text && solve(&Level::parse(t).unwrap()).solution.map(|s| s.len()) != Some(len))
        .count();
    assert!(needed >= 2, "only {needed} mechanics matter: {at}");
}

#[test]
fn mixed_levels_keep_their_promises_and_need_two_mechanics() {
    for tier in &TIERS[..2] {
        for seed in 1..=3 {
            check_mixed(seed, tier);
        }
    }
}

fn regenerates_v2(tiers: &[u8]) {
    let levels = suite("suite-v2.json");
    for world in [World::Crumble, World::Ice, World::Springs, World::Mixed] {
        for t in 1..=4 {
            assert_eq!(levels.iter().filter(|l| l.world == world && l.tier == t).count(), 5, "{world:?} tier {t}");
        }
        for l in levels.iter().filter(|l| l.world == world && tiers.contains(&l.tier)) {
            let g = generate(l.seed, world, &TIERS[l.tier as usize - 1]).unwrap();
            assert_eq!(
                (g.level.fingerprint(), g.solution.len()),
                (l.fingerprint, l.moves),
                "{world:?} tier {} seed {}",
                l.tier,
                l.seed
            );
        }
    }
}

/// Suite v2 (P39) is 5 levels per tier for each new world, and the published levels regenerate exactly.
#[test]
fn suite_v2_still_regenerates() {
    regenerates_v2(&[1, 2]);
}

/// Tiers 3 and 4 take minutes: `bash scripts/native.sh test -p puzzle-core --release -- --ignored`.
#[test]
#[ignore]
fn suite_v2_still_regenerates_tiers_3_and_4() {
    regenerates_v2(&[3, 4]);
}
