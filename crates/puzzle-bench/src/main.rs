//! `puzzle-bench --model M [--suite v1|v2] [--worlds a,b] [--tiers 1,2] [--levels N] [--fresh N] [--trials K]
//! [--max-calls N] [--timeout S] [--out DIR] [--allow-sweep]`
//!
//! Plays *Out of the Box* levels with headless Claude Code (`claude -p`) through `puzzle-mcp`, or with a native player
//! (`solver`, `random`, `greedy`), and writes `<out>/<stamp>-<model>/results.json` (schema: `puzzle_bench::PuzzleResult`
//! per run, `puzzle_bench::Summary`, and the sweep's settings), each run's evidence in its own directory. The levels
//! are a suite's (v1: Classic, 10 per tier; v2: the other worlds, 5 per tier), each checked against its fingerprint,
//! narrowed by `--worlds`, `--tiers` and `--levels` (the first N per world and tier); `--fresh N` adds N fresh seeds
//! per world and tier (P13), kept in the results. The sweep and its guards are `bench_core`'s: more than one LLM run
//! needs `--allow-sweep`.

use std::path::PathBuf;

use bench_core::{Run, Sweep, fail, sweep};
use puzzle_bench::{Key, Puzzle, fresh_seed, summarise};
use puzzle_core::World;
use puzzle_core::levels::read_suite;
use puzzle_mcp::{MAX_CALLS, instructions};
use serde_json::json;

struct Args {
    model: String,
    suite: String,
    worlds: Option<Vec<World>>,
    tiers: Vec<u8>,
    levels: Option<usize>,
    fresh: usize,
    trials: u32,
    max_calls: u32,
    timeout: u64,
    out: PathBuf,
    allow_sweep: bool,
    budget_usd: f64,
}

fn main() {
    let args = parse();
    let fail = |m: &str| -> ! { fail("puzzle-bench", m) };
    let path = format!("{}/../../bench/puzzle/suite-{}.json", env!("CARGO_MANIFEST_DIR"), args.suite);
    let suite = read_suite(&std::fs::read_to_string(&path).unwrap_or_else(|e| fail(&format!("{path}: {e}"))));
    let mut levels: Vec<Key> = vec![];
    let mut game = Puzzle { levels: std::collections::HashMap::new() };
    let mut worlds: Vec<World> = suite.iter().map(|e| e.world).collect();
    worlds.dedup();
    if let Some(only) = &args.worlds {
        worlds.retain(|w| only.contains(w));
    }
    for &world in &worlds {
        for &tier in &args.tiers {
            let entries = suite.iter().filter(|e| e.world == world && e.tier == tier);
            for e in entries.take(args.levels.unwrap_or(usize::MAX)) {
                // The suite's level as published. A session's server generates it again from the seed, and its
                // record names it by fingerprint, which the analysis checks against this one.
                game.levels.insert((world.name().into(), tier, e.seed), (e.level.clone(), e.moves));
                levels.push(Key { world: world.name().into(), tier, seed: e.seed, fresh: false, trial: 1 });
            }
            for _ in 0..args.fresh {
                let seed = fresh_seed(world, tier);
                let key = Key { world: world.name().into(), tier, seed, fresh: true, trial: 1 };
                game.levels.insert((key.world.clone(), tier, seed), key.pick().level().expect("a fresh seed's level"));
                levels.push(key);
            }
        }
    }
    let runs: Vec<Run<Key>> = levels
        .iter()
        .flat_map(|k| (1..=args.trials).map(move |trial| Key { trial, ..k.clone() }))
        .map(|key| {
            let world = World::from_name(&key.world).expect("a world");
            let mut server_args = vec!["--world".into(), key.world.clone(), "--tier".into(), key.tier.to_string()];
            server_args.extend([
                "--seed".into(),
                key.seed.to_string(),
                "--max-calls".into(),
                args.max_calls.to_string(),
            ]);
            Run {
                dir: format!("{}-t{}-s{}-k{}", key.world, key.tier, key.seed, key.trial),
                server_args,
                prompt: prompt(world, args.max_calls),
                key,
            }
        })
        .collect();
    let s = Sweep {
        model: args.model.clone(),
        label: args.model.clone(),
        out: args.out.clone(),
        timeout: args.timeout,
        budget_usd: args.budget_usd,
        system: "You are an agent playing a puzzle through tools. Solve it as well as you can.".into(),
        allow_sweep: args.allow_sweep,
        schema: "puzzle-bench results v1 (crates/puzzle-bench)",
        settings: json!({
            "suite": args.suite,
            "worlds": worlds.iter().map(|w| w.name()).collect::<Vec<_>>(),
            "tiers": args.tiers,
            "fresh_per_tier": args.fresh,
            "trials": args.trials,
            "max_calls": args.max_calls,
            "prompts": worlds.iter().map(|&w| (w.name().to_owned(), json!(prompt(w, args.max_calls)))).collect::<serde_json::Map<_, _>>(),
        }),
    };
    let results = sweep(&game, &s, &runs);
    let s = summarise(&results);
    eprintln!("puzzle-bench: {} runs, {} verified; solve rate {:.0}%", s.runs, s.verified, s.solve_rate * 100.0);
}

/// The task: the server's rules and legend for the world, repeated here so they reach the model whatever its harness
/// does with MCP instructions, then the goal.
fn prompt(world: World, max_calls: u32) -> String {
    format!(
        "{}\n\nYour goal: solve the level, reaching the exit. You have at most {max_calls} tool calls in total (look and \
         status count too). Play until it is solved or the session is over, then reply with one line saying how it went.",
        instructions(world)
    )
}

fn parse() -> Args {
    let fail = |m: &str| -> ! { fail("puzzle-bench", m) };
    let mut a = Args {
        model: String::new(),
        suite: "v1".into(),
        worlds: None,
        tiers: vec![1, 2, 3, 4],
        levels: None,
        fresh: 0,
        trials: 1,
        max_calls: MAX_CALLS,
        timeout: 1800,
        out: PathBuf::from("bench/puzzle/results"),
        allow_sweep: false,
        budget_usd: 5.0,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        if flag == "--allow-sweep" {
            a.allow_sweep = true;
            continue;
        }
        let v = it.next().unwrap_or_else(|| fail(&format!("{flag} needs a value")));
        let num = |s: &str| s.parse::<u64>().unwrap_or_else(|_| fail(&format!("{flag}: not a number: {s}")));
        match flag.as_str() {
            "--model" => a.model = v,
            "--suite" => a.suite = v,
            "--worlds" => {
                let named =
                    v.split(',').map(|w| World::from_name(w.trim()).unwrap_or_else(|| fail(&format!("no world {w}"))));
                a.worlds = Some(named.collect());
            }
            "--tiers" => a.tiers = v.split(',').map(|t| num(t.trim()) as u8).collect(),
            "--levels" => a.levels = Some(num(&v) as usize),
            "--fresh" => a.fresh = num(&v) as usize,
            "--trials" => a.trials = num(&v) as u32,
            "--max-calls" => a.max_calls = num(&v) as u32,
            "--timeout" => a.timeout = num(&v),
            "--out" => a.out = v.into(),
            "--budget-usd" => a.budget_usd = v.parse().unwrap_or_else(|_| fail("--budget-usd: not a number")),
            _ => fail(&format!("unknown argument {flag}")),
        }
    }
    if a.model.is_empty() {
        fail("--model is required (e.g. haiku, sonnet, opus, or a native player: solver, random, greedy)");
    }
    a
}
