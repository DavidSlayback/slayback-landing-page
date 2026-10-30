//! `box-bench --model M [--seeds 1,2,3] [--trials K] [--max-calls N] [--rim M|game] [--max-seconds S] [--timeout S]
//! [--out DIR] [--allow-sweep]`
//!
//! Bench v1 plays the mini box (`box_env::MINI_RIM`, `--rim game` for the game's own first box). It plays *Escape the
//! Box* with headless Claude Code (`claude -p`) through `box-mcp`, once per seed x trial, then
//! replays every run's trace and writes `<out>/<stamp>-<model>/results.json` (schema: `box_bench::RunResult` per run,
//! `box_bench::Summary`, and the sweep's settings). Each run's evidence sits beside it in `s<seed>-t<trial>/`: the
//! MCP config, the server's `record.json` and the agent's `transcript.jsonl`. The sweep and its guards (subscription
//! billing, no CLAUDE.md, only the game's tools) are `bench_core`'s.
//!
//! Until David approves a sweep, one run of at most 40 calls is the limit; `--allow-sweep` lifts it.

use std::path::PathBuf;

use bench_core::{Run, Sweep, fail, sweep};
use box_bench::{Escape, Key, summarise};
use box_mcp::instructions;
use serde_json::json;

/// The capped default: one run, at most this many tool calls.
const SMOKE_CALLS: u32 = 40;

struct Args {
    model: String,
    seeds: Vec<u32>,
    trials: u32,
    max_calls: u32,
    timeout: u64,
    out: PathBuf,
    allow_sweep: bool,
    budget_usd: f64,
    /// The first box's rim: the mini box by default (bench v1); `--rim game` for the game's own.
    rim: Option<f32>,
    /// The play-time limit, seconds of game time, the same for every agent (bench v1: 60; LLM runs end sooner, at
    /// their call cap).
    max_seconds: f32,
}

impl Args {
    fn max_steps(&self) -> u32 {
        (self.max_seconds * box_env::FPS / box_env::EnvConfig::default().frame_skip as f32).round() as u32
    }
}

fn main() {
    let args = parse();
    if !box_bench::native(&args.model) && !args.allow_sweep && args.max_calls > SMOKE_CALLS {
        fail("box-bench", &format!("more than {SMOKE_CALLS} calls needs --allow-sweep (David approves sweeps)"));
    }
    let game = Escape { rim: args.rim, max_steps: args.max_steps() };
    let prompt = prompt(args.max_calls);
    let runs: Vec<Run<Key>> = args
        .seeds
        .iter()
        .flat_map(|&seed| (1..=args.trials).map(move |trial| Key { seed, trial }))
        .map(|key| Run {
            key,
            dir: format!("s{}-t{}", key.seed, key.trial),
            server_args: server_args(&args, key.seed),
            prompt: prompt.clone(),
        })
        .collect();
    let label = args.model.strip_prefix("policy:").map_or(args.model.clone(), |p| {
        let dir = std::path::Path::new(p).parent().and_then(|d| d.file_name());
        format!("policy-{}", dir.map_or("file".into(), |n| n.to_string_lossy().into_owned()))
    });
    let s = Sweep {
        model: args.model.clone(),
        label,
        out: args.out.clone(),
        timeout: args.timeout,
        budget_usd: args.budget_usd,
        system: "You are an agent playing a video game through tools. Play it as well as you can.".into(),
        allow_sweep: args.allow_sweep,
        schema: "box-bench results v1 (crates/box-bench)",
        settings: json!({
            "seeds": args.seeds,
            "trials": args.trials,
            "max_calls": args.max_calls,
            "bench_version": 1,
            "first_rim": args.rim,
            "max_seconds": args.max_seconds,
            "prompt": prompt,
        }),
    };
    let results = sweep(&game, &s, &runs);
    let s = summarise(&results);
    eprintln!(
        "box-bench: {} runs, {} verified; escape rate {:.0}%, pass^{} {:.0}%",
        s.runs,
        s.verified,
        s.escape_rate * 100.0,
        args.trials,
        s.pass_k * 100.0
    );
}

/// box-mcp's arguments for a run, apart from where it keeps its record: its seed, the first box's rim, the call cap
/// and the play-time limit.
fn server_args(args: &Args, seed: u32) -> Vec<String> {
    let mut a = vec!["--seed".to_owned(), seed.to_string(), "--max-calls".to_owned(), args.max_calls.to_string()];
    a.extend(["--max-steps".to_owned(), args.max_steps().to_string()]);
    if let Some(rim) = args.rim {
        a.extend(["--rim".to_owned(), rim.to_string()]);
    }
    a
}

/// The task, the same for every model and run: the rules come from the server's own instructions, repeated here so
/// they reach the model whatever its harness does with MCP instructions.
fn prompt(max_calls: u32) -> String {
    format!(
        "{}\n\nYour goal: escape as many boxes as you can before you die. You have at most {max_calls} tool \
         calls in total (look and status count too), so make them count. Play until the run is over or your calls run \
         out, then reply with one line saying how it went.",
        instructions()
    )
}

fn parse() -> Args {
    let fail = |m: &str| -> ! { fail("box-bench", m) };
    let mut a = Args {
        model: String::new(),
        seeds: vec![1],
        trials: 1,
        max_calls: SMOKE_CALLS,
        rim: Some(box_env::MINI_RIM),
        max_seconds: 60.0,
        timeout: 1800,
        out: PathBuf::from("bench/results"),
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
            "--seeds" => a.seeds = v.split(',').map(|s| num(s.trim()) as u32).collect(),
            "--trials" => a.trials = num(&v) as u32,
            "--max-calls" => a.max_calls = num(&v) as u32,
            "--max-seconds" => a.max_seconds = v.parse().unwrap_or_else(|_| fail("--max-seconds: a number")),
            "--rim" => {
                a.rim = if v == "game" {
                    None
                } else {
                    Some(v.parse().unwrap_or_else(|_| fail("--rim: metres, or `game`")))
                }
            }
            "--timeout" => a.timeout = num(&v),
            "--out" => a.out = v.into(),
            "--budget-usd" => a.budget_usd = v.parse().unwrap_or_else(|_| fail("--budget-usd: not a number")),
            _ => fail(&format!("unknown argument {flag}")),
        }
    }
    if a.model.is_empty() {
        fail("--model is required (e.g. haiku, sonnet, opus, or a baseline: brawler, random)");
    }
    a
}
