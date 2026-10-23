//! `box-bench --model M [--seeds 1,2,3] [--trials K] [--max-calls N] [--rim M|game] [--max-seconds S] [--timeout S]
//! [--out DIR] [--allow-sweep]`
//!
//! Bench v1 plays the mini box (`box_env::MINI_RIM`, `--rim game` for the game's own first box). It plays *Escape the
//! Box* with headless Claude Code (`claude -p`) through `box-mcp`, once per seed x trial, then
//! replays every run's trace and writes `<out>/<stamp>-<model>/results.json` (schema: `box_bench::RunResult` per run,
//! `box_bench::Summary`, and the sweep's settings). Each run's evidence sits beside it in `s<seed>-t<trial>/`: the
//! MCP config, the server's `record.json` and the agent's `transcript.jsonl`.
//!
//! Runs go on the Claude subscription (David, 2026-10-19: "Use my Claude subscription"; "try to use the max if you
//! can"): `ANTHROPIC_API_KEY` and `ANTHROPIC_AUTH_TOKEN` are removed from the agent's environment, and a run whose
//! session reports any API-key source stops the sweep. `CLAUDE_CODE_DISABLE_CLAUDE_MDS` keeps David's personal
//! CLAUDE.md out of the agent's context; the agent gets only box-mcp's tools.
//!
//! Until David approves a sweep, one run of at most 40 calls is the limit; `--allow-sweep` lifts it.

use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use box_bench::{RunResult, analyse, baseline, native, summarise};
use box_mcp::{INSTRUCTIONS, Record};
use serde_json::{Value, json};

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
        (self.max_seconds * 60.0 / box_env::EnvConfig::default().frame_skip as f32).round() as u32
    }
}

fn main() {
    let args = parse();
    let native = native(&args.model);
    if !native && !args.allow_sweep && (args.seeds.len() as u32 * args.trials > 1 || args.max_calls > SMOKE_CALLS) {
        fail(&format!(
            "more than one run or more than {SMOKE_CALLS} calls needs --allow-sweep (David approves sweeps: they use his plan)"
        ));
    }
    let server = sibling("box-mcp");
    if !native && !server.exists() {
        fail(&format!("{} not found: build it first (native.sh build -p box-mcp)", server.display()));
    }

    let stamp = stamp();
    // Absolute: the agent runs from inside its run directory, and the server writes its record by this path.
    let label = args.model.strip_prefix("policy:").map_or(args.model.clone(), |p| {
        format!(
            "policy-{}",
            std::path::Path::new(p)
                .parent()
                .and_then(|d| d.file_name())
                .map_or("file".into(), |n| n.to_string_lossy().into_owned())
        )
    });
    let sweep = std::path::absolute(args.out.join(format!("{stamp}-{}", label.replace(['/', ':', ' ', '\\'], "_"))))
        .unwrap_or_else(|e| fail(&format!("resolving --out: {e}")));
    std::fs::create_dir_all(&sweep).unwrap_or_else(|e| fail(&format!("{}: {e}", sweep.display())));
    let prompt = prompt(args.max_calls);
    let system = "You are an agent playing a video game through tools. Play it as well as you can.";
    let mut results = Vec::new();

    'sweep: for &seed in &args.seeds {
        for trial in 1..=args.trials {
            let dir_name = format!("s{seed}-t{trial}");
            let dir = sweep.join(&dir_name);
            std::fs::create_dir_all(&dir).expect("a run directory");
            eprintln!("box-bench: seed {seed} trial {trial} ({})", args.model);
            let (r, limited) = if native {
                (play_baseline(&args.model, &dir, seed, trial, args.rim, args.max_steps()), false)
            } else {
                run(&args, &server, &dir, seed, &prompt, system)
            };
            let result = RunResult { seed, trial, dir: dir_name, ..r };
            match (&result.analysis, &result.error) {
                (Some(a), _) => eprintln!(
                    "  {:?}{}: {} kills, {} escaped, best pile {:.0}% of the rim, {} calls, {:.0} s of play, verified {}",
                    a.ending,
                    if a.flags.is_empty() { String::new() } else { format!(" {:?}", a.flags) },
                    a.replayed.kills,
                    a.replayed.escapes,
                    a.best_pile * 100.0,
                    a.calls,
                    a.game_seconds,
                    a.verified
                ),
                (None, Some(e)) => eprintln!("  no result: {e}"),
                _ => {}
            }
            if let Some(used) = result.plan_used {
                eprintln!("  plan: {:.0}% of the five-hour window used", used * 100.0);
            }
            results.push(result);
            write(&sweep, &args, &prompt, system, &results);
            if limited {
                eprintln!("box-bench: the plan's rate limit refused the session: stopping the sweep here");
                break 'sweep;
            }
        }
    }
    let s = summarise(&results);
    eprintln!(
        "box-bench: {} runs, {} verified; escape rate {:.0}%, pass^{} {:.0}%; written to {}",
        s.runs,
        s.verified,
        s.escape_rate * 100.0,
        args.trials,
        s.pass_k * 100.0,
        sweep.join("results.json").display()
    );
}

/// A baseline's run: played here, its record kept beside it like a session's.
fn play_baseline(name: &str, dir: &Path, seed: u32, trial: u32, rim: Option<f32>, max_steps: u32) -> RunResult {
    let started = Instant::now();
    let record =
        baseline(name, seed, trial, rim, max_steps).unwrap_or_else(|| fail(&format!("no baseline or policy {name:?}")));
    std::fs::write(dir.join("record.json"), serde_json::to_string_pretty(&record).unwrap() + "\n").unwrap();
    RunResult {
        seed,
        trial,
        dir: String::new(),
        analysis: Some(analyse(&record)),
        error: None,
        wall_seconds: started.elapsed().as_secs_f32(),
        reported_cost_usd: 0.0,
        agent_turns: 0,
        input_tokens: 0,
        output_tokens: 0,
        plan_used: None,
    }
}

/// One agent session: launch, wait (killing it past the timeout), then check and analyse its evidence. Also says
/// whether the plan's rate limit refused it, which ends the sweep.
fn run(args: &Args, server: &Path, dir: &Path, seed: u32, prompt: &str, system: &str) -> (RunResult, bool) {
    let record_path = dir.join("record.json");
    let config = json!({ "mcpServers": { "box": {
        "command": server,
        "args": server_args(args, seed, &record_path),
    } } });
    std::fs::write(dir.join("mcp.json"), serde_json::to_string_pretty(&config).unwrap() + "\n").unwrap();
    let tools: Vec<String> = ["look", "status", "move", "punch", "kick", "dash", "grab", "wait"]
        .iter()
        .map(|t| format!("mcp__box__{t}"))
        .collect();

    let transcript = std::fs::File::create(dir.join("transcript.jsonl")).unwrap();
    let mut cmd = Command::new("claude");
    cmd.current_dir(dir)
        .args(["-p", prompt, "--model", &args.model, "--mcp-config", "mcp.json", "--strict-mcp-config"])
        .args(["--tools", "", "--allowedTools", &tools.join(",")])
        .args(["--system-prompt", system, "--setting-sources", "project", "--no-session-persistence"])
        .args(["--output-format", "stream-json", "--verbose", "--max-budget-usd", &args.budget_usd.to_string()])
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("ANTHROPIC_AUTH_TOKEN")
        .env("CLAUDE_CODE_DISABLE_CLAUDE_MDS", "1")
        .env("CLAUDE_CODE_DISABLE_AUTO_MEMORY", "1")
        .stdin(Stdio::null())
        .stdout(transcript)
        .stderr(Stdio::from(std::fs::File::create(dir.join("stderr.txt")).unwrap()));
    let started = Instant::now();
    let mut child = cmd.spawn().unwrap_or_else(|e| fail(&format!("starting claude: {e}")));
    let mut timed_out = false;
    loop {
        if child.try_wait().expect("waiting on claude").is_some() {
            break;
        }
        if started.elapsed() > Duration::from_secs(args.timeout) {
            let _ = child.kill();
            let _ = child.wait();
            timed_out = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    let wall_seconds = started.elapsed().as_secs_f32();

    let (init, result, rate) = read_transcript(&dir.join("transcript.jsonl"));
    let limited = rate.as_ref().is_some_and(|r| r["status"] == "rejected")
        || result.as_ref().is_some_and(|r| r["api_error_status"] == 429);
    // The billing guard: a subscription session reports no API-key source.
    if let Some(source) = init.as_ref().and_then(|i| i["apiKeySource"].as_str()).filter(|s| *s != "none") {
        fail(&format!("the session used an API key ({source}), not the subscription: stopping the sweep"));
    }
    let model_tools: Vec<&str> = init
        .as_ref()
        .and_then(|i| i["tools"].as_array())
        .map(|t| t.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    if let Some(stray) = model_tools.iter().find(|t| !t.starts_with("mcp__box__")) {
        fail(&format!("the agent had a tool besides the game's ({stray}): stopping the sweep"));
    }

    // A game server that didn't start is the harness's failure, not the agent's: no result, and said so.
    let connected = init
        .as_ref()
        .and_then(|i| i["mcp_servers"].as_array())
        .is_some_and(|servers| servers.iter().any(|s| s["name"] == "box" && s["status"] == "connected"));

    let usage = result.as_ref().map(|r| &r["usage"]);
    let tokens = |k: &str| usage.and_then(|u| u[k].as_u64()).unwrap_or(0);
    let mut out = RunResult {
        seed,
        trial: 0,
        dir: String::new(),
        analysis: None,
        error: None,
        wall_seconds,
        reported_cost_usd: result.as_ref().and_then(|r| r["total_cost_usd"].as_f64()).unwrap_or(0.0),
        agent_turns: result.as_ref().and_then(|r| r["num_turns"].as_u64()).unwrap_or(0) as u32,
        input_tokens: tokens("input_tokens")
            + tokens("cache_read_input_tokens")
            + tokens("cache_creation_input_tokens"),
        output_tokens: tokens("output_tokens"),
        plan_used: rate.as_ref().and_then(|r| r["unifiedWindows"]["five_hour"]["utilization"].as_f64()),
    };
    if !connected {
        out.error = Some("the game server didn't connect (see stderr.txt and the transcript's init event)".into());
        return (out, limited);
    }
    match std::fs::read_to_string(&record_path).ok().and_then(|s| serde_json::from_str::<Record>(&s).ok()) {
        Some(record) => out.analysis = Some(analyse(&record)),
        None => {
            out.error = Some(if timed_out {
                format!("timed out after {} s with no record", args.timeout)
            } else {
                "no record: the agent never reached the game".into()
            })
        }
    }
    if timed_out && out.error.is_none() {
        out.error = Some(format!("timed out after {} s (analysed up to there)", args.timeout));
    }
    (out, limited)
}

/// box-mcp's arguments for a run: its seed, the first box's rim, the call cap and where to keep its record.
fn server_args(args: &Args, seed: u32, record: &Path) -> Vec<String> {
    let mut a = vec!["--seed".to_owned(), seed.to_string(), "--max-calls".to_owned(), args.max_calls.to_string()];
    a.extend(["--max-steps".to_owned(), args.max_steps().to_string()]);
    if let Some(rim) = args.rim {
        a.extend(["--rim".to_owned(), rim.to_string()]);
    }
    a.extend(["--trace".to_owned(), record.display().to_string()]);
    a
}

/// The session's init event (model, tools, billing source), its final result event, and its last rate-limit info.
fn read_transcript(path: &Path) -> (Option<Value>, Option<Value>, Option<Value>) {
    let (mut init, mut result, mut rate) = (None, None, None);
    let Ok(file) = std::fs::File::open(path) else { return (None, None, None) };
    for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
        let Ok(e) = serde_json::from_str::<Value>(&line) else { continue };
        match (e["type"].as_str(), e["subtype"].as_str()) {
            // The first init is the session's own (later ones, after compaction or a resume, list no servers).
            (Some("system"), Some("init")) if init.is_none() => init = Some(e),
            (Some("result"), _) => result = Some(e),
            (Some("rate_limit_event"), _) => rate = Some(e["rate_limit_info"].clone()),
            _ => {}
        }
    }
    (init, result, rate)
}

/// The task, the same for every model and run: the rules come from the server's own instructions, repeated here so
/// they reach the model whatever its harness does with MCP instructions.
fn prompt(max_calls: u32) -> String {
    format!(
        "{INSTRUCTIONS}\n\nYour goal: escape as many boxes as you can before you die. You have at most {max_calls} tool \
         calls in total (look and status count too), so make them count. Play until the run is over or your calls run \
         out, then reply with one line saying how it went."
    )
}

fn write(sweep: &Path, args: &Args, prompt: &str, system: &str, results: &[RunResult]) {
    let doc = json!({
        "schema": "box-bench results v1 (crates/box-bench)",
        "game_commit": git(&["rev-parse", "HEAD"]),
        "game_dirty": !git(&["status", "--porcelain"]).is_empty(),
        "claude_version": output("claude", &["--version"]),
        "agent": "claude -p (Claude Code, headless) through box-mcp; subscription billing",
        "model": args.model,
        "seeds": args.seeds,
        "trials": args.trials,
        "max_calls": args.max_calls,
        "bench_version": 1,
        "first_rim": args.rim,
        "max_seconds": args.max_seconds,
        "system_prompt": system,
        "prompt": prompt,
        "summary": summarise(results),
        "runs": results,
    });
    std::fs::write(sweep.join("results.json"), serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
}

fn git(args: &[&str]) -> String {
    output("git", args)
}

fn output(cmd: &str, args: &[&str]) -> String {
    Command::new(cmd)
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_default()
}

/// A built binary next to this one (both come out of the same target directory).
fn sibling(name: &str) -> PathBuf {
    let me = std::env::current_exe().expect("this executable's path");
    me.with_file_name(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

/// UTC `YYYY-MM-DD-HHMM`, from the system clock (no date crate).
fn stamp() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Civil from days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + (m <= 2) as i64;
    format!("{y:04}-{m:02}-{d:02}-{:02}{:02}", rem / 3600, rem % 3600 / 60)
}

fn parse() -> Args {
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

fn fail(message: &str) -> ! {
    eprintln!("box-bench: {message}");
    std::process::exit(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_real_transcript_reads_as_a_subscription_session_with_the_game_connected_and_room_left() {
        // The first capped run's transcript, as Claude Code 2.1.303 wrote it.
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../bench/results/2026-10-19-2134-haiku/s1-t1/transcript.jsonl");
        let (init, result, rate) = read_transcript(&path);
        let init = init.expect("an init event");
        assert_eq!(init["apiKeySource"], "none", "billed to the subscription");
        assert!(
            init["mcp_servers"].as_array().unwrap().iter().any(|s| s["name"] == "box" && s["status"] == "connected")
        );
        assert!(init["tools"].as_array().unwrap().iter().all(|t| t.as_str().unwrap().starts_with("mcp__box__")));
        assert!(result.expect("a result event")["num_turns"].as_u64().unwrap() > 0);
        let rate = rate.expect("a rate-limit event");
        assert_eq!(rate["status"], "allowed");
        assert!(rate["unifiedWindows"]["five_hour"]["utilization"].as_f64().is_some());
    }
}
