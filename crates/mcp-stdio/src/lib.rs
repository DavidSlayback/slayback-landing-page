//! The MCP plumbing a game server needs, hand-rolled on stdio: JSON-RPC (initialize, picking the client's protocol
//! version when it is one we know; ping; tools/list; tools/call; anything else refused), the session's cap on tool
//! calls (a benchmark's turn budget), and the loop that reads a message a line and rewrites the record after each.
//! A game supplies its name, instructions and tools and plays each call ([`Game`]); Escape the Box (`box-mcp`) and
//! *Out of the Box* (`puzzle-mcp`) are the two.

use std::io::{BufRead, Write};
use std::path::Path;

use serde_json::{Value, json};

/// Protocol versions we answer in, newest first: the client's own when it asks for one of these, else the newest.
const VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// What a call returns: the text items of its reply, `Err` when the agent made a mistake or the game refused.
pub type Reply = Result<Vec<String>, Vec<String>>;

/// A game behind the server.
pub trait Game {
    /// `serverInfo.name`.
    fn name(&self) -> &'static str;
    /// What the agent is told once, at the start.
    fn instructions(&self) -> String;
    /// The `tools/list` array.
    fn tools(&self) -> Value;
    /// The game's state for the agent, `calls_left` being the calls the cap still allows (`None`: no cap).
    fn status(&self, calls_left: Option<u32>) -> Value;
    /// Plays one tool call.
    fn call(&mut self, tool: &str, args: &Value, calls_left: Option<u32>) -> Reply;
    /// Notes a finished call and its reply for the game's record, calls refused past the cap included (they never
    /// reach `call`).
    fn note(&mut self, tool: &str, args: &Value, reply: &Reply);
}

/// One session: a game, the calls made on it and the cap on them.
pub struct Session<G> {
    pub game: G,
    calls: u32,
    max_calls: Option<u32>,
}

impl<G: Game> Session<G> {
    pub fn new(game: G) -> Session<G> {
        Session { game, calls: 0, max_calls: None }
    }

    /// At most `max` tool calls (of any tool): past it, every call is refused and the session is over. A benchmark's
    /// turn budget, since an agent's harness may have none.
    pub fn with_max_calls(mut self, max: u32) -> Session<G> {
        self.max_calls = Some(max);
        self
    }

    pub fn max_calls(&self) -> Option<u32> {
        self.max_calls
    }

    pub fn calls_left(&self) -> Option<u32> {
        self.max_calls.map(|m| m.saturating_sub(self.calls))
    }

    /// Answers one JSON-RPC message; notifications (no `id`) get no answer.
    pub fn handle(&mut self, msg: &Value) -> Option<Value> {
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        let result = match method {
            "initialize" => Ok(self.initialize(&params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": self.game.tools() })),
            "tools/call" => Ok(self.call(&params)),
            _ if id.is_none() => return None, // notifications/initialized and the like
            _ => Err((-32601, format!("method not found: {method}"))),
        };
        let id = id?;
        Some(match result {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err((code, message)) => {
                json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
            }
        })
    }

    fn initialize(&self, params: &Value) -> Value {
        let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or(VERSIONS[0]);
        let version = VERSIONS.iter().find(|v| **v == asked).copied().unwrap_or(VERSIONS[0]);
        json!({
            "protocolVersion": version,
            "capabilities": { "tools": {} },
            "serverInfo": { "name": self.game.name(), "version": env!("CARGO_PKG_VERSION") },
            "instructions": self.game.instructions(),
        })
    }

    fn call(&mut self, params: &Value) -> Value {
        let tool = params.get("name").and_then(Value::as_str).unwrap_or("");
        let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
        let reply = match self.calls_left() {
            Some(0) => {
                let refusal =
                    json!({ "error": "out of tool calls: the run is over", "status": self.game.status(Some(0)) });
                Err(vec![refusal.to_string()])
            }
            left => self.game.call(tool, &args, left),
        };
        let error = reply.is_err();
        self.game.note(tool, &args, &reply);
        self.calls += 1;
        let content: Vec<Value> = match reply {
            Ok(texts) | Err(texts) => texts.into_iter().map(|text| json!({ "type": "text", "text": text })).collect(),
        };
        json!({ "content": content, "isError": error })
    }
}

/// Serves `session` on stdio until stdin closes: a message a line in, a reply a line out. With `trace`, `record` is
/// written there after every message, so a session cut off mid-run still leaves it.
pub fn serve<G: Game>(session: &mut Session<G>, trace: Option<&Path>, record: impl Fn(&Session<G>) -> Value) {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => session.handle(&msg),
            Err(e) => {
                Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": e.to_string() } }))
            }
        };
        if let Some(reply) = reply {
            writeln!(out, "{reply}").and_then(|_| out.flush()).expect("stdout");
        }
        if let Some(path) = trace {
            let text = serde_json::to_string_pretty(&record(session)).expect("a record serialises") + "\n";
            std::fs::write(path, text).unwrap_or_else(|e| fail("mcp", &format!("writing {}: {e}", path.display())));
        }
    }
}

/// Exits with a message, as a server does on a bad argument.
pub fn fail(server: &str, message: &str) -> ! {
    eprintln!("{server}: {message}");
    std::process::exit(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Echo(Vec<(String, bool)>);

    impl Game for Echo {
        fn name(&self) -> &'static str {
            "echo"
        }
        fn instructions(&self) -> String {
            "Say something.".into()
        }
        fn tools(&self) -> Value {
            json!([{ "name": "say" }])
        }
        fn status(&self, calls_left: Option<u32>) -> Value {
            json!({ "calls_left": calls_left })
        }
        fn call(&mut self, tool: &str, args: &Value, _: Option<u32>) -> Reply {
            if tool == "say" { Ok(vec![args.to_string()]) } else { Err(vec![format!("no tool {tool}")]) }
        }
        fn note(&mut self, tool: &str, _: &Value, reply: &Reply) {
            self.0.push((tool.into(), reply.is_err()));
        }
    }

    fn call(s: &mut Session<Echo>, tool: &str) -> Value {
        s.handle(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": tool } })).unwrap()
    }

    #[test]
    fn it_answers_in_the_clients_protocol_version_when_it_knows_it() {
        let mut s = Session::new(Echo(vec![]));
        let init = |s: &mut Session<Echo>, v: &str| {
            let msg = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": v } });
            s.handle(&msg).unwrap()["result"]["protocolVersion"].clone()
        };
        assert_eq!(init(&mut s, "2025-03-26"), "2025-03-26");
        assert_eq!(init(&mut s, "1999-01-01"), VERSIONS[0]);
    }

    #[test]
    fn past_the_cap_every_call_is_refused_and_noted() {
        let mut s = Session::new(Echo(vec![])).with_max_calls(1);
        assert_eq!(call(&mut s, "say")["result"]["isError"], false);
        let refused = call(&mut s, "say");
        assert_eq!(refused["result"]["isError"], true);
        assert!(refused["result"]["content"][0]["text"].as_str().unwrap().contains("out of tool calls"));
        assert_eq!(s.game.0, [("say".to_string(), false), ("say".to_string(), true)]);
        assert_eq!(s.calls_left(), Some(0));
    }

    #[test]
    fn notifications_get_no_answer_and_unknown_methods_an_error() {
        let mut s = Session::new(Echo(vec![]));
        assert_eq!(s.handle(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })), None);
        let r = s.handle(&json!({ "jsonrpc": "2.0", "id": 3, "method": "resources/list" })).unwrap();
        assert_eq!(r["error"]["code"], -32601);
    }
}
