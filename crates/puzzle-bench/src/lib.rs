//! *Out of the Box* as an agent benchmark (OB5, `docs/planning/puzzle-spec.md`, "The bench"): what a session's
//! evidence means, the native players, and the published numbers. The sweep itself is `bench_core`'s; [`Puzzle`] is
//! this game behind it.
//!
//! A run is one session on one level through `puzzle-mcp`. It counts only if its record replays against
//! `puzzle-core` to the outcome it claims ([`analyse`]). The headline is the solve rate per world and tier (P5); on
//! solved levels, moves over the shortest solution; for every session, calls, tokens, refused move strings, undos
//! and restarts. Fresh seeds (P13) are summarised apart from the suite's: a gap between the two flags contamination.

use bench_core::{Game, Run, RunResult};
use puzzle_core::session::Session;
use puzzle_core::{Facing, Key as Press, Play, World};
use puzzle_mcp::{Pick, Record, Server, TOOL_NAMES, replay};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Which run: the level (world, tier, seed), whether its seed was drawn fresh for this sweep, and the trial.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Key {
    pub world: String,
    pub tier: u8,
    pub seed: u64,
    pub fresh: bool,
    pub trial: u32,
}

impl Key {
    pub fn pick(&self) -> Pick {
        Pick { world: World::from_name(&self.world).expect("a world"), tier: self.tier, seed: self.seed }
    }
}

/// How a session ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ending {
    Solved,
    /// The board filled with the level unsolved.
    OutOfMoves,
    /// Hit the tool-call cap.
    OutOfCalls,
    /// The agent stopped calling tools with moves and calls left.
    Stopped,
}

/// What replaying a session shows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Analysis {
    /// The record replays to the outcome it claims, on the level it names.
    pub verified: bool,
    pub ending: Ending,
    pub moves: usize,
    pub shortest: usize,
    /// Moves over the shortest solution, on a solved level.
    pub over_shortest: Option<f32>,
    pub calls: usize,
    pub move_calls: usize,
    /// Move strings stopped by a refused key: a direct measure of whether the agent understood the rules.
    pub refused: usize,
    pub undos: usize,
    pub restarts: usize,
    /// Calls that were mistakes (bad arguments, unknown tools, acting once over), not counting the cap.
    pub mistakes: usize,
}

/// Replays `record` call by call against `puzzle-core` and measures the play.
pub fn analyse(record: &Record) -> Analysis {
    let level = Key { world: record.world.clone(), tier: record.tier, seed: record.seed, fresh: false, trial: 0 };
    let verified = replay(record) == Ok(record.outcome);
    let mut session = level.pick().level().map(|(l, shortest)| Session::new(l, shortest));
    let (mut move_calls, mut refused, mut undos, mut restarts) = (0, 0, 0, 0);
    for c in record.calls.iter().filter(|c| !c.error) {
        let Some(s) = session.as_mut() else { break };
        match c.tool.as_str() {
            "move" => {
                move_calls += 1;
                let keys = c.arguments["keys"].as_str().unwrap_or("");
                refused += s.keys(keys).applied.stop.is_some() as usize;
            }
            "undo" => {
                undos += 1;
                s.undo(c.arguments["n"].as_u64().unwrap_or(1) as usize);
            }
            "restart" => {
                restarts += 1;
                s.restart();
            }
            _ => {}
        }
    }
    let capped = record.max_calls.is_some_and(|m| record.calls.len() as u32 > m);
    let errors = record.calls.iter().filter(|c| c.error).count();
    let past_cap = if capped { record.calls.len() - record.max_calls.unwrap() as usize } else { 0 };
    let o = record.outcome;
    let ending = if o.solved {
        Ending::Solved
    } else if o.moves >= record.budget {
        Ending::OutOfMoves
    } else if record.max_calls.is_some_and(|m| record.calls.len() as u32 >= m) {
        Ending::OutOfCalls
    } else {
        Ending::Stopped
    };
    Analysis {
        verified,
        ending,
        moves: o.moves,
        shortest: record.shortest,
        over_shortest: o.solved.then(|| o.moves as f32 / record.shortest.max(1) as f32),
        calls: record.calls.len(),
        move_calls,
        refused,
        undos,
        restarts,
        mistakes: errors - past_cap,
    }
}

/// The players that need no model, played natively on the same levels: the solver (a shortest solution, the
/// ceiling), `random` (uniform keys until solved or the board is full, the floor) and `greedy` (toward the exit,
/// climbing when it can, else a random legal key).
pub const PLAYERS: [&str; 3] = ["solver", "random", "greedy"];

/// Plays native player `name` on `key`'s level through the server itself, so its record is a session's like any
/// other (with no call cap): the solver in one move call, the others a key per call. `random` and `greedy` draw from
/// the seed and the trial, so their runs repeat and their trials differ.
pub fn play(name: &str, key: &Key) -> Option<Record> {
    let pick = key.pick();
    let (level, shortest) = pick.level()?;
    let mut server = Server::on(pick, level.clone(), shortest, u32::MAX, false);
    let mut call = |id: usize, tool: &str, args: Value| {
        let msg = json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": { "name": tool, "arguments": args } });
        server.handle(&msg);
    };
    let mut s = (key.seed as u32 ^ (key.seed >> 32) as u32).wrapping_mul(31) ^ key.trial.wrapping_mul(0x9E37_79B9);
    s = s.max(1);
    let mut next = move || {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        s
    };
    match name {
        "solver" => {
            let solution: String = puzzle_core::solve(&level).solution?.iter().map(|k| format!("{k:?}")).collect();
            call(0, "move", json!({ "keys": solution }));
        }
        "random" | "greedy" => {
            let mut session = Session::new(level, shortest);
            // Every key a turn or better is legal somewhere, so this ends; the bound is a guard.
            for id in 0..100 * session.budget() {
                if session.over() {
                    break;
                }
                let press = if name == "random" { random(&mut next) } else { greedy(session.play(), &mut next) };
                let keys = format!("{press:?}");
                session.keys(&keys);
                call(id, "move", json!({ "keys": keys }));
            }
        }
        _ => return None,
    }
    Some(server.record())
}

fn random(next: &mut impl FnMut() -> u32) -> Press {
    [Press::L, Press::R, Press::U, Press::D][(next() % 4) as usize]
}

/// Toward the exit's column: a step if already facing it, else a turn; blocked, a climb; else a random legal key.
fn greedy(play: &Play, next: &mut impl FnMut() -> u32) -> Press {
    let s = play.state();
    let legal = |k: Press| {
        let mut p = play.clone();
        p.apply(&format!("{k:?}")).played == 1
    };
    let dx = play.level.exit.0 as i32 - s.x as i32;
    let toward = if dx < 0 { Press::L } else { Press::R };
    let facing = if s.facing == Facing::Left { Press::L } else { Press::R };
    if dx != 0 && (facing != toward || legal(toward)) {
        return toward;
    }
    if legal(Press::U) {
        return Press::U;
    }
    let options: Vec<Press> = [Press::L, Press::R, Press::U, Press::D].into_iter().filter(|&k| legal(k)).collect();
    options[(next() as usize) % options.len()]
}

/// One run as the results file reports it.
pub type PuzzleResult = RunResult<Key, Analysis>;

/// *Out of the Box* behind the bench.
pub struct Puzzle;

impl Game for Puzzle {
    type Key = Key;
    type Record = Record;
    type Analysis = Analysis;
    const BENCH: &'static str = "puzzle-bench";
    const SERVER: &'static str = "puzzle";
    const BINARY: &'static str = "puzzle-mcp";

    fn tools(&self) -> Vec<String> {
        TOOL_NAMES.iter().map(|t| t.to_string()).collect()
    }

    fn native(&self, model: &str) -> bool {
        PLAYERS.contains(&model)
    }

    fn play(&self, model: &str, run: &Run<Key>) -> Option<Record> {
        play(model, &run.key)
    }

    fn analyse(&self, record: &Record) -> Analysis {
        analyse(record)
    }

    fn line(&self, a: &Analysis) -> String {
        format!(
            "{:?}: {} moves (shortest {}), {} calls, {} refused, {} undos, {} restarts, verified {}",
            a.ending, a.moves, a.shortest, a.calls, a.refused, a.undos, a.restarts, a.verified
        )
    }

    fn summarise(&self, runs: &[PuzzleResult]) -> Value {
        serde_json::to_value(summarise(runs)).expect("a summary serialises")
    }
}

/// The numbers for one group of runs (a world and tier, suite or fresh), over verified runs only.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub world: String,
    pub tier: u8,
    pub fresh: bool,
    pub runs: usize,
    pub verified: usize,
    pub solved: usize,
    pub solve_rate: f32,
    /// Mean moves over the shortest solution, on solved levels.
    pub over_shortest: Option<f32>,
    pub mean_calls: f32,
    pub mean_input_tokens: f32,
    pub mean_output_tokens: f32,
    /// Refused move strings over move calls.
    pub refused_rate: f32,
    pub mean_undos: f32,
    pub mean_restarts: f32,
    pub endings: Vec<(Ending, usize)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    pub runs: usize,
    pub verified: usize,
    pub solve_rate: f32,
    pub rows: Vec<Row>,
    pub reported_cost_usd: f64,
}

pub fn summarise(runs: &[PuzzleResult]) -> Summary {
    let counted = |rs: &[&PuzzleResult]| -> Vec<(PuzzleResult, Analysis)> {
        rs.iter().filter_map(|r| r.analysis.clone().filter(|a| a.verified).map(|a| ((*r).clone(), a))).collect()
    };
    let mut groups: Vec<(String, u8, bool)> =
        runs.iter().map(|r| (r.key.world.clone(), r.key.tier, r.key.fresh)).collect();
    groups.dedup();
    groups.sort_by_key(|g| (World::from_name(&g.0).map_or(9, |w| w as u8), g.1, g.2));
    groups.dedup();
    let rows = groups
        .into_iter()
        .map(|(world, tier, fresh)| {
            let group: Vec<&PuzzleResult> =
                runs.iter().filter(|r| r.key.world == world && r.key.tier == tier && r.key.fresh == fresh).collect();
            let ok = counted(&group);
            let n = ok.len().max(1) as f32;
            let mean = |f: &dyn Fn(&PuzzleResult, &Analysis) -> f32| ok.iter().map(|(r, a)| f(r, a)).sum::<f32>() / n;
            let solved: Vec<f32> = ok.iter().filter_map(|(_, a)| a.over_shortest).collect();
            let move_calls: usize = ok.iter().map(|(_, a)| a.move_calls).sum();
            Row {
                world,
                tier,
                fresh,
                runs: group.len(),
                verified: ok.len(),
                solved: solved.len(),
                solve_rate: solved.len() as f32 / n,
                over_shortest: (!solved.is_empty()).then(|| solved.iter().sum::<f32>() / solved.len() as f32),
                mean_calls: mean(&|_, a| a.calls as f32),
                mean_input_tokens: mean(&|r, _| r.input_tokens as f32),
                mean_output_tokens: mean(&|r, _| r.output_tokens as f32),
                refused_rate: ok.iter().map(|(_, a)| a.refused).sum::<usize>() as f32 / move_calls.max(1) as f32,
                mean_undos: mean(&|_, a| a.undos as f32),
                mean_restarts: mean(&|_, a| a.restarts as f32),
                endings: [Ending::Solved, Ending::OutOfMoves, Ending::OutOfCalls, Ending::Stopped]
                    .map(|e| (e, ok.iter().filter(|(_, a)| a.ending == e).count()))
                    .to_vec(),
            }
        })
        .collect();
    let all = counted(&runs.iter().collect::<Vec<_>>());
    Summary {
        runs: runs.len(),
        verified: all.len(),
        solve_rate: all.iter().filter(|(_, a)| a.ending == Ending::Solved).count() as f32 / all.len().max(1) as f32,
        rows,
        reported_cost_usd: runs.iter().map(|r| r.reported_cost_usd).sum(),
    }
}

/// A fresh seed (P13): from 2^32 up, drawn from the OS's randomness (std seeds every hasher's keys with it), the
/// first that makes a level of `world` and `tier`.
pub fn fresh_seed(world: World, tier: u8) -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u8(tier);
    let start = (1u64 << 32) + (h.finish() >> 24);
    (start..).find(|&seed| Pick { world, tier, seed }.level().is_some()).expect("some seed makes a level")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(world: &str, tier: u8, seed: u64) -> Key {
        Key { world: world.into(), tier, seed, fresh: false, trial: 1 }
    }

    #[test]
    fn the_solver_solves_in_the_shortest_moves_and_verifies() {
        let a = analyse(&play("solver", &key("classic", 2, 1)).unwrap());
        assert!(a.verified && a.ending == Ending::Solved, "{a:?}");
        assert_eq!(a.over_shortest, Some(1.0));
        assert_eq!(a.calls, 1);
    }

    #[test]
    fn random_and_greedy_play_until_solved_or_out_of_moves_repeatably() {
        for name in ["random", "greedy"] {
            let k = key("ice", 1, 1);
            let a = analyse(&play(name, &k).unwrap());
            assert!(a.verified && matches!(a.ending, Ending::Solved | Ending::OutOfMoves), "{name}: {a:?}");
            assert_eq!(play(name, &k), play(name, &k), "{name} repeats");
        }
        assert!(play("nobody", &key("classic", 1, 1)).is_none());
    }

    #[test]
    fn a_doctored_record_does_not_verify() {
        let mut record = play("solver", &key("classic", 1, 1)).unwrap();
        record.outcome.moves += 1;
        assert!(!analyse(&record).verified);
    }

    #[test]
    fn the_summary_groups_by_world_tier_and_fresh_and_counts_verified_runs() {
        let run = |world: &str, tier, fresh, player| {
            let key = Key { fresh, ..key(world, tier, 1) };
            let analysis = Some(analyse(&play(player, &key).unwrap()));
            RunResult {
                key,
                dir: String::new(),
                analysis,
                error: None,
                wall_seconds: 0.0,
                reported_cost_usd: 0.5,
                agent_turns: 0,
                input_tokens: 100,
                output_tokens: 10,
                plan_used: None,
            }
        };
        let runs =
            [run("classic", 1, false, "solver"), run("classic", 1, true, "solver"), run("ice", 2, false, "solver")];
        let s = summarise(&runs);
        assert_eq!((s.runs, s.verified, s.rows.len()), (3, 3, 3));
        assert_eq!(s.solve_rate, 1.0);
        assert!(s.rows[0].world == "classic" && !s.rows[0].fresh && s.rows[1].fresh && s.rows[2].world == "ice");
        assert_eq!(s.rows[0].mean_input_tokens, 100.0);
        assert!((s.reported_cost_usd - 1.5).abs() < 1e-9);
    }

    #[test]
    fn fresh_seeds_start_above_the_bench_and_training_ranges() {
        let seed = fresh_seed(World::Classic, 1);
        assert!(seed >= 1 << 32);
        assert!(Pick { world: World::Classic, tier: 1, seed }.level().is_some());
    }
}
