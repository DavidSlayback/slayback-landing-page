//! *Escape the Box* as an MCP server (ML track M5, `docs/planning/ml-track.md`): an LLM agent plays one run of the
//! headless game ([`box_env::Env`]) through the tools in `tools.json`, turn by turn. The game stands still between
//! calls; each action tool advances it a bounded time and reports what happened.
//!
//! The protocol is MCP over stdio (newline-delimited JSON-RPC 2.0), just the parts a tools-only server needs:
//! `initialize`, `ping`, `tools/list` and `tools/call`. [`Server::handle`] is the whole of it; `main.rs` feeds it
//! lines. Every step the agent takes is in the env's trace, so a session's [`Record`] replays to the same outcome
//! (`box_env::replay`), which is how a benchmark result is checked.

use box_env::{Action, Env, EnvConfig, Events, FPS, Outcome, Trace, obs_json};
use game_core::agent::{COMPASS, DASH, GRAB, KICK, PUNCH};
use game_core::climb::MAX_STEP;
use game_core::run::{AT_WALL, REFILL, TIMEOUT_PENALTY};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The tool definitions, shared with the page's WebMCP tools (M8).
pub const TOOLS: &str = include_str!("../tools.json");

/// Protocol versions this server speaks, newest first. It answers a client in the client's own version when listed.
const VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// What the server tells the agent once, at the start: the game's rules and conventions, its numbers taken from the
/// game.
pub fn instructions() -> String {
    format!(
        "You are the fighter in Escape the Box, a 3D brawler seen from above. Enemies keep coming through four doors \
         (two in the back wall, one in each side). Defeated enemies burst into debris blocks that freeze where they \
         land. You escape a box when the pile of debris under your feet reaches the rim's height while you stand \
         against a wall (within {AT_WALL} m of it: a pile out in the middle doesn't get you out); then the next box \
         starts (a higher rim, a busier crowd), and you keep {refill}% more health. You cannot jump: you climb by \
         walking up debris in steps of up to {MAX_STEP} m, so heap kills against a wall to build a ramp up to it. The \
         box clock running out costs {TIMEOUT_PENALTY} health and restarts it. One life: the run ends when your \
         health reaches zero. Coordinates are metres on the floor from its centre: x is east (right), y is north \
         (away from the camera, toward the back wall). The game is frozen between your tool calls; each action \
         advances it. Call look often.",
        refill = (REFILL * 100.0).round(),
    )
}

/// Steps spent turning to face an attack's direction, and recovering after it.
const TURN_STEPS: u32 = 2;
const RECOVER_STEPS: u32 = 5;

/// One tool call, as the session's record keeps it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Call {
    pub tool: String,
    pub arguments: Value,
    /// Env steps it advanced.
    pub steps: u32,
    pub error: bool,
}

/// A session so far: the env's trace (what replay checks), its outcome, and the agent's calls.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub trace: Trace,
    pub outcome: Outcome,
    pub calls: Vec<Call>,
    /// The cap on tool calls, if any: calls past it were refused (and are in `calls`, as errors).
    pub max_calls: Option<u32>,
}

/// The server: one run of the game and the calls made on it.
pub struct Server {
    env: Env,
    calls: Vec<Call>,
    max_calls: Option<u32>,
}

impl Server {
    pub fn new(config: EnvConfig) -> Server {
        Server { env: Env::new(config), calls: Vec::new(), max_calls: None }
    }

    /// At most `max` tool calls (of any tool): past it, every call is refused and the run counts as over. A
    /// benchmark's turn budget, since an agent's harness may have none.
    pub fn with_max_calls(mut self, max: u32) -> Server {
        self.max_calls = Some(max);
        self
    }

    /// The session so far.
    pub fn record(&self) -> Record {
        Record {
            trace: self.env.trace(),
            outcome: self.env.outcome(),
            calls: self.calls.clone(),
            max_calls: self.max_calls,
        }
    }

    fn calls_left(&self) -> Option<u32> {
        self.max_calls.map(|m| m.saturating_sub(self.calls.len() as u32))
    }

    /// Answers one JSON-RPC message; notifications (no `id`) get no answer.
    pub fn handle(&mut self, msg: &Value) -> Option<Value> {
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        let result = match method {
            "initialize" => Ok(initialize(&params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools() })),
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

    fn call(&mut self, params: &Value) -> Value {
        let tool = params.get("name").and_then(Value::as_str).unwrap_or("").to_owned();
        let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
        let before = self.env.trace().actions.len() as u32;
        let outcome = if self.calls_left() == Some(0) {
            Err(json!({ "error": "out of tool calls: the run is over", "status": self.status() }))
        } else {
            self.run(&tool, &args)
        };
        let steps = self.env.trace().actions.len() as u32 - before;
        let error = outcome.is_err();
        self.calls.push(Call { tool, arguments: args, steps, error });
        let text = match outcome {
            Ok(v) | Err(v) => v.to_string(),
        };
        json!({ "content": [{ "type": "text", "text": text }], "isError": error })
    }

    fn run(&mut self, tool: &str, args: &Value) -> Result<Value, Value> {
        match tool {
            "look" => Ok(json!({ "status": self.status(), "observation": obs_json(self.env.last()) })),
            "status" => Ok(self.status()),
            _ if !TOOL_NAMES.contains(&tool) => Err(json!({ "error": format!("no tool named {tool:?}") })),
            _ if self.env.done() => {
                Err(json!({ "error": "the run is over: no more actions", "status": self.status() }))
            }
            "move" => {
                let dir = direction(args)?;
                let n = steps_for(args, &self.env, 2.0)?;
                Ok(self.act(&format!("moved {} for {:.2} s", COMPASS[dir - 1], self.secs(n)), &[(dir, 0, n)]))
            }
            "wait" => {
                let n = steps_for(args, &self.env, 3.0)?;
                Ok(self.act(&format!("waited {:.2} s", self.secs(n)), &[(0, 0, n)]))
            }
            "punch" | "kick" => {
                let verb = if tool == "punch" { PUNCH } else { KICK };
                let mut plan = Vec::new();
                if args.get("direction").is_some() {
                    plan.push((direction(args)?, 0, TURN_STEPS));
                }
                plan.extend([(0, verb, 1), (0, 0, RECOVER_STEPS)]);
                Ok(self.act(&format!("{tool}ed"), &plan))
            }
            "dash" => {
                let dir = direction(args)?;
                Ok(self.act(&format!("dashed {}", COMPASS[dir - 1]), &[(dir, DASH, 1)]))
            }
            "grab" => Ok(self.act("pressed grab", &[(0, GRAB, 1), (0, 0, 2)])),
            _ => unreachable!("every listed tool is handled"),
        }
    }

    /// Plays `plan`, a list of (direction, verb, steps): the verb is pressed on the first of its steps only. Stops
    /// early if the run ends.
    fn act(&mut self, did: &str, plan: &[(usize, usize, u32)]) -> Value {
        let hp = self.env.last().player.hp;
        let (mut events, mut hurt, mut last_hp, mut steps) = (Events::default(), 0.0f32, hp, 0);
        'plan: for &(dir, verb, n) in plan {
            for i in 0..n {
                let s = self.env.step(Action::new(dir, if i == 0 { verb } else { 0 }));
                steps += 1;
                events.kills += s.events.kills;
                events.escapes += s.events.escapes;
                events.died |= s.events.died;
                hurt += (last_hp - s.obs.player.hp).max(0.0);
                last_hp = s.obs.player.hp;
                if s.done {
                    break 'plan;
                }
            }
        }
        json!({
            "did": did,
            "game_seconds": round(self.secs(steps)),
            "events": {
                "kills": events.kills,
                "boxes_escaped": events.escapes,
                "damage_taken": round(hurt),
                "died": events.died,
            },
            "run_over": self.env.done(),
            "observation": obs_json(self.env.last()),
        })
    }

    fn status(&self) -> Value {
        let o = self.env.outcome();
        let obs = self.env.last();
        json!({
            "box": obs.box_number,
            "hp": round(obs.player.hp),
            "max_hp": round(obs.player.max_hp),
            "time_left": round(obs.time_left),
            "kills": o.kills,
            "boxes_escaped": o.escapes,
            "score": o.score,
            "game_seconds": round(self.secs(o.steps)),
            "run_over": self.env.done() || self.calls_left() == Some(0),
            "calls_left": self.calls_left(),
        })
    }

    fn secs(&self, steps: u32) -> f32 {
        steps as f32 * self.env.config().frame_skip as f32 / FPS
    }
}

/// The tools in `tools.json`, in its order.
pub const TOOL_NAMES: [&str; 8] = ["look", "status", "move", "punch", "kick", "dash", "grab", "wait"];

fn tools() -> Value {
    serde_json::from_str(TOOLS).expect("tools.json is valid JSON")
}

fn initialize(params: &Value) -> Value {
    let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or(VERSIONS[0]);
    let version = VERSIONS.iter().find(|v| **v == asked).copied().unwrap_or(VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "escape-the-box", "version": env!("CARGO_PKG_VERSION") },
        "instructions": instructions(),
    })
}

/// The direction argument as an action-table direction (1..=8).
fn direction(args: &Value) -> Result<usize, Value> {
    match args.get("direction").and_then(Value::as_str) {
        Some(name) => COMPASS
            .iter()
            .position(|c| *c == name)
            .map(|k| k + 1)
            .ok_or_else(|| json!({ "error": format!("direction must be one of {COMPASS:?}, not {name:?}") })),
        None => Err(json!({ "error": "direction is required" })),
    }
}

/// The seconds argument (0.1 up to `max`) as whole env steps, at least one.
fn steps_for(args: &Value, env: &Env, max: f32) -> Result<u32, Value> {
    let secs = args.get("seconds").and_then(Value::as_f64).ok_or_else(|| json!({ "error": "seconds is required" }))?;
    if !(0.1..=max as f64).contains(&secs) {
        return Err(json!({ "error": format!("seconds must be between 0.1 and {max}") }));
    }
    let step = env.config().frame_skip as f64 / FPS as f64;
    Ok(((secs / step).round() as u32).max(1))
}

fn round(x: f32) -> f64 {
    (x as f64 * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tool_file_names_exactly_the_tools_the_server_handles() {
        let names: Vec<String> =
            tools().as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_owned()).collect();
        assert_eq!(names, TOOL_NAMES);
        for t in tools().as_array().unwrap() {
            assert_eq!(t["inputSchema"]["type"], "object", "{}", t["name"]);
            assert!(t["description"].as_str().is_some_and(|d| d.len() > 20), "{}", t["name"]);
        }
    }

    /// Built from the game's constants, the text is still exactly what the benchmark's agents were told.
    #[test]
    fn the_instructions_read_as_they_always_have() {
        let told = "You are the fighter in Escape the Box, a 3D brawler seen from above. Enemies keep coming \
through four doors (two in the back wall, one in each side). Defeated enemies burst into debris blocks that freeze \
where they land. You escape a box when the pile of debris under your feet reaches the rim's height while you stand \
against a wall (within 0.8 m of it: a pile out in the middle doesn't get you out); then the next box starts (a higher rim, a busier crowd), and you keep 30% more health. You cannot jump: you climb by walking up debris \
in steps of up to 0.4 m, so heap kills against a wall to build a ramp up to it. The box clock running out costs 25 health and \
restarts it. One life: the run ends when your health reaches zero. Coordinates are metres on the floor from its \
centre: x is east (right), y is north (away from the camera, toward the back wall). The game is frozen between your \
tool calls; each action advances it. Call look often.";
        assert_eq!(instructions(), told);
    }

    #[test]
    fn compass_names_follow_the_action_table() {
        // Direction 3 (north) is +y in plane coordinates, away from the camera; direction 1 (east) is +x.
        let (north, _) = Action::new(3, 0).decode();
        let (east, _) = Action::new(1, 0).decode();
        assert_eq!(COMPASS[2], "north");
        assert!(north[1] > 0.99 && east[0] > 0.99);
    }

    #[test]
    fn it_answers_in_the_clients_protocol_version_when_it_knows_it() {
        assert_eq!(initialize(&json!({ "protocolVersion": "2025-06-18" }))["protocolVersion"], "2025-06-18");
        assert_eq!(initialize(&json!({ "protocolVersion": "1999-01-01" }))["protocolVersion"], VERSIONS[0]);
    }
}
