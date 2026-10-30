//! Writes a bench suite (P13), each world's levels from seed 1 upward, skipping any seed that doesn't fill.
//!
//! - Suite v1, Classic, 10 levels per tier:
//!   `bash scripts/native.sh run -p puzzle-core --release --example suite > bench/puzzle/suite-v1.json`
//! - Suite v2 (P39, M5), 5 levels per tier for each of Crumble, Ice, Springs and Mixed:
//!   `bash scripts/native.sh run -p puzzle-core --release --example suite -- v2 > bench/puzzle/suite-v2.json`
//! - The page's built-in levels, 25 per tier in every world, each tidied (`puzzle_core::levels`; each world and tier
//!   on a thread of its own, about 10 minutes on 14 cores):
//!   `bash scripts/native.sh run -p puzzle-core --release --example suite -- page > crates/puzzle-core/levels/page.json`
//! - One world, for a quick look: `-- crumble 2 2` (world, levels per tier, tiers up to).
//!
//! Each level is one line, which `puzzle_core::levels` and the browser tests read back. Seeds below 1,000,000 are
//! kept for the bench; RL training draws from 1,000,000 up to 2^32, and a run's fresh seeds from 2^32 up.

use puzzle_core::{CAP, TIERS, Tier, World, generate, tidy};

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
    let page = name == "out-of-the-box page";
    let groups: Vec<(World, &Tier)> = worlds
        .iter()
        .map(|&w| World::from_name(w).unwrap_or_else(|| panic!("no world {w}")))
        .flat_map(|world| TIERS[..tiers].iter().map(move |tier| (world, tier)))
        .collect();
    let lines: Vec<String> = std::thread::scope(|s| {
        let handles: Vec<_> =
            groups.iter().map(|&(world, tier)| s.spawn(move || group(name, world, tier, per_tier, page))).collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
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

/// One world and tier's lines: the first `per_tier` seeds that fill, tidied for the page.
fn group(name: &str, world: World, tier: &Tier, per_tier: usize, page: bool) -> Vec<String> {
    let (mut seed, mut lines) = (1u64, vec![]);
    while lines.len() < per_tier {
        if let Some(g) = generate(seed, world, tier) {
            let (level, solution) = if page { tidy(&g.level, g.solution, world, tier) } else { (g.level, g.solution) };
            let keys: String = solution.iter().map(|k| format!("{k:?}")).collect();
            // Suite v1 has no world field, so its file stays byte for byte what it was.
            let tag =
                if name == "out-of-the-box v1" { String::new() } else { format!("\"world\": \"{}\", ", world.name()) };
            lines.push(format!(
                "    {{{tag}\"tier\": {}, \"seed\": {seed}, \"fingerprint\": \"{:#018x}\", \"moves\": {}, \"level\": \"{}\", \"solution\": \"{keys}\"}}",
                tier.n,
                level.fingerprint(),
                solution.len(),
                level.to_string().replace('\n', "\\n"),
            ));
        }
        seed += 1;
    }
    lines
}
