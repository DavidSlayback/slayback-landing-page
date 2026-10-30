//! *Escape the Box* as an MCP server (ML track M5, `docs/planning/ml-track.md`): an LLM agent plays one run of the
//! headless game ([`box_env::Env`]) through the tools in `tools.json`, turn by turn. The game stands still between
//! calls; each action tool advances it a bounded time and reports what happened.
//!
//! The protocol is MCP over stdio, `mcp_stdio`'s, shared with *Out of the Box*'s server; this crate is the game behind
//! it ([`Escape`]). [`Server::handle`] answers one message; `main.rs` serves stdio. Every step the agent takes is in the env's trace, so a session's [`Record`] replays to the same outcome
//! (`box_env::replay`), which is how a benchmark result is checked.

use box_env::{Action, Env, EnvConfig, Events, FPS, Outcome, Trace, obs_json};
use game_core::agent::{COMPASS, DASH, GRAB, KICK, PUNCH};
use game_core::climb::MAX_STEP;
use game_core::run::{AT_WALL, REFILL, TIMEOUT_PENALTY};
use mcp_stdio::{Game, Reply, Session};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The tool definitions, shared with the page's WebMCP tools (M8).
pub const TOOLS: &str = include_str!("../tools.json");

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

/// The server: the game behind `mcp_stdio`'s session, which counts the calls against the cap.
pub struct Server(Session<Escape>);

impl Server {
    pub fn new(config: EnvConfig) -> Server {
        Server(Session::new(Escape { env: Env::new(config), calls: Vec::new(), noted: 0 }))
    }

    /// At most `max` tool calls (of any tool): past it, every call is refused and the run counts as over. A
    /// benchmark's turn budget, since an agent's harness may have none.
    pub fn with_max_calls(self, max: u32) -> Server {
        Server(self.0.with_max_calls(max))
    }

    /// The session so far.
    pub fn record(&self) -> Record {
        record(&self.0)
    }

    /// Answers one JSON-RPC message; notifications (no `id`) get no answer.
    pub fn handle(&mut self, msg: &Value) -> Option<Value> {
        self.0.handle(msg)
    }

    /// Serves stdio until it closes, writing the record to `trace` after every message.
    pub fn serve(&mut self, trace: Option<&std::path::Path>) {
        mcp_stdio::serve(&mut self.0, trace, |s| serde_json::to_value(record(s)).expect("a record serialises"));
    }
}

fn record(session: &Session<Escape>) -> Record {
    let game = &session.game;
    Record {
        trace: game.env.trace(),
        outcome: game.env.outcome(),
        calls: game.calls.clone(),
        max_calls: session.max_calls(),
    }
}

/// One run of the game and the calls made on it.
pub struct Escape {
    env: Env,
    calls: Vec<Call>,
    /// The env's trace length when the last call was noted: the steps a call advanced are those past it.
    noted: u32,
}

impl Game for Escape {
    fn name(&self) -> &'static str {
        "escape-the-box"
    }

    fn instructions(&self) -> String {
        instructions()
    }

    fn tools(&self) -> Value {
        tools()
    }

    fn status(&self, calls_left: Option<u32>) -> Value {
        self.status(calls_left)
    }

    fn call(&mut self, tool: &str, args: &Value, calls_left: Option<u32>) -> Reply {
        match self.run(tool, args, calls_left) {
            Ok(v) => Ok(vec![v.to_string()]),
            Err(v) => Err(vec![v.to_string()]),
        }
    }

    fn note(&mut self, tool: &str, args: &Value, reply: &Reply) {
        let (len, error) = (self.env.trace().actions.len() as u32, reply.is_err());
        self.calls.push(Call { tool: tool.to_owned(), arguments: args.clone(), steps: len - self.noted, error });
        self.noted = len;
    }
}

impl Escape {
    fn run(&mut self, tool: &str, args: &Value, calls_left: Option<u32>) -> Result<Value, Value> {
        match tool {
            "look" => Ok(json!({ "status": self.status(calls_left), "observation": obs_json(self.env.last()) })),
            "status" => Ok(self.status(calls_left)),
            _ if !TOOL_NAMES.contains(&tool) => Err(json!({ "error": format!("no tool named {tool:?}") })),
            _ if self.env.done() => {
                Err(json!({ "error": "the run is over: no more actions", "status": self.status(calls_left) }))
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

    fn status(&self, calls_left: Option<u32>) -> Value {
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
            "run_over": self.env.done() || calls_left == Some(0),
            "calls_left": calls_left,
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
}
