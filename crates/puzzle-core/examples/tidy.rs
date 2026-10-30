//! Times the tidy pass over the first built-in levels of every world and tier, and prints one before and after.
//! `cargo run --release -p puzzle-core --example tidy -- [levels per world and tier]`

use std::time::Instant;

use puzzle_core::{TIERS, levels, tidy};

fn main() {
    let n: u64 = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(2);
    for e in levels::page().into_iter().filter(|e| e.seed <= n) {
        let t = Instant::now();
        let (tidied, solution) = tidy(&e.level, e.solution.clone(), e.world, &TIERS[e.tier as usize - 1]);
        let count = |s: &str, cs: &[char]| s.chars().filter(|c| cs.contains(c)).count();
        let (a, b) = (e.level.to_string(), tidied.to_string());
        println!(
            "{} tier {} seed {}: {:.2} s, {} moves, blocks {} -> {}, walls {} -> {}",
            e.world.name(),
            e.tier,
            e.seed,
            t.elapsed().as_secs_f32(),
            solution.len(),
            count(&a, &['B', 'I']),
            count(&b, &['B', 'I']),
            count(&a, &['#']),
            count(&b, &['#'])
        );
        if e.seed == 1 && e.tier >= 3 {
            println!("{a}\n\n{b}\n");
        }
    }
}
