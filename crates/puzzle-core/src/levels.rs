//! Which level a (world, tier, seed) plays: the one place the page, the suite writer and the tests ask.
//!
//! Generating a tier-4 level takes seconds natively and far longer on a phone (`examples/generate.rs` times it), and
//! [`tidy`](crate::tidy)ing it as long again, so the page's first 25 levels per tier in every world are built in:
//! `levels/page.json`, written offline by `examples/suite.rs -- page`, each the generator's level tidied. A seed in it
//! loads at once; any other seed is generated and only [`seal`]ed, which costs nothing (David: "Seal only"). A seed
//! that makes no level hands over to the next one.
//!
//! The built-in levels follow the newest generator (David's call), while the bench's suites (`bench/puzzle/`) stay
//! frozen, untidied, for comparable scores. While both come from the same generator, tidying a suite level gives the
//! built-in level for its seed, which `tests/levels.rs` checks; a generator change that re-rolls levels means a new
//! suite version and a rebuilt `page.json`.

use crate::{Key, Level, TIERS, World, generate, seal};

/// The built-in levels, one per line in the suites' format.
const PAGE: &str = include_str!("../levels/page.json");

/// How many seeds from the asked one to try before giving up. Every seed 1-12 of every world and tier fills
/// (`examples/generate.rs`), so a miss is rare and eight in a row rarer still.
const FALLBACK: u64 = 8;

/// A level ready to play: the seed it came from (the asked one, or a later one when that made no level) and a
/// shortest solution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picked {
    pub seed: u64,
    pub level: Level,
    pub solution: Vec<Key>,
}

/// One level of a suite file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub world: World,
    /// 1 to 4.
    pub tier: u8,
    pub seed: u64,
    pub fingerprint: u64,
    pub moves: usize,
    pub level: Level,
    pub solution: Vec<Key>,
}

/// The level for `seed` in `world` and `tier` (1 to 4): built in (tidied) if it can be, generated and sealed if not,
/// from the next seeds if that one makes no level. `None` for a tier out of range, or eight seeds in a row that make none.
pub fn pick(world: World, tier: u8, seed: u64) -> Option<Picked> {
    let t = TIERS.get((tier as usize).checked_sub(1)?)?;
    pick_with(seed, |s| {
        built_in(world, tier, s).or_else(|| generate(s, world, t).map(|g| (seal(&g.level), g.solution)))
    })
}

/// [`pick`]'s fallback, apart from where levels come from.
fn pick_with(seed: u64, mut level: impl FnMut(u64) -> Option<(Level, Vec<Key>)>) -> Option<Picked> {
    (seed..seed.saturating_add(FALLBACK))
        .find_map(|s| level(s).map(|(level, solution)| Picked { seed: s, level, solution }))
}

/// The built-in level for `seed`, if `page.json` has it.
fn built_in(world: World, tier: u8, seed: u64) -> Option<(Level, Vec<Key>)> {
    let key = format!("\"world\": \"{}\", \"tier\": {tier}, \"seed\": {seed},", world.name());
    entry(PAGE.lines().find(|l| l.contains(&key))?).map(|e| (e.level, e.solution))
}

/// The page's built-in levels.
pub fn page() -> Vec<Entry> {
    read_suite(PAGE)
}

/// Every level in a suite file's text. Each level sits on one line of its own, so this needs no JSON parser; a line
/// with no `world` is Classic (suite v1 predates worlds).
pub fn read_suite(text: &str) -> Vec<Entry> {
    text.lines()
        .filter(|l| l.contains("\"fingerprint\""))
        .map(|l| entry(l).unwrap_or_else(|| panic!("bad suite line: {l}")))
        .collect()
}

fn entry(line: &str) -> Option<Entry> {
    // A field's value: quoted text up to the closing quote, or a bare number up to the comma.
    let field = |name: &str| -> Option<&str> {
        let rest = &line[line.find(&format!("\"{name}\": "))? + name.len() + 4..];
        Some(match rest.strip_prefix('"') {
            Some(quoted) => &quoted[..quoted.find('"')?],
            None => &rest[..rest.find([',', '}'])?],
        })
    };
    let world = match field("world") {
        Some(name) => World::from_name(name)?,
        None => World::Classic,
    };
    Some(Entry {
        world,
        tier: field("tier")?.parse().ok()?,
        seed: field("seed")?.parse().ok()?,
        fingerprint: u64::from_str_radix(field("fingerprint")?.trim_start_matches("0x"), 16).ok()?,
        moves: field("moves")?.parse().ok()?,
        level: Level::parse(&field("level")?.replace("\\n", "\n")).ok()?,
        solution: field("solution")?.chars().map(Key::from_char).collect::<Option<_>>()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level() -> (Level, Vec<Key>) {
        (Level::parse("#>E#\n####").unwrap(), vec![Key::R])
    }

    #[test]
    fn a_seed_that_makes_no_level_hands_over_to_the_next() {
        let picked = pick_with(10, |s| (s >= 12).then(level)).unwrap();
        assert_eq!(picked.seed, 12);
    }

    #[test]
    fn eight_seeds_in_a_row_with_no_level_give_none() {
        let mut asked = vec![];
        let picked = pick_with(1, |s| {
            asked.push(s);
            None
        });
        assert_eq!(picked, None);
        assert_eq!(asked, (1..=8).collect::<Vec<_>>());
    }

    #[test]
    fn a_tier_out_of_range_gives_none() {
        assert_eq!(pick(World::Classic, 0, 1), None);
        assert_eq!(pick(World::Classic, 5, 1), None);
    }

    #[test]
    fn world_names_and_numbers_round_trip() {
        for (n, w) in World::ALL.into_iter().enumerate() {
            assert_eq!((World::from_name(w.name()), World::from_number(n as u32)), (Some(w), Some(w)));
            assert_eq!(w as usize, n, "a world's number feeds its seeds: renumbering re-rolls it");
        }
        assert_eq!(World::from_name("lava"), None);
        assert_eq!(World::from_number(5), None);
    }
}
