//! What every game's agent benchmark shares (P23): a sweep of runs, each played by headless Claude Code (`claude -p`)
//! through the game's MCP server or by a native player, its evidence checked and analysed, and the results file
//! rewritten after every run. A game plugs in through [`Game`]: its server, its tools, its native players, how a
//! record is analysed and how runs are summarised. *Escape the Box* (`box-bench`) and *Out of the Box*
//! (`puzzle-bench`) are the two.
//!
//! Runs go on the Claude subscription (David: "Use my Claude subscription"; "try to use the max if you can"):
//! `ANTHROPIC_API_KEY` and `ANTHROPIC_AUTH_TOKEN` are removed from the agent's environment, and a session that
//! reports any API-key source stops the sweep. `CLAUDE_CODE_DISABLE_CLAUDE_MDS` keeps David's personal CLAUDE.md out
//! of the agent's context; the agent gets only the game's tools, which its transcript must confirm. A run counts only
//! once its record replays (each game's analysis says whether it did).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// One run as a results file reports it: which run (`key`, the game's), where its evidence is, its analysis, and the
/// session's own figures.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunResult<K, A> {
    #[serde(flatten)]
    pub key: K,
    /// Where its evidence is, relative to the results file.
    pub dir: String,
    pub analysis: Option<A>,
    /// Why the run has no analysis (the agent never connected, the record is missing, ...).
    pub error: Option<String>,
    pub wall_seconds: f32,
    /// Claude Code's own figure: a list-price estimate, not what a subscription is charged.
    pub reported_cost_usd: f64,
    pub agent_turns: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// The plan's five-hour usage after the run, 0..1, as the session's last rate-limit event reported it.
    #[serde(default)]
    pub plan_used: Option<f64>,
}

/// One run a sweep makes.
#[derive(Clone, Debug)]
pub struct Run<K> {
    pub key: K,
    /// Its directory in the sweep's.
    pub dir: String,
    /// The game server's arguments, apart from `--trace`, which the sweep adds.
    pub server_args: Vec<String>,
    /// The task, as the agent is given it.
    pub prompt: String,
}

/// A game behind the bench.
pub trait Game {
    type Key: Serialize + DeserializeOwned + Clone;
    type Record: DeserializeOwned + Serialize;
    type Analysis: Serialize + DeserializeOwned + Clone;
    /// The bench's own name, for its messages.
    const BENCH: &'static str;
    /// The MCP server's name in the agent's config (its tools are `mcp__<SERVER>__*`) and its binary, built beside
    /// the bench's.
    const SERVER: &'static str;
    const BINARY: &'static str;
    /// The tools the agent may call, unprefixed.
    fn tools(&self) -> Vec<String>;
    /// Whether `model` names a native player rather than an LLM.
    fn native(&self, model: &str) -> bool;
    /// A native player's run, as a record like a session's; `None` for a player the game doesn't have.
    fn play(&self, model: &str, run: &Run<Self::Key>) -> Option<Self::Record>;
    /// Replays a record and measures the play.
    fn analyse(&self, record: &Self::Record) -> Self::Analysis;
    /// One line on a run, for the console.
    fn line(&self, analysis: &Self::Analysis) -> String;
    /// The sweep's headline numbers.
    fn summarise(&self, runs: &[RunResult<Self::Key, Self::Analysis>]) -> Value;
}

/// A sweep's settings beyond its runs.
pub struct Sweep {
    pub model: String,
    /// The model as the sweep's directory names it.
    pub label: String,
    /// Where the sweep's directory goes (`<out>/<stamp>-<model>/`).
    pub out: PathBuf,
    /// Seconds before a session is killed.
    pub timeout: u64,
    /// Claude Code's `--max-budget-usd`, a guard on its list-price estimate.
    pub budget_usd: f64,
    pub system: String,
    /// More than one LLM run needs David's approval: the runs use his plan.
    pub allow_sweep: bool,
    /// The results file's schema name and the game's own settings, kept beside the runs.
    pub schema: &'static str,
    pub settings: Value,
}

/// Plays every run, then returns their results; `results.json` in the sweep's directory is rewritten after each. A
/// rate-limited session ends the sweep early; a session that billed an API key or had a stray tool ends the process.
pub fn sweep<G: Game>(game: &G, s: &Sweep, runs: &[Run<G::Key>]) -> Vec<RunResult<G::Key, G::Analysis>> {
    let fail = |m: &str| -> ! { fail(G::BENCH, m) };
    let native = game.native(&s.model);
    if !native && !s.allow_sweep && runs.len() > 1 {
        fail("more than one run needs --allow-sweep (David approves sweeps: they use his plan)");
    }
    let server = sibling(G::BINARY);
    if !native && !server.exists() {
        fail(&format!("{} not found: build it first", server.display()));
    }
    // Absolute: the agent runs from inside its run directory, and the server writes its record by this path.
    let label = s.label.replace(['/', ':', ' ', '\\'], "_");
    let dir = std::path::absolute(s.out.join(format!("{}-{label}", stamp())))
        .unwrap_or_else(|e| fail(&format!("resolving --out: {e}")));
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| fail(&format!("{}: {e}", dir.display())));
    let mut results = Vec::new();
    for run in runs {
        let at = dir.join(&run.dir);
        std::fs::create_dir_all(&at).expect("a run directory");
        eprintln!("{}: {} ({})", G::BENCH, run.dir, s.model);
        let (mut r, limited) =
            if native { (play_native(game, s, run, &at), false) } else { session(game, s, run, &at, &server) };
        r.dir = run.dir.clone();
        match (&r.analysis, &r.error) {
            (Some(a), _) => eprintln!("  {}", game.line(a)),
            (None, Some(e)) => eprintln!("  no result: {e}"),
            _ => {}
        }
        if let Some(used) = r.plan_used {
            eprintln!("  plan: {:.0}% of the five-hour window used", used * 100.0);
        }
        results.push(r);
        write(game, s, &dir, &results);
        if limited {
            eprintln!("{}: the plan's rate limit refused the session: stopping the sweep here", G::BENCH);
            break;
        }
    }
    eprintln!("{}: {} runs written to {}", G::BENCH, results.len(), dir.join("results.json").display());
    results
}

fn empty<G: Game>(key: &G::Key) -> RunResult<G::Key, G::Analysis> {
    RunResult {
        key: key.clone(),
        dir: String::new(),
        analysis: None,
        error: None,
        wall_seconds: 0.0,
        reported_cost_usd: 0.0,
        agent_turns: 0,
        input_tokens: 0,
        output_tokens: 0,
        plan_used: None,
    }
}

/// A native player's run: played here, its record kept beside it like a session's.
fn play_native<G: Game>(game: &G, s: &Sweep, run: &Run<G::Key>, dir: &Path) -> RunResult<G::Key, G::Analysis> {
    let started = Instant::now();
    let record = game.play(&s.model, run).unwrap_or_else(|| fail(G::BENCH, &format!("no player {:?}", s.model)));
    std::fs::write(dir.join("record.json"), serde_json::to_string_pretty(&record).unwrap() + "\n").unwrap();
    RunResult {
        analysis: Some(game.analyse(&record)),
        wall_seconds: started.elapsed().as_secs_f32(),
        ..empty::<G>(&run.key)
    }
}

/// One agent session: launch, wait (killing it past the timeout), then check and analyse its evidence. Also says
/// whether the plan's rate limit refused it, which ends the sweep.
fn session<G: Game>(
    game: &G,
    s: &Sweep,
    run: &Run<G::Key>,
    dir: &Path,
    server: &Path,
) -> (RunResult<G::Key, G::Analysis>, bool) {
    let record_path = dir.join("record.json");
    let mut args = run.server_args.clone();
    args.extend(["--trace".to_owned(), record_path.display().to_string()]);
    let config = json!({ "mcpServers": { G::SERVER: { "command": server, "args": args } } });
    std::fs::write(dir.join("mcp.json"), serde_json::to_string_pretty(&config).unwrap() + "\n").unwrap();
    let tools: Vec<String> = game.tools().iter().map(|t| format!("mcp__{}__{t}", G::SERVER)).collect();

    let transcript = std::fs::File::create(dir.join("transcript.jsonl")).unwrap();
    let mut cmd = Command::new("claude");
    cmd.current_dir(dir)
        .args(["-p", &run.prompt, "--model", &s.model, "--mcp-config", "mcp.json", "--strict-mcp-config"])
        .args(["--tools", "", "--allowedTools", &tools.join(",")])
        .args(["--system-prompt", &s.system, "--setting-sources", "project", "--no-session-persistence"])
        .args(["--output-format", "stream-json", "--verbose", "--max-budget-usd", &s.budget_usd.to_string()])
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("ANTHROPIC_AUTH_TOKEN")
        .env("CLAUDE_CODE_DISABLE_CLAUDE_MDS", "1")
        .env("CLAUDE_CODE_DISABLE_AUTO_MEMORY", "1")
        .stdin(Stdio::null())
        .stdout(transcript)
        .stderr(Stdio::from(std::fs::File::create(dir.join("stderr.txt")).unwrap()));
    let started = Instant::now();
    let mut child = cmd.spawn().unwrap_or_else(|e| fail(G::BENCH, &format!("starting claude: {e}")));
    let mut timed_out = false;
    loop {
        if child.try_wait().expect("waiting on claude").is_some() {
            break;
        }
        if started.elapsed() > Duration::from_secs(s.timeout) {
            let _ = child.kill();
            let _ = child.wait();
            timed_out = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    let wall_seconds = started.elapsed().as_secs_f32();

    let transcript = Transcript::read(&std::fs::read_to_string(dir.join("transcript.jsonl")).unwrap_or_default());
    let Checked { stop, limited, connected } = transcript.check(G::SERVER);
    if let Some(why) = stop {
        fail(G::BENCH, &format!("{why}: stopping the sweep"));
    }
    let Transcript { result, rate, .. } = transcript;

    let usage = result.as_ref().map(|r| &r["usage"]);
    let tokens = |k: &str| usage.and_then(|u| u[k].as_u64()).unwrap_or(0);
    let mut out = RunResult {
        wall_seconds,
        reported_cost_usd: result.as_ref().and_then(|r| r["total_cost_usd"].as_f64()).unwrap_or(0.0),
        agent_turns: result.as_ref().and_then(|r| r["num_turns"].as_u64()).unwrap_or(0) as u32,
        input_tokens: tokens("input_tokens")
            + tokens("cache_read_input_tokens")
            + tokens("cache_creation_input_tokens"),
        output_tokens: tokens("output_tokens"),
        plan_used: rate.as_ref().and_then(|r| r["unifiedWindows"]["five_hour"]["utilization"].as_f64()),
        ..empty::<G>(&run.key)
    };
    // A game server that didn't start is the harness's failure, not the agent's: no result, and said so.
    if !connected {
        out.error = Some("the game server didn't connect (see stderr.txt and the transcript's init event)".into());
        return (out, limited);
    }
    match std::fs::read_to_string(&record_path).ok().and_then(|t| serde_json::from_str::<G::Record>(&t).ok()) {
        Some(record) => out.analysis = Some(game.analyse(&record)),
        None => {
            out.error = Some(if timed_out {
                format!("timed out after {} s with no record", s.timeout)
            } else {
                "no record: the agent never reached the game".into()
            })
        }
    }
    if timed_out && out.error.is_none() {
        out.error = Some(format!("timed out after {} s (analysed up to there)", s.timeout));
    }
    (out, limited)
}

fn write<G: Game>(game: &G, s: &Sweep, dir: &Path, results: &[RunResult<G::Key, G::Analysis>]) {
    let mut doc = json!({
        "schema": s.schema,
        "game_commit": output("git", &["rev-parse", "HEAD"]),
        "game_dirty": !output("git", &["status", "--porcelain"]).is_empty(),
        "claude_version": output("claude", &["--version"]),
        "agent": format!("claude -p (Claude Code, headless) through {}; subscription billing", G::BINARY),
        "model": s.model,
        "system_prompt": s.system,
    });
    if let (Some(doc), Some(settings)) = (doc.as_object_mut(), s.settings.as_object()) {
        doc.extend(settings.clone());
        doc.insert("summary".into(), game.summarise(results));
        doc.insert("runs".into(), serde_json::to_value(results).expect("results serialise"));
    }
    std::fs::write(dir.join("results.json"), serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
}

/// A command's trimmed stdout, empty if it can't run.
pub fn output(cmd: &str, args: &[&str]) -> String {
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
pub fn stamp() -> String {
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

/// Exits with a message, as the benches do on a bad argument or a broken guard.
pub fn fail(bench: &str, message: &str) -> ! {
    eprintln!("{bench}: {message}");
    std::process::exit(2)
}

/// What an agent session's stream-json transcript holds for the harness: its first init event (model, tools, billing
/// source, servers), its final result event, and its last rate-limit info.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Transcript {
    pub init: Option<Value>,
    pub result: Option<Value>,
    pub rate: Option<Value>,
}

/// What a transcript says about the harness, checked before the run's record is read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checked {
    /// Why the sweep must stop at once: the session billed an API key rather than the subscription, or the agent had
    /// a tool besides the game's.
    pub stop: Option<String>,
    /// The plan's rate limit refused the session, which ends the sweep after this run.
    pub limited: bool,
    /// The game server connected. When it didn't, the run is the harness's failure, not the agent's.
    pub connected: bool,
}

impl Transcript {
    pub fn read(text: &str) -> Transcript {
        let mut t = Transcript::default();
        for e in text.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()) {
            match (e["type"].as_str(), e["subtype"].as_str()) {
                // The first init is the session's own (later ones, after compaction or a resume, list no servers).
                (Some("system"), Some("init")) if t.init.is_none() => t.init = Some(e),
                (Some("result"), _) => t.result = Some(e),
                (Some("rate_limit_event"), _) => t.rate = Some(e["rate_limit_info"].clone()),
                _ => {}
            }
        }
        t
    }

    /// The harness's checks on a session whose game server is named `server` (its tools are `mcp__<server>__*`).
    pub fn check(&self, server: &str) -> Checked {
        let init = self.init.as_ref();
        let billed = init.and_then(|i| i["apiKeySource"].as_str()).filter(|s| *s != "none");
        let prefix = format!("mcp__{server}__");
        let tools = init.and_then(|i| i["tools"].as_array()).into_iter().flatten().filter_map(Value::as_str);
        let stray = tools.into_iter().find(|t| !t.starts_with(&prefix));
        let stop = match (billed, stray) {
            (Some(source), _) => Some(format!("the session used an API key ({source}), not the subscription")),
            (None, Some(tool)) => Some(format!("the agent had a tool besides the game's ({tool})")),
            (None, None) => None,
        };
        let limited = self.rate.as_ref().is_some_and(|r| r["status"] == "rejected")
            || self.result.as_ref().is_some_and(|r| r["api_error_status"] == 429);
        let connected = init
            .and_then(|i| i["mcp_servers"].as_array())
            .is_some_and(|all| all.iter().any(|s| s["name"] == server && s["status"] == "connected"));
        Checked { stop, limited, connected }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transcript() -> Transcript {
        // The first capped run's transcript, as Claude Code 2.1.303 wrote it.
        let path =
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../bench/results/2026-10-19-2134-haiku/s1-t1/transcript.jsonl");
        Transcript::read(&std::fs::read_to_string(path).expect(path))
    }

    #[test]
    fn a_real_transcript_passes_every_check() {
        let t = transcript();
        assert!(t.result.as_ref().expect("a result event")["num_turns"].as_u64().unwrap() > 0);
        let rate = t.rate.as_ref().expect("a rate-limit event");
        assert!(rate["unifiedWindows"]["five_hour"]["utilization"].as_f64().is_some());
        assert_eq!(t.check("box"), Checked { stop: None, limited: false, connected: true });
    }

    #[test]
    fn an_api_key_session_stops_the_sweep() {
        let mut t = transcript();
        t.init.as_mut().unwrap()["apiKeySource"] = json!("ANTHROPIC_API_KEY");
        assert!(t.check("box").stop.unwrap().contains("API key (ANTHROPIC_API_KEY)"));
    }

    #[test]
    fn a_tool_besides_the_games_stops_the_sweep() {
        let mut t = transcript();
        t.init.as_mut().unwrap()["tools"].as_array_mut().unwrap().push(json!("Bash"));
        assert!(t.check("box").stop.unwrap().contains("(Bash)"));
        assert!(transcript().check("puzzle").stop.is_some(), "another server's name makes every box tool a stray");
    }

    #[test]
    fn a_server_that_failed_to_start_is_not_connected() {
        let mut t = transcript();
        t.init.as_mut().unwrap()["mcp_servers"] = json!([{ "name": "box", "status": "failed" }]);
        assert_eq!(t.check("box"), Checked { stop: None, limited: false, connected: false });
        assert!(!Transcript::default().check("box").connected, "no init event, nothing connected");
    }

    #[test]
    fn a_rejected_rate_limit_or_a_429_ends_the_sweep() {
        let mut t = transcript();
        t.rate.as_mut().unwrap()["status"] = json!("rejected");
        assert!(t.check("box").limited);
        let mut t = transcript();
        t.result.as_mut().unwrap()["api_error_status"] = json!(429);
        assert!(t.check("box").limited);
    }

    #[test]
    fn only_the_first_init_counts() {
        let first = r#"{"type":"system","subtype":"init","apiKeySource":"none","tools":[],"mcp_servers":[{"name":"box","status":"connected"}]}"#;
        let later = r#"{"type":"system","subtype":"init","apiKeySource":"none","tools":[],"mcp_servers":[]}"#;
        assert!(Transcript::read(&format!("{first}\nnot json\n{later}")).check("box").connected);
    }

    #[test]
    fn a_run_key_flattens_into_the_result() {
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        struct Key {
            seed: u32,
            trial: u32,
        }
        let r: RunResult<Key, Value> = RunResult {
            key: Key { seed: 3, trial: 1 },
            dir: "s3-t1".into(),
            analysis: None,
            error: None,
            wall_seconds: 1.0,
            reported_cost_usd: 0.0,
            agent_turns: 0,
            input_tokens: 0,
            output_tokens: 0,
            plan_used: None,
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!((v["seed"].clone(), v["trial"].clone()), (json!(3), json!(1)));
        assert_eq!(serde_json::from_value::<RunResult<Key, Value>>(v).unwrap(), r);
    }
}
