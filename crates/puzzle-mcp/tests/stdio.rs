//! The server as a client sees it: the binary on stdio, every tool called, the budgets enforced, and the session's
//! record replaying to its outcome against `puzzle-core`.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use puzzle_core::{TIERS, World, generate};
use puzzle_mcp::{Record, replay};
use serde_json::{Value, json};

struct Client {
    child: std::process::Child,
    out: BufReader<std::process::ChildStdout>,
    next: u64,
}

impl Client {
    fn start(args: &[&str]) -> Client {
        // Read when the test runs: cargo sets it then, not when the test compiles.
        let exe = std::env::var("CARGO_BIN_EXE_puzzle-mcp").expect("cargo test sets CARGO_BIN_EXE_puzzle-mcp");
        let mut child = Command::new(exe)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("the server starts");
        let out = BufReader::new(child.stdout.take().unwrap());
        let mut c = Client { child, out, next: 1 };
        c.request("initialize", json!({ "protocolVersion": "2025-06-18" }));
        c
    }

    fn send(&mut self, msg: Value) {
        let stdin = self.child.stdin.as_mut().unwrap();
        writeln!(stdin, "{msg}").unwrap();
        stdin.flush().unwrap();
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        let mut line = String::new();
        self.out.read_line(&mut line).expect("a reply");
        let reply: Value = serde_json::from_str(&line).unwrap_or_else(|e| panic!("not JSON ({e}): {line}"));
        assert_eq!(reply["id"], id, "{reply}");
        reply
    }

    /// Calls `tool`: its reply's texts and whether it was an error.
    fn call(&mut self, tool: &str, arguments: Value) -> (Vec<String>, bool) {
        let r = self.request("tools/call", json!({ "name": tool, "arguments": arguments }));
        let result = &r["result"];
        let texts = result["content"].as_array().unwrap().iter().map(|c| c["text"].as_str().unwrap().to_owned());
        (texts.collect(), result["isError"].as_bool().unwrap())
    }

    fn finish(mut self) {
        drop(self.child.stdin.take());
        assert!(self.child.wait().unwrap().success());
    }
}

fn trace_file(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("puzzle-mcp-{name}-{}.json", std::process::id()))
}

fn read(path: &std::path::Path) -> Record {
    let record = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let _ = std::fs::remove_file(path);
    record
}

#[test]
fn a_session_calls_every_tool_solves_the_level_and_its_record_replays() {
    let solution: String =
        generate(1, World::Classic, &TIERS[0]).unwrap().solution.iter().map(|k| format!("{k:?}")).collect();
    let trace = trace_file("solve");
    let mut c = Client::start(&["--seed", "1", "--json", "--trace", trace.to_str().unwrap()]);
    let init = c.request("initialize", json!({ "protocolVersion": "2025-11-25" }));
    assert!(init["result"]["instructions"].as_str().unwrap().contains("# wall"));
    c.send(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
    assert_eq!(c.request("ping", json!({}))["result"], json!({}));
    let names: Vec<String> = c.request("tools/list", json!({}))["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(names, ["look", "move", "undo", "restart", "status"]);

    let (look, err) = c.call("look", json!({}));
    assert!(!err && look[0].starts_with("Classic world, tier 1.") && look[0].contains("Moves on the board: 0 of"));
    let obs: Value = serde_json::from_str(&look[1]).unwrap();
    assert_eq!(obs["status"]["calls_left"], 29, "{obs}");

    let (moved, err) = c.call("move", json!({ "keys": "XR" }));
    assert!(!err && moved[0].starts_with("Played 0 of 2 key(s): key 1 ('X') was refused: it is not L, R, U or D."));
    let (moved, _) = c.call("move", json!({ "keys": &solution[..2] }));
    assert!(moved[0].starts_with("Played 2 of 2 key(s)."), "{moved:?}");
    let (undone, _) = c.call("undo", json!({ "n": 5 }));
    assert!(undone[0].starts_with("Took back 2 move(s)."));
    c.call("move", json!({ "keys": "R" }));
    let (restarted, _) = c.call("restart", json!({}));
    assert!(restarted[0].contains("Moves on the board: 0 of"));
    let (solved, _) = c.call("move", json!({ "keys": solution }));
    assert!(solved[0].contains(": solved!") && solved[0].contains("Solved: the session is over."), "{solved:?}");
    let (status, err) = c.call("status", json!({}));
    assert!(!err && status[0].contains("Solved"), "{status:?}");

    // Mistakes are tool errors the agent reads, not protocol failures.
    let (r, err) = c.call("move", json!({ "keys": "L" }));
    assert!(err && r[0].starts_with("The session is over"));
    let (_, err) = c.call("fly", json!({}));
    assert!(err);
    assert_eq!(c.request("resources/list", json!({}))["error"]["code"], -32601);
    c.finish();

    let record = read(&trace);
    assert_eq!((record.world.as_str(), record.tier, record.seed), ("classic", 1, 1));
    assert_eq!(record.calls.len(), 10, "every tools/call is kept, mistakes too");
    assert!(record.outcome.solved && record.outcome.moves == solution.len());
    assert_eq!(replay(&record), Ok(record.outcome), "the session replays to its outcome");
}

#[test]
fn filling_the_board_unsolved_ends_the_session() {
    let trace = trace_file("budget");
    let mut c = Client::start(&["--world", "ice", "--tier", "1", "--seed", "2", "--trace", trace.to_str().unwrap()]);
    let budget = read_budget(&mut c);
    // Turning on the spot always plays and never solves.
    let (r, err) = c.call("move", json!({ "keys": "LR".repeat(budget) }));
    assert!(!err && r[0].contains(&format!("the board holds all {budget} allowed moves, unsolved")), "{r:?}");
    assert!(r[0].contains("The board is full, unsolved: the session is over."));
    let (r, err) = c.call("undo", json!({}));
    assert!(err && r[0].starts_with("The session is over"));
    c.finish();
    let record = read(&trace);
    assert_eq!(record.outcome.moves, budget);
    assert_eq!(replay(&record), Ok(record.outcome));
}

fn read_budget(c: &mut Client) -> usize {
    let (look, _) = c.call("look", json!({}));
    let at = look[0].find("Moves on the board: 0 of ").unwrap() + "Moves on the board: 0 of ".len();
    look[0][at..].split(' ').next().unwrap().parse().unwrap()
}

#[test]
fn past_the_call_cap_every_call_is_refused_and_the_record_says_so() {
    let trace = trace_file("cap");
    let mut c = Client::start(&["--max-calls", "2", "--trace", trace.to_str().unwrap()]);
    let (r, err) = c.call("status", json!({}));
    assert!(!err && r[0].contains("Tool calls left: 1."), "{r:?}");
    c.call("move", json!({ "keys": "L" }));
    let (r, err) = c.call("look", json!({}));
    assert!(err && r[0].contains("out of tool calls"), "{r:?}");
    c.finish();
    let record = read(&trace);
    assert_eq!(record.max_calls, Some(2));
    assert_eq!(record.calls.len(), 3);
    assert!(record.calls[2].error);
}

#[test]
fn a_record_for_another_level_does_not_replay() {
    let trace = trace_file("wrong");
    let c = Client::start(&["--trace", trace.to_str().unwrap()]);
    c.finish();
    let mut record = read(&trace);
    record.seed = 2;
    assert!(replay(&record).unwrap_err().contains("not"));
}

#[test]
fn the_kept_smoke_session_replays() {
    let path = format!("{}/../../bench/puzzle/smoke/2026-10-29-ob4-haiku/record.json", env!("CARGO_MANIFEST_DIR"));
    let record: Record = serde_json::from_str(&std::fs::read_to_string(&path).expect(&path)).unwrap();
    assert!(record.outcome.solved);
    assert_eq!(replay(&record), Ok(record.outcome));
}
