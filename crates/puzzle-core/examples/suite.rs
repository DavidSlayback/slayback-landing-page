//! Writes a bench suite (P13), each world's levels from seed 1 upward, skipping any seed that doesn't fill.
//!
//! - Suite v1, Classic, 10 levels per tier:
//!   `bash scripts/native.sh run -p puzzle-core --release --example suite > bench/puzzle/suite-v1.json`
//! - Suite v2 (P39, M5), 5 levels per tier for each of Crumble, Ice, Springs and Mixed:
//!   `bash scripts/native.sh run -p puzzle-core --release --example suite -- v2 > bench/puzzle/suite-v2.json`
//! - The page's built-in levels, 25 per tier in every world (`puzzle_core::levels`; about 10 minutes):
//!   `bash scripts/native.sh run -p puzzle-core --release --example suite -- page > crates/puzzle-core/levels/page.json`
//! - One world, for a quick look: `-- crumble 2 2` (world, levels per tier, tiers up to).
//!
//! Each level is one line, which `puzzle_core::levels` and the browser tests read back. Seeds below 1,000,000 are
//! kept for the bench; RL training draws from 1,000,000 up to 2^32, and a run's fresh seeds from 2^32 up.

use puzzle_core::{CAP, TIERS, World, generate};

fn main() {
    let mut args = std::env::args().skip(1);
    let first = args.next().unwrap_or_else(|| "classic".into());
    let (name, worlds, per_tier, tiers) = if first == "v2" {
        ("out-of-the-box v2", vec!["crumble", "ice", "springs", "mixed"], 5, 4)
    } else if first == "page" {
        ("out-of-the-box page", World::ALL.iter().map(|w| w.name()).collect(), 25, 4)
    } else {
        let per_tier: usize = args.next().and_then(|a| a.parse().ok()).unwrap_or(10);
        let tiers: usize = args.next().and_then(|a| a.parse().ok()).unwrap_or(4);
        let name = if first == "classic" { "out-of-the-box v1" } else { "one world" };
        (name, vec![first.as_str()], per_tier, tiers)
    };
    let mut lines = vec![];
    for &w in &worlds {
        let world = World::from_name(w).unwrap_or_else(|| panic!("no world {w}"));
        for tier in &TIERS[..tiers] {
            let (mut seed, mut kept) = (1u64, 0);
            while kept < per_tier {
                if let Some(g) = generate(seed, world, tier) {
                    let keys: String = g.solution.iter().map(|k| format!("{k:?}")).collect();
                    // Suite v1 has no world field, so its file stays byte for byte what it was.
                    let tag = if name == "out-of-the-box v1" { String::new() } else { format!("\"world\": \"{w}\", ") };
                    lines.push(format!(
                        "    {{{tag}\"tier\": {}, \"seed\": {seed}, \"fingerprint\": \"{:#018x}\", \"moves\": {}, \"level\": \"{}\", \"solution\": \"{keys}\"}}",
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
    }
    println!("{{");
    println!("  \"suite\": \"{name}\",");
    let bands: Vec<String> = TIERS.iter().map(|t| format!("[{}, {}]", t.moves.0, t.moves.1)).collect();
    println!("  \"tiers\": [{}],", bands.join(", "));
    println!("  \"cap\": {CAP},");
    println!("  \"levels\": [");
    println!("{}", lines.join(",\n"));
    println!("  ]");
    println!("}}");
}
