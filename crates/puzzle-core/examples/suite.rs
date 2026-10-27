//! Writes the bench suite (P13): 10 levels per tier, from seed 1 upward, skipping any seed that doesn't fill.
//!
//! `bash scripts/native.sh run -p puzzle-core --release --example suite > bench/puzzle/suite-v1.json`
//!
//! `-- crumble 1 1` writes one Crumble level for tier 1 instead (world, levels per tier, tiers up to), as the browser
//! tests' fixture until suite v2 (M5).
//!
//! Each level is one line, so `tests/generate.rs` reads it back without a JSON parser. Seeds below 1,000,000 are kept
//! for the bench; RL training draws from 1,000,000 up to 2^32, and a run's fresh seeds from 2^32 up.

use puzzle_core::{CAP, TIERS, World, generate};

fn main() {
    let mut args = std::env::args().skip(1);
    let world = if args.next().as_deref() == Some("crumble") { World::Crumble } else { World::Classic };
    let per_tier: usize = args.next().and_then(|a| a.parse().ok()).unwrap_or(10);
    let tiers: usize = args.next().and_then(|a| a.parse().ok()).unwrap_or(4);
    let mut lines = vec![];
    for tier in &TIERS[..tiers] {
        let (mut seed, mut kept) = (1u64, 0);
        while kept < per_tier {
            if let Some(g) = generate(seed, world, tier) {
                let keys: String = g.solution.iter().map(|k| format!("{k:?}")).collect();
                lines.push(format!(
                    "    {{\"tier\": {}, \"seed\": {seed}, \"fingerprint\": \"{:#018x}\", \"moves\": {}, \"level\": \"{}\", \"solution\": \"{keys}\"}}",
                    tier.n,
                    g.level.fingerprint(),
                    g.solution.len(),
                    g.level.to_string().replace('\n', "\\n"),
                ));
                kept += 1;
            }
            seed += 1;
        }
    }
    println!("{{");
    let name = if world == World::Classic { "out-of-the-box v1" } else { "crumble fixture, until suite v2" };
    println!("  \"suite\": \"{name}\",");
    let bands: Vec<String> = TIERS.iter().map(|t| format!("[{}, {}]", t.moves.0, t.moves.1)).collect();
    println!("  \"tiers\": [{}],", bands.join(", "));
    println!("  \"cap\": {CAP},");
    println!("  \"levels\": [");
    println!("{}", lines.join(",\n"));
    println!("  ]");
    println!("}}");
}
