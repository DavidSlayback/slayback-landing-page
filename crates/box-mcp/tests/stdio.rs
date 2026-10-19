//! The server as a client sees it: the binary on stdio, every tool called, and the session's record replaying to its
//! outcome.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use box_mcp::Record;
use serde_json::{Value, json};

struct Client {
    child: std::process::Child,
    out: BufReader<std::process::ChildStdout>,
    next: u64,
}

impl Client {
    fn start(args: &[&str]) -> Client {
        let mut child = Command::new(env!("CARGO_BIN_EXE_box-mcp"))
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("the server starts");
        let out = BufReader::new(child.stdout.take().unwrap());
        Client { child, out, next: 1 }
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

    /// Calls `tool` and returns its parsed text payload and whether it was an error.
    fn call(&mut self, tool: &str, arguments: Value) -> (Value, bool) {
        let r = self.request("tools/call", json!({ "name": tool, "arguments": arguments }));
        let result = &r["result"];
        let text = result["content"][0]["text"].as_str().unwrap_or_else(|| panic!("{r}"));
        (serde_json::from_str(text).unwrap(), result["isError"].as_bool().unwrap())
    }

    fn finish(mut self) {
        drop(self.child.stdin.take());
        assert!(self.child.wait().unwrap().success());
    }
}

#[test]
fn an_agent_session_over_stdio_calls_every_tool_and_its_record_replays() {
    let trace = std::env::temp_dir().join(format!("box-mcp-test-{}.json", std::process::id()));
    let mut c = Client::start(&["--seed", "5", "--trace", trace.to_str().unwrap()]);

    let init = c.request("initialize", json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "0" } }));
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    assert!(init["result"]["capabilities"]["tools"].is_object());
    assert!(init["result"]["instructions"].as_str().unwrap().contains("rim"));
    c.send(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
    assert_eq!(c.request("ping", json!({}))["result"], json!({}));

    let listed = c.request("tools/list", json!({}));
    let names: Vec<&str> =
        listed["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["look", "status", "move", "punch", "kick", "dash", "grab", "wait"]);

    let (look, err) = c.call("look", json!({}));
    assert!(!err);
    assert_eq!(look["status"]["box"], 1);
    assert_eq!(look["observation"]["you"]["hp"], look["observation"]["you"]["max_hp"]);
    let x0 = look["observation"]["you"]["position"][0].as_f64().unwrap();

    let (moved, err) = c.call("move", json!({ "direction": "east", "seconds": 0.5 }));
    assert!(!err, "{moved}");
    assert!((moved["game_seconds"].as_f64().unwrap() - 0.53).abs() < 0.01, "8 steps of 4 frames: {moved}");
    let x1 = moved["observation"]["you"]["position"][0].as_f64().unwrap();
    assert!(x1 - x0 > 1.5, "ran east: {x0} -> {x1}");

    for (tool, args) in [
        ("punch", json!({})),
        ("kick", json!({ "direction": "north" })),
        ("dash", json!({ "direction": "west" })),
        ("kick", json!({})),
        ("grab", json!({})),
        ("wait", json!({ "seconds": 1.0 })),
    ] {
        let (r, err) = c.call(tool, args.clone());
        assert!(!err, "{tool} {args}: {r}");
        assert!(r["events"]["kills"].is_u64() && r["observation"]["box"] == 1, "{tool}: {r}");
    }
    let (status, _) = c.call("status", json!({}));
    assert!(status["game_seconds"].as_f64().unwrap() > 2.0, "{status}");

    // Mistakes are tool errors the agent can read, not protocol failures.
    let (r, err) = c.call("move", json!({ "direction": "up", "seconds": 1 }));
    assert!(err && r["error"].as_str().unwrap().contains("north"), "{r}");
    let (r, err) = c.call("wait", json!({ "seconds": 30 }));
    assert!(err, "{r}");
    let (_, err) = c.call("fly", json!({}));
    assert!(err);
    assert!(c.request("resources/list", json!({}))["error"]["code"] == -32601);

    c.finish();
    let record: Record = serde_json::from_str(&std::fs::read_to_string(&trace).unwrap()).unwrap();
    let _ = std::fs::remove_file(&trace);
    assert_eq!(record.trace.config.seed, 5);
    assert_eq!(record.calls.len(), 12, "every tools/call is kept, the three mistakes too");
    assert_eq!(record.calls.iter().filter(|c| c.error).count(), 3);
    assert_eq!(record.calls.iter().map(|c| c.steps).sum::<u32>(), record.outcome.steps);
    assert_eq!(box_env::replay(&record.trace), record.outcome, "the session replays to its outcome");
}

#[test]
fn after_the_run_ends_actions_are_refused_but_looking_still_works() {
    // A very short run: cut off after 3 steps.
    let mut c = Client::start(&["--max-steps", "3"]);
    c.request("initialize", json!({ "protocolVersion": "2025-11-25" }));
    let (r, _) = c.call("wait", json!({ "seconds": 1.0 }));
    assert_eq!(r["run_over"], true, "{r}");
    let (r, err) = c.call("move", json!({ "direction": "north", "seconds": 0.5 }));
    assert!(err && r["error"].as_str().unwrap().contains("over"), "{r}");
    let (r, err) = c.call("look", json!({}));
    assert!(!err && r["status"]["run_over"] == true);
    c.finish();
}
