//! Times generation per tier: how often a seed fills, how long it takes, how many candidates it tries, and how long
//! the solutions come out.
//!
//! `bash scripts/native.sh run -p puzzle-core --release --example generate [seeds per tier, default 20] [tiers, e.g. 34] [world: classic, crumble, ice or springs]`

use std::time::Instant;

use puzzle_core::{TIERS, World, generate};

fn main() {
    let mut args = std::env::args().skip(1);
    let seeds: u64 = args.next().and_then(|a| a.parse().ok()).unwrap_or(20);
    let only = args.next().unwrap_or_else(|| "1234".into());
    let world = match args.next().as_deref() {
        Some("crumble") => World::Crumble,
        Some("ice") => World::Ice,
        Some("springs") => World::Springs,
        _ => World::Classic,
    };
    for tier in TIERS.iter().filter(|t| only.contains(char::from(b'0' + t.n))) {
        let (mut ms, mut tries, mut lens, mut misses) = (vec![], vec![], vec![], 0);
        for seed in 1..=seeds {
            let t = Instant::now();
            let found = generate(seed, world, tier);
            ms.push(t.elapsed().as_secs_f64() * 1e3);
            match found {
                Some(g) => {
                    tries.push(g.tries);
                    lens.push(g.solution.len());
                }
                None => misses += 1,
            }
        }
        ms.sort_by(f64::total_cmp);
        tries.sort();
        lens.sort();
        let mid = |v: &[usize]| v.get(v.len() / 2).copied().unwrap_or(0);
        println!(
            "tier {} ({}x{}, {}-{} moves): filled {}/{seeds}  ms median {:.0} max {:.0} total {:.1} s  tries median {} max {}  moves {:?}",
            tier.n,
            tier.w,
            tier.h,
            tier.moves.0,
            tier.moves.1,
            seeds as usize - misses,
            ms[ms.len() / 2],
            ms[ms.len() - 1],
            ms.iter().sum::<f64>() / 1e3,
            mid(&tries),
            tries.last().copied().unwrap_or(0),
            lens,
        );
    }
}
