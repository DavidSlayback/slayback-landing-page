//! *Out of the Box* as an MCP server (OB4, `docs/planning/puzzle-spec.md`, "Agents through MCP"): an LLM agent plays
//! one level through the tools in `tools.json`. The level is the bench's: `generate(seed)` as it comes, untidied, so
//! suites v1 and v2 hold exactly these levels (David: "The bench's"). Budgets and undo are
//! `puzzle_core::session`'s (P10); the call cap (P21) and the protocol are `mcp_stdio`'s.
//!
//! Replies are text an agent reads: the grid as the level's text form (P4), facing, what is carried and the counters.
//! With `--json` each reply adds the same as JSON. Every call and its reply go into the session's [`Record`], which
//! [`replay`] plays back against `puzzle-core`.

use mcp_stdio::{Game, Reply, Session};
use puzzle_core::session::{self, Act, Moved};
use puzzle_core::{Facing, Level, Refusal, TIERS, World, generate};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The tool definitions.
pub const TOOLS: &str = include_str!("../tools.json");

/// The tools in `tools.json`, in its order.
pub const TOOL_NAMES: [&str; 5] = ["look", "move", "undo", "restart", "status"];

/// Tool calls a session allows (P21): a move string can carry a whole tier-4 solution.
pub const MAX_CALLS: u32 = 30;

/// What the agent is told once: the full rules and legend, with no worked example (P11), and the world's own rules.
pub fn instructions(world: World) -> String {
    let mut text = String::from(
        "You are playing Out of the Box, a turn-based puzzle. You stand in a pit of walls and loose blocks and must \
         reach the exit. The level is a grid shown as text, one line per row, y growing downward. Legend: # wall, B \
         block, E exit, . empty, > or < you, facing right or left.",
    );
    let legend = match world {
        World::Classic => "",
        World::Crumble => " ~ cracked floor.",
        World::Ice => " I ice block (i on the exit).",
        World::Springs => " ^ spring.",
        World::Mixed => " ~ cracked floor, ^ spring, I ice block (i on the exit).",
    };
    text += legend;
    text += " A block on the exit shows as b. A block you carry is drawn in the cell above you.\n\n\
Keys, played as a string with the move tool:\n\
- L or R: if you already face that way, step one cell that way, then fall until you land; otherwise turn to face it \
(a turn counts as a move). Refused if the cell is filled, or if you carry a block and the cell above it is filled.\n\
- U: climb onto the cell in front of you, up one and forward one. Refused if that cell is empty, if the cell above \
it or above you is filled, or if you carry a block and the cell two above the one in front is filled.\n\
- D: with empty hands, lift the block in front of you (at your level) onto your head; refused if there is none, if \
something sits on it, or if the cell above you is filled. Carrying, put the block down in the cell in front of you at \
head height, where it falls until it rests; refused if that cell is filled.\n\n\
After every move everything settles: whatever has nothing under it falls a row at a time, all together, you and \
what you carry included, until nothing moves. The level is solved when a move ends with you on the exit; falling \
through it on the way down doesn't count. A refused key changes nothing and costs nothing, and stops the rest of \
the string; the reply says which key and why.";
    if matches!(world, World::Crumble | World::Mixed) {
        text += "\n\nCracked floor (~) is solid until whatever stands directly on it leaves it (you walk, climb or \
fall off, or a block is lifted off), then it is gone for good. Turning on it, or standing on a block that stands on \
it, doesn't break it; something passing over it during a move does.";
    }
    if matches!(world, World::Ice | World::Mixed) {
        text += "\n\nIce blocks (I) slide. One you put down slides away from you until the next cell is filled, \
falling off ledges and sliding on the same way. Anything that falls at least a cell and lands on resting ice knocks \
it sliding the way the lander was going (you: the way you face), and the lander drops into the gap. Sliding ice \
that hits ice stops, and the ice it hit slides on, carrying what is stacked on it. You never ride ice; you lift and \
carry it like any block.";
    }
    if matches!(world, World::Springs | World::Mixed) {
        text += "\n\nSprings (^) are floor that throws what arrives on them (walking, climbing, falling or hopping \
on; turning on one doesn't count). A bounce rises as high as you fell, at least one row, stopping at a ceiling; at \
the top it hops one cell forward if that is clear (for you, with room for what you carry), then falls, and landing \
on another spring bounces again. If the hop is blocked it comes back down onto its spring. A block you put down \
bounces the same way, forward the way you face.";
    }
    text += "\n\nMoves count only while they are on the board: undo takes moves back and restart clears them, both \
free. The board may hold at most three times the level's shortest solution; filling it unsolved ends the session. \
Tool calls are limited too (status shows how many are left). Every move, undo and restart returns the new grid, so \
there is no need to call look between them.";
    text
}

/// One tool call, as the record keeps it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Call {
    pub tool: String,
    pub arguments: Value,
    pub reply: Vec<String>,
    pub error: bool,
}

/// How the session ended, or stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    pub solved: bool,
    /// Moves on the board.
    pub moves: usize,
}

/// A session: which level, its budgets, every call with its reply, and the outcome. [`replay`] checks it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub world: String,
    pub tier: u8,
    pub seed: u64,
    /// The level's fingerprint, as in the bench's suite files.
    pub fingerprint: String,
    pub shortest: usize,
    pub budget: usize,
    pub max_calls: Option<u32>,
    pub calls: Vec<Call>,
    pub outcome: Outcome,
}

/// Which level a session plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pick {
    pub world: World,
    pub tier: u8,
    pub seed: u64,
}

impl Pick {
    /// The bench's level for it and its shortest solution's length; `None` for a tier out of 1-4 or a seed that makes
    /// no level.
    pub fn level(&self) -> Option<(Level, usize)> {
        let tier = TIERS.get((self.tier as usize).checked_sub(1)?)?;
        generate(self.seed, self.world, tier).map(|g| (g.level, g.solution.len()))
    }
}

/// The game behind the server: one level's session and the calls made on it.
pub struct Puzzle {
    pick: Pick,
    session: session::Session,
    json: bool,
    calls: Vec<Call>,
}

/// The server: the puzzle behind `mcp_stdio`'s session, which counts calls against the cap.
pub struct Server(Session<Puzzle>);

impl Server {
    /// A server for `pick`'s level, its calls capped at `max_calls`, with JSON in each reply when `json`. `None` when
    /// the pick makes no level.
    pub fn new(pick: Pick, max_calls: u32, json: bool) -> Option<Server> {
        let (level, shortest) = pick.level()?;
        let session = session::Session::new(level, shortest);
        Some(Server(Session::new(Puzzle { pick, session, json, calls: vec![] }).with_max_calls(max_calls)))
    }

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

fn record(s: &Session<Puzzle>) -> Record {
    let p = &s.game;
    let o = p.session.outcome();
    Record {
        world: p.pick.world.name().into(),
        tier: p.pick.tier,
        seed: p.pick.seed,
        fingerprint: format!("{:#018x}", p.session.play().level.fingerprint()),
        shortest: p.session.budget() / session::BUDGET,
        budget: p.session.budget(),
        max_calls: s.max_calls(),
        calls: p.calls.clone(),
        outcome: Outcome { solved: o.solved, moves: o.moves },
    }
}

/// Plays a record's calls back against `puzzle-core` from its level: the outcome they reach, or why the record is
/// wrong (the level is not the one it names). Calls that were errors played nothing, so they are skipped.
pub fn replay(record: &Record) -> Result<Outcome, String> {
    let world = World::from_name(&record.world).ok_or(format!("no world {:?}", record.world))?;
    let pick = Pick { world, tier: record.tier, seed: record.seed };
    let (level, shortest) = pick.level().ok_or("the pick makes no level")?;
    let fingerprint = format!("{:#018x}", level.fingerprint());
    if fingerprint != record.fingerprint {
        return Err(format!("the level is {fingerprint}, not {}", record.fingerprint));
    }
    let acts: Vec<Act> = record
        .calls
        .iter()
        .filter(|c| !c.error)
        .filter_map(|c| match c.tool.as_str() {
            "move" => Some(Act::Keys(c.arguments["keys"].as_str()?.to_owned())),
            "undo" => Some(Act::Undo(undo_count(&c.arguments).ok()?)),
            "restart" => Some(Act::Restart),
            _ => None,
        })
        .collect();
    let o = session::Session::replay(level, shortest, &acts).outcome();
    Ok(Outcome { solved: o.solved, moves: o.moves })
}

impl Game for Puzzle {
    fn name(&self) -> &'static str {
        "out-of-the-box"
    }

    fn instructions(&self) -> String {
        instructions(self.pick.world)
    }

    fn tools(&self) -> Value {
        serde_json::from_str(TOOLS).expect("tools.json is valid JSON")
    }

    fn status(&self, calls_left: Option<u32>) -> Value {
        let o = self.session.outcome();
        json!({
            "solved": o.solved,
            "moves": o.moves,
            "budget": self.session.budget(),
            "calls_left": calls_left,
            "over": self.session.over() || calls_left == Some(0),
        })
    }

    fn call(&mut self, tool: &str, args: &Value, calls_left: Option<u32>) -> Reply {
        // Counted after this call.
        let left = calls_left.map(|n| n - 1);
        let first = match tool {
            "look" => String::new(),
            "status" => return Ok(vec![self.counters(left)]),
            _ if !TOOL_NAMES.contains(&tool) => return Err(vec![format!("No tool named {tool:?}.")]),
            _ if self.session.over() => {
                return Err(vec![format!("The session is over: no more moves.\n{}", self.counters(left))]);
            }
            "move" => {
                let keys = args.get("keys").and_then(Value::as_str).ok_or(vec!["keys is required.".to_owned()])?;
                let wanted = keys.chars().count();
                said(self.session.keys(keys), wanted, self.session.budget())
            }
            "undo" => {
                let n = undo_count(args).map_err(|e| vec![e])?;
                format!("Took back {} move(s).", self.session.undo(n))
            }
            "restart" => {
                self.session.restart();
                "Back at the start.".into()
            }
            _ => unreachable!("every listed tool is handled"),
        };
        let mut text = if first.is_empty() { String::new() } else { first + "\n" };
        text += &self.observation(left);
        let mut reply = vec![text];
        if self.json {
            reply.push(self.json(left).to_string());
        }
        Ok(reply)
    }

    fn note(&mut self, tool: &str, args: &Value, reply: &Reply) {
        let (texts, error) = match reply {
            Ok(t) => (t.clone(), false),
            Err(t) => (t.clone(), true),
        };
        self.calls.push(Call { tool: tool.to_owned(), arguments: args.clone(), reply: texts, error });
    }
}

impl Puzzle {
    /// The grid and everything an agent needs beside it.
    fn observation(&self, calls_left: Option<u32>) -> String {
        let play = self.session.play();
        let s = play.state();
        let facing = if s.facing == Facing::Left { "left" } else { "right" };
        let carrying = match (s.carrying, s.carrying_ice) {
            (false, _) => "nothing",
            (true, false) => "a block",
            (true, true) => "an ice block",
        };
        let (world, tier) = (self.pick.world.name(), self.pick.tier);
        format!(
            "{} world, tier {tier}.\n{}\nYou face {facing}, carrying {carrying}.\n{}",
            capitalised(world),
            play.show(),
            self.counters(calls_left)
        )
    }

    fn counters(&self, calls_left: Option<u32>) -> String {
        let o = self.session.outcome();
        let calls = calls_left.map_or(String::new(), |n| format!(" Tool calls left: {n}."));
        let end = match (o.solved, self.session.over()) {
            (true, _) => " Solved: the session is over.",
            (false, true) => " The board is full, unsolved: the session is over.",
            _ => "",
        };
        format!("Moves on the board: {} of {} allowed.{calls}{end}", o.moves, self.session.budget())
    }

    fn json(&self, calls_left: Option<u32>) -> Value {
        let play = self.session.play();
        let s = play.state();
        json!({
            "world": self.pick.world.name(),
            "tier": self.pick.tier,
            "grid": play.show().lines().collect::<Vec<_>>(),
            "you": [s.x, s.y],
            "facing": if s.facing == Facing::Left { "left" } else { "right" },
            "carrying": s.carrying,
            "carrying_ice": s.carrying_ice,
            "exit": [play.level.exit.0, play.level.exit.1],
            "status": self.status(calls_left),
        })
    }
}

/// What a move string did, in a sentence.
fn said(m: Moved, wanted: usize, budget: usize) -> String {
    let a = m.applied;
    let played = format!("Played {} of {wanted} key(s)", a.played);
    match (a.solved, a.stop, m.out_of_moves) {
        (true, ..) => format!("{played}: solved!"),
        (_, Some(stop), _) => {
            format!("{played}: key {} ({:?}) was refused: {}.", stop.index + 1, stop.key, why(stop.why))
        }
        (_, _, true) => format!("{played}: the board holds all {budget} allowed moves, unsolved."),
        _ => format!("{played}."),
    }
}

fn why(r: Refusal) -> &'static str {
    match r {
        Refusal::Blocked => "the cell you would move into is filled",
        Refusal::CarryBlocked => "the block you carry would hit something",
        Refusal::NothingToClimb => "the cell in front of you is empty, so there is nothing to climb",
        Refusal::NoHeadroom => "there is no room above you or above the step",
        Refusal::NoBlock => "there is no block in front of you at your level",
        Refusal::Covered => "something sits on the block in front of you",
        Refusal::NoRoom => "the cell in front of you at head height is filled",
        Refusal::NotAKey => "it is not L, R, U or D",
    }
}

/// The `n` argument of undo: a whole number from 1, default 1.
fn undo_count(args: &Value) -> Result<usize, String> {
    match args.get("n") {
        None => Ok(1),
        Some(n) => n.as_u64().filter(|&n| n >= 1).map(|n| n as usize).ok_or("n must be a whole number from 1".into()),
    }
}

fn capitalised(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tool_file_names_exactly_the_tools_the_server_handles() {
        let tools: Value = serde_json::from_str(TOOLS).unwrap();
        let names: Vec<&str> = tools.as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, TOOL_NAMES);
    }

    #[test]
    fn the_instructions_give_each_worlds_rules_and_legend_and_no_example() {
        let classic = instructions(World::Classic);
        assert!(classic.contains("# wall") && !classic.contains("Cracked floor") && !classic.contains("Springs ("));
        let mixed = instructions(World::Mixed);
        for rule in ["Cracked floor (~)", "Ice blocks (I)", "Springs (^)"] {
            assert!(mixed.contains(rule), "{rule}");
        }
        assert!(!classic.contains("For example") && classic.contains("no need to call look"));
    }
}
