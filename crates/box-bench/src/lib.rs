//! *Escape the Box* as an agent benchmark (ML track M6, `docs/planning/ml-track.md`): what a run's evidence means.
//!
//! A run is one agent session playing one run of the game through `box-mcp`. Its evidence is the server's record
//! (`box_mcp::Record`: the trace, the outcome, the calls) and the agent's transcript. A run counts only if replaying
//! its trace reproduces the recorded outcome ([`analyse`] replays it and measures the play as it goes). [`summarise`]
//! turns runs into the published numbers: escape rate, pass^k per seed, and the failure taxonomy.
//!
//! `main.rs` launches the sessions; everything here is pure, so it is tested without an agent.

use box_env::{ACTIONS, Action, Env, EnvConfig, Outcome};
use box_mcp::Record;
use serde::{Deserialize, Serialize};

/// How a run ended. One per run, first match wins in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ending {
    /// Escaped at least one box (and then died, ran out, or stopped).
    Escaped,
    /// Died without escaping.
    Died,
    /// Hit the tool-call cap.
    OutOfCalls,
    /// Hit the env's step cap.
    OutOfTime,
    /// The agent stopped calling tools with the run still going.
    Stopped,
}

/// Behaviour seen in a run, any number per run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Flag {
    /// The same call (tool and arguments) [`LOOP`] or more times in a row.
    Looped,
    /// Within [`WALL`] of a wall for most of the game time.
    HuggedWalls,
    /// Killed enough to build a ramp ([`RAMP_KILLS`]) but never stood on a pile half the rim's height.
    SpreadThePile,
    /// Attacked in under a tenth of its action calls.
    Passive,
    /// At least [`MISTAKES`] calls refused as mistakes (bad arguments, unknown tools), not counting the cap.
    Fumbled,
}

/// Repeats that count as a loop.
pub const LOOP: usize = 5;
/// Metres from a wall that count as hugging it.
pub const WALL: f32 = 0.8;
/// Kills that, heaped in one place, reach box 1's rim (about 20 fodder; the README's G3 measurements).
pub const RAMP_KILLS: u32 = 15;
/// Refused calls that count as fumbling.
pub const MISTAKES: usize = 3;

/// What replaying a run shows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Analysis {
    /// The replay reproduced the recorded outcome exactly.
    pub verified: bool,
    pub replayed: Outcome,
    pub ending: Ending,
    pub flags: Vec<Flag>,
    /// The highest the player stood on the pile, as a share of that box's rim.
    pub best_pile: f32,
    /// Share of steps spent within [`WALL`] of a wall.
    pub wall_share: f32,
    pub game_seconds: f32,
    /// The progress score: the episode's summed reward (`box_env` module docs), the bench's secondary number.
    pub score: f32,
    pub calls: usize,
    pub refused: usize,
}

/// Replays `record`'s trace, checks it against the recorded outcome, and measures the play.
pub fn analyse(record: &Record) -> Analysis {
    let trace = &record.trace;
    let mut env = Env::new(trace.config);
    let (mut best_pile, mut near_wall) = (0.0f32, 0usize);
    for &a in &trace.actions {
        let s = env.step(Action(a));
        let o = &s.obs;
        best_pile = best_pile.max(o.player.pile / o.rim.max(1e-3));
        let [x, y] = [o.player.at[0], o.player.at[2]];
        let gap = (o.floor[0] / 2.0 - x.abs()).min(o.floor[1] / 2.0 - y.abs());
        near_wall += (gap < WALL) as usize;
    }
    let replayed = env.outcome();
    let steps = trace.actions.len();
    let wall_share = if steps == 0 { 0.0 } else { near_wall as f32 / steps as f32 };

    let capped = record.max_calls.is_some_and(|m| record.calls.len() as u32 > m);
    let refused = record.calls.iter().filter(|c| c.error).count();
    let mistakes = refused - if capped { record.calls.len() - record.max_calls.unwrap() as usize } else { 0 };
    let ending = if replayed.escapes > 0 {
        Ending::Escaped
    } else if replayed.died {
        Ending::Died
    } else if capped || record.max_calls.is_some_and(|m| record.calls.len() as u32 >= m) {
        Ending::OutOfCalls
    } else if replayed.steps >= trace.config.max_steps {
        Ending::OutOfTime
    } else {
        Ending::Stopped
    };

    let mut flags = Vec::new();
    let same_run =
        record.calls.windows(LOOP).any(|w| w.iter().all(|c| c.tool == w[0].tool && c.arguments == w[0].arguments));
    if same_run {
        flags.push(Flag::Looped);
    }
    if wall_share > 0.5 {
        flags.push(Flag::HuggedWalls);
    }
    if replayed.kills >= RAMP_KILLS && best_pile < 0.5 {
        flags.push(Flag::SpreadThePile);
    }
    let actions: Vec<_> = record.calls.iter().filter(|c| !c.error && c.steps > 0).collect();
    let attacks = actions.iter().filter(|c| matches!(c.tool.as_str(), "punch" | "kick")).count();
    if !actions.is_empty() && attacks * 10 < actions.len() {
        flags.push(Flag::Passive);
    }
    if mistakes >= MISTAKES {
        flags.push(Flag::Fumbled);
    }

    Analysis {
        verified: replayed == record.outcome,
        replayed,
        ending,
        flags,
        best_pile,
        wall_share,
        game_seconds: replayed.steps as f32 * trace.config.frame_skip as f32 / 60.0,
        score: f32::from_bits(replayed.reward_bits),
        calls: record.calls.len(),
        refused,
    }
}

/// The agents that need no model, played natively on the same seeds as the LLMs: the table's floor.
pub const BASELINES: [&str; 2] = ["brawler", "random"];

/// Plays baseline `name` (one of [`BASELINES`]) on `seed` until the run ends, as a record like a session's (with no
/// calls). `random` draws its actions from `seed` and `trial`; `brawler` (`box_env::brawler`: walk at the nearest
/// enemy, punch in reach) plays the same every trial.
pub fn baseline(name: &str, seed: u32, trial: u32, rim: Option<f32>) -> Option<Record> {
    let mut env = Env::new(EnvConfig { seed, rim, ..EnvConfig::default() });
    let mut s = (seed.wrapping_mul(31) ^ trial.wrapping_mul(0x9E37_79B9)).max(1);
    let mut random = move || {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        Action((s % ACTIONS as u32) as u8)
    };
    while !env.done() {
        let a = match name {
            "brawler" => box_env::brawler(env.last()),
            "random" => random(),
            _ => return None,
        };
        env.step(a);
    }
    Some(Record { trace: env.trace(), outcome: env.outcome(), calls: Vec::new(), max_calls: None })
}

/// One run as the results file reports it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunResult {
    pub seed: u32,
    pub trial: u32,
    /// Where its evidence is, relative to the results file.
    pub dir: String,
    pub analysis: Option<Analysis>,
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

impl RunResult {
    fn counted(&self) -> Option<&Analysis> {
        self.analysis.as_ref().filter(|a| a.verified)
    }
}

/// A sweep's headline numbers, over verified runs only.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    pub runs: usize,
    pub verified: usize,
    pub escape_rate: f32,
    /// Seeds where every trial escaped, over seeds with all trials verified.
    pub pass_k: f32,
    /// Seeds where at least one trial escaped.
    pub pass_any: f32,
    pub mean_boxes_escaped: f32,
    pub mean_kills: f32,
    pub mean_best_pile: f32,
    pub mean_calls: f32,
    pub mean_game_seconds: f32,
    pub mean_score: f32,
    pub endings: Vec<(Ending, usize)>,
    pub flags: Vec<(Flag, usize)>,
    pub reported_cost_usd: f64,
}

pub fn summarise(runs: &[RunResult]) -> Summary {
    let counted: Vec<&Analysis> = runs.iter().filter_map(RunResult::counted).collect();
    let n = counted.len().max(1) as f32;
    let mean = |f: &dyn Fn(&Analysis) -> f32| counted.iter().map(|a| f(a)).sum::<f32>() / n;

    let mut seeds: Vec<u32> = runs.iter().map(|r| r.seed).collect();
    seeds.sort_unstable();
    seeds.dedup();
    let (mut all, mut any, mut complete) = (0, 0, 0);
    for seed in &seeds {
        let trials: Vec<&RunResult> = runs.iter().filter(|r| r.seed == *seed).collect();
        let escaped: Vec<bool> =
            trials.iter().filter_map(|r| r.counted()).map(|a| a.ending == Ending::Escaped).collect();
        if escaped.len() == trials.len() {
            complete += 1;
            all += escaped.iter().all(|e| *e) as usize;
        }
        any += escaped.iter().any(|e| *e) as usize;
    }

    let endings = [Ending::Escaped, Ending::Died, Ending::OutOfCalls, Ending::OutOfTime, Ending::Stopped]
        .map(|e| (e, counted.iter().filter(|a| a.ending == e).count()))
        .to_vec();
    let flags = [Flag::Looped, Flag::HuggedWalls, Flag::SpreadThePile, Flag::Passive, Flag::Fumbled]
        .map(|f| (f, counted.iter().filter(|a| a.flags.contains(&f)).count()))
        .to_vec();

    Summary {
        runs: runs.len(),
        verified: counted.len(),
        escape_rate: counted.iter().filter(|a| a.ending == Ending::Escaped).count() as f32 / n,
        pass_k: all as f32 / complete.max(1) as f32,
        pass_any: any as f32 / seeds.len().max(1) as f32,
        mean_boxes_escaped: mean(&|a| a.replayed.escapes as f32),
        mean_kills: mean(&|a| a.replayed.kills as f32),
        mean_best_pile: mean(&|a| a.best_pile),
        mean_calls: mean(&|a| a.calls as f32),
        mean_game_seconds: mean(&|a| a.game_seconds),
        mean_score: mean(&|a| a.score),
        endings,
        flags,
        reported_cost_usd: runs.iter().map(|r| r.reported_cost_usd).sum(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use box_env::EnvConfig;
    use box_mcp::Server;
    use serde_json::{Value, json};

    /// A session driven straight through the server (no process, no agent).
    fn session(seed: u32, max_calls: Option<u32>, calls: &[(&str, Value)]) -> Record {
        let mut server = Server::new(EnvConfig { seed, ..EnvConfig::default() });
        if let Some(m) = max_calls {
            server = server.with_max_calls(m);
        }
        for (i, (tool, args)) in calls.iter().enumerate() {
            let msg = json!({ "jsonrpc": "2.0", "id": i, "method": "tools/call", "params": { "name": tool, "arguments": args } });
            server.handle(&msg);
        }
        server.record()
    }

    #[test]
    fn a_replayed_session_verifies_and_a_doctored_one_does_not() {
        let record =
            session(2, None, &[("move", json!({ "direction": "north", "seconds": 1.0 })), ("punch", json!({}))]);
        let a = analyse(&record);
        assert!(a.verified, "{a:?}");
        assert_eq!(a.ending, Ending::Stopped, "the agent stopped with the run going");
        let mut doctored = record.clone();
        doctored.outcome.kills += 5;
        assert!(!analyse(&doctored).verified, "a claimed result the trace doesn't produce is rejected");
    }

    #[test]
    fn hitting_the_cap_ends_out_of_calls_and_repeats_are_a_loop() {
        let wait = ("wait", json!({ "seconds": 0.2 }));
        let calls: Vec<_> = std::iter::repeat_n(wait, 7).collect();
        let a = analyse(&session(1, Some(6), &calls));
        assert!(a.verified);
        assert_eq!(a.ending, Ending::OutOfCalls);
        assert!(a.flags.contains(&Flag::Looped) && a.flags.contains(&Flag::Passive), "{:?}", a.flags);
        assert!(!a.flags.contains(&Flag::Fumbled), "the cap's refusal isn't a mistake");
    }

    #[test]
    fn mistakes_are_fumbles_and_running_into_a_corner_hugs_walls() {
        let a = analyse(&session(
            1,
            None,
            &[
                ("move", json!({ "direction": "up", "seconds": 1 })),
                ("fly", json!({})),
                ("wait", json!({ "seconds": 99 })),
                ("move", json!({ "direction": "northeast", "seconds": 2.0 })),
                ("move", json!({ "direction": "northeast", "seconds": 2.0 })),
                ("wait", json!({ "seconds": 3.0 })),
            ],
        ));
        assert!(a.flags.contains(&Flag::Fumbled) && a.flags.contains(&Flag::HuggedWalls), "{a:?}");
    }

    #[test]
    fn baselines_play_until_they_die_and_verify() {
        for name in BASELINES {
            let a = analyse(&baseline(name, 1, 1, None).unwrap());
            assert!(a.verified && a.ending == Ending::Died, "{name}: {a:?}");
        }
        assert!(baseline("nobody", 1, 1, None).is_none());
    }

    #[test]
    fn the_summary_counts_verified_runs_and_pass_k_needs_every_trial() {
        let run = |seed, trial, ending, verified| RunResult {
            seed,
            trial,
            dir: String::new(),
            analysis: Some(Analysis {
                verified,
                replayed: Outcome {
                    steps: 60,
                    box_number: 2,
                    score: 0,
                    kills: 4,
                    escapes: (ending == Ending::Escaped) as u32,
                    died: true,
                    reward_bits: 0,
                    obs_hash: 0,
                },
                ending,
                flags: vec![],
                best_pile: 0.5,
                wall_share: 0.0,
                game_seconds: 4.0,
                score: 2.0,
                calls: 10,
                refused: 0,
            }),
            error: None,
            wall_seconds: 1.0,
            reported_cost_usd: 0.25,
            agent_turns: 10,
            input_tokens: 0,
            output_tokens: 0,
            plan_used: None,
        };
        let runs = [
            run(1, 1, Ending::Escaped, true),
            run(1, 2, Ending::Escaped, true),
            run(2, 1, Ending::Escaped, true),
            run(2, 2, Ending::Died, true),
            run(3, 1, Ending::Escaped, false),
        ];
        let s = summarise(&runs);
        assert_eq!((s.runs, s.verified), (5, 4));
        assert!((s.escape_rate - 0.75).abs() < 1e-6);
        assert!((s.pass_k - 0.5).abs() < 1e-6, "seed 1 of the two complete seeds; seed 3 isn't complete");
        assert!((s.pass_any - 2.0 / 3.0).abs() < 1e-6);
        assert!((s.reported_cost_usd - 1.25).abs() < 1e-9);
    }
}
