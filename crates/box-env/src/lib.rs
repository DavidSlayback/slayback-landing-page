//! *Escape the Box* as an environment for agents (ML track, `docs/planning/ml-track.md`): [`Env::reset`] and
//! [`Env::step`] over the real game, headless (`game3d::headless`), with the discrete actions and observations of
//! `game_core::agent`. An action is held for [`EnvConfig::frame_skip`] frames (its verb pressed on the first only).
//! An episode is one run: it ends when the player dies, or is cut off after [`EnvConfig::max_steps`].
//!
//! The same seed and the same actions give the same run, so an episode is fully described by its [`Trace`], and
//! [`replay`] reproduces its [`Outcome`] exactly: how results are checked.
//!
//! # Reward
//!
//! Per step, from what changed ([`reward`]):
//! - [`KILL`] for each enemy defeated;
//! - [`CLIMB`] times the rise, in rims, of the highest the player has stood on the pile against a wall in this box
//!   (only new highs count, so stepping on and off a pile pays once; a pile out in the middle can't get them out);
//! - [`ESCAPE`] for each box escaped;
//! - minus [`HURT`] times the health lost, as a share of the maximum (blows and the box clock's penalty);
//! - minus [`DEATH`] when the run ends in death;
//! - minus [`STEP`] every step, so dawdling costs a little.
//!
//! Known ways to game it, for the write-up: farming kills without heaping them where they can be climbed (the
//! climb and escape terms are meant to outweigh it).

use std::path::PathBuf;

use bevy::prelude::*;
use game_core::agent::Obs;
pub use game_core::agent::{ACTIONS, Action, FLAT};
use game3d::Frame;
use game3d::enemy::Waves;
use game3d::fighter::{Defeated, Team};
use game3d::flow::Phase;
use game3d::input::{Actions, ReadActions};
use game3d::run::BoxEnded;
use game3d::score::RunScore;
use game3d::store::Store;
use serde::{Deserialize, Serialize};

mod json;
pub use json::obs_json;

/// For each enemy defeated.
pub const KILL: f32 = 1.0;
/// Per rim of new height stood on in this box.
pub const CLIMB: f32 = 2.0;
/// For each box escaped.
pub const ESCAPE: f32 = 10.0;
/// Per maximum health lost.
pub const HURT: f32 = 2.0;
/// When the run ends in death.
pub const DEATH: f32 = 5.0;
/// Every step.
pub const STEP: f32 = 0.001;

/// How episodes run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvConfig {
    /// Picks the run's dice (which kinds come through the doors, which carry bombs).
    pub seed: u32,
    /// Frames each action is held for (60 frames a second).
    pub frame_skip: u32,
    /// Steps before an episode is cut off.
    pub max_steps: u32,
}

impl Default for EnvConfig {
    fn default() -> Self {
        // 4 frames: 15 decisions a second. 20,000 steps: about 22 minutes of play.
        Self { seed: 0, frame_skip: 4, max_steps: 20_000 }
    }
}

/// What happened during a step, besides the observation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Events {
    pub kills: u32,
    pub escapes: u32,
    pub died: bool,
}

/// What [`reward`] compares across a step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Progress {
    pub box_number: usize,
    pub rim: f32,
    pub hp: f32,
    pub max_hp: f32,
    /// The highest the player has stood on the pile against a wall in this box, metres.
    pub best_pile: f32,
}

impl Progress {
    /// Where things stand after `obs`, carrying `before`'s best pile within the same box.
    pub fn after(before: &Progress, obs: &Obs) -> Progress {
        let same_box = obs.box_number == before.box_number;
        let best = if same_box { before.best_pile } else { 0.0 };
        Progress {
            box_number: obs.box_number,
            rim: obs.rim,
            hp: obs.player.hp,
            max_hp: obs.player.max_hp,
            best_pile: if obs.player.wall_gap <= game_core::run::AT_WALL { best.max(obs.player.pile) } else { best },
        }
    }
}

/// The reward for a step from `before` to `after` with `events` (see the module docs).
pub fn reward(before: &Progress, after: &Progress, events: &Events) -> f32 {
    let mut r = KILL * events.kills as f32 + ESCAPE * events.escapes as f32 - STEP;
    if after.box_number == before.box_number && after.rim > 0.0 {
        r += CLIMB * (after.best_pile - before.best_pile).max(0.0) / after.rim;
    }
    // Health only falls through harm; an escape's refill isn't a reward.
    if after.max_hp > 0.0 {
        r -= HURT * (before.hp - after.hp).max(0.0) / after.max_hp;
    }
    if events.died {
        r -= DEATH;
    }
    r
}

/// One step's result.
#[derive(Clone, Debug)]
pub struct Step {
    pub obs: Obs,
    pub reward: f32,
    /// The run ended (death) or was cut off.
    pub done: bool,
    /// Cut off at [`EnvConfig::max_steps`] rather than ended.
    pub truncated: bool,
    pub events: Events,
}

/// An episode, as recorded: replaying it ([`replay`]) reproduces its [`Outcome`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trace {
    pub config: EnvConfig,
    pub actions: Vec<u8>,
}

/// How an episode stands: what a result reports and a replay must match.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    pub steps: u32,
    pub box_number: usize,
    pub score: u64,
    pub kills: u32,
    pub escapes: u32,
    pub died: bool,
    /// The summed reward's bits (an exact match, not a near one).
    pub reward_bits: u32,
    /// A hash of the last observation's flat encoding, bit for bit.
    pub obs_hash: u64,
}

/// What the agent presses this frame (copied over the devices' `Actions` each frame).
#[derive(Resource, Default)]
struct Command(Actions);

/// Events since the last step, from the game's messages.
#[derive(Resource, Default)]
struct Tally(Events);

/// The game, stepped by an agent.
pub struct Env {
    app: App,
    config: EnvConfig,
    store: PathBuf,
    steps: u32,
    progress: Progress,
    actions: Vec<u8>,
    totals: Events,
    total_reward: f32,
    done: bool,
    last: Obs,
}

impl Env {
    /// A fresh episode under `config`.
    pub fn new(config: EnvConfig) -> Env {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        // Its own records directory, so episodes never read or overwrite a real best score.
        let store = std::env::temp_dir().join(format!("box-env-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&store).expect("a records directory");
        let app = build(config.seed, Store::in_dir(&store));
        let mut env = Env {
            app,
            config,
            store,
            steps: 0,
            progress: Progress::default(),
            actions: Vec::new(),
            totals: Events::default(),
            total_reward: 0.0,
            done: false,
            last: placeholder(),
        };
        env.start();
        env
    }

    /// Starts a new episode with `seed` (the rest of the config kept) and returns its first observation.
    pub fn reset(&mut self, seed: u32) -> Obs {
        self.config.seed = seed;
        self.app = build(seed, Store::in_dir(&self.store));
        self.start();
        self.last.clone()
    }

    fn start(&mut self) {
        // The first frame runs startup: the box, the player, and the move to playing.
        self.app.update();
        self.steps = 0;
        self.actions.clear();
        self.totals = Events::default();
        self.total_reward = 0.0;
        self.done = false;
        self.app.world_mut().resource_mut::<Tally>().0 = Events::default();
        self.last = self.observe();
        self.progress = Progress::after(&Progress { box_number: self.last.box_number, ..default() }, &self.last);
    }

    pub fn config(&self) -> EnvConfig {
        self.config
    }

    /// The observation now.
    pub fn observe(&mut self) -> Obs {
        game3d::observe::observe(self.app.world_mut())
    }

    /// The last observation returned.
    pub fn last(&self) -> &Obs {
        &self.last
    }

    /// Holds `action` for `frame_skip` frames (fewer if the run ends first). After the episode is done it does
    /// nothing and reports done again.
    pub fn step(&mut self, action: Action) -> Step {
        if self.done {
            return Step {
                obs: self.last.clone(),
                reward: 0.0,
                done: true,
                truncated: false,
                events: Events::default(),
            };
        }
        let (dir, verbs) = action.decode();
        for frame in 0..self.config.frame_skip {
            let pressed = if frame == 0 { verbs } else { Default::default() };
            self.app.world_mut().resource_mut::<Command>().0 = Actions { dir, verbs: pressed };
            self.app.update();
            if self.app.world().resource::<Tally>().0.died {
                break;
            }
        }
        self.app.world_mut().resource_mut::<Command>().0 = Actions::default();
        let events = std::mem::take(&mut self.app.world_mut().resource_mut::<Tally>().0);
        let obs = self.observe();
        let progress = Progress::after(&self.progress, &obs);
        let r = reward(&self.progress, &progress, &events);

        self.steps += 1;
        self.actions.push(action.0);
        self.progress = progress;
        self.total_reward += r;
        self.totals.kills += events.kills;
        self.totals.escapes += events.escapes;
        self.totals.died |= events.died;
        let truncated = !events.died && self.steps >= self.config.max_steps;
        self.done = events.died || truncated;
        self.last = obs.clone();
        Step { obs, reward: r, done: self.done, truncated, events }
    }

    pub fn done(&self) -> bool {
        self.done
    }

    /// The episode so far, to replay.
    pub fn trace(&self) -> Trace {
        Trace { config: self.config, actions: self.actions.clone() }
    }

    /// How the episode stands.
    pub fn outcome(&self) -> Outcome {
        let score = self.app.world().resource::<RunScore>().0.points;
        Outcome {
            steps: self.steps,
            box_number: self.last.box_number,
            score,
            kills: self.totals.kills,
            escapes: self.totals.escapes,
            died: self.totals.died,
            reward_bits: self.total_reward.to_bits(),
            obs_hash: hash(&self.last.flat()),
        }
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.store);
    }
}

/// Plays `trace` again from the start and reports how it ended: equal to the recorded [`Outcome`] for a genuine trace.
pub fn replay(trace: &Trace) -> Outcome {
    let mut env = Env::new(trace.config);
    for &a in &trace.actions {
        env.step(Action(a));
    }
    env.outcome()
}

/// A scripted baseline: walk at the nearest enemy and punch it when in reach; with none about, stand still. No
/// heaping, no climbing: the floor any learned policy should beat.
pub fn brawler(obs: &Obs) -> Action {
    let Some(e) = obs.enemies.iter().find(|e| !e.carried) else { return Action::new(0, 0) };
    let [x, y] = e.offset;
    // The nearest of the eight directions toward it (direction k + 1 points at k·45°).
    let k = ((y.atan2(x) / std::f32::consts::FRAC_PI_4).round() as i32).rem_euclid(8) as usize;
    let reach = (x * x + y * y).sqrt() < 1.6;
    Action::new(k + 1, if reach { 2 } else { 0 })
}

/// The headless game with the agent's controls and books added, starting straight into play.
fn build(seed: u32, store: Store) -> App {
    game3d::headless::app(store, |app| {
        // Spread the seed's bits so neighbouring seeds don't start with neighbouring dice.
        let spread = seed.wrapping_mul(0x85EB_CA6B).rotate_left(13) ^ 0xC2B2_AE35;
        app.insert_resource(Waves { seed: spread, ..default() })
            .init_resource::<Command>()
            .init_resource::<Tally>()
            .add_systems(PostStartup, |mut next: ResMut<NextState<Phase>>| next.set(Phase::Playing))
            .add_systems(PreUpdate, drive.after(ReadActions))
            .add_systems(Update, tally.in_set(Frame::Show));
    })
}

fn drive(command: Res<Command>, mut actions: ResMut<Actions>) {
    *actions = command.0;
}

fn tally(mut defeated: MessageReader<Defeated>, mut ended: MessageReader<BoxEnded>, mut tally: ResMut<Tally>) {
    for d in defeated.read() {
        if d.team == Team::Enemy {
            tally.0.kills += 1;
        }
    }
    for e in ended.read() {
        match e {
            BoxEnded::Escaped { .. } => tally.0.escapes += 1,
            BoxEnded::Died { .. } => tally.0.died = true,
        }
    }
}

/// FNV-1a over the floats' bits.
fn hash(v: &[f32]) -> u64 {
    v.iter()
        .flat_map(|x| x.to_bits().to_le_bytes())
        .fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3))
}

/// An observation before the first frame has run (never returned).
fn placeholder() -> Obs {
    Obs {
        player: Default::default(),
        enemies: Vec::new(),
        heights: Vec::new(),
        floor: [1.0, 1.0],
        rim: 1.0,
        box_number: 1,
        time_left: 0.0,
        timer: 1.0,
        doors: [false; 4],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(hp: f32, best_pile: f32) -> Progress {
        Progress { box_number: 1, rim: 1.5, hp, max_hp: 100.0, best_pile }
    }

    #[test]
    fn a_quiet_step_costs_only_the_step() {
        assert_eq!(reward(&at(100.0, 0.0), &at(100.0, 0.0), &Events::default()), -STEP);
    }

    #[test]
    fn kills_pay_each() {
        let e = Events { kills: 3, ..default() };
        assert!((reward(&at(100.0, 0.0), &at(100.0, 0.0), &e) - (3.0 * KILL - STEP)).abs() < 1e-6);
    }

    #[test]
    fn climbing_pays_for_new_heights_in_rims_only() {
        let up = reward(&at(100.0, 0.0), &at(100.0, 0.75), &Events::default());
        assert!((up - (CLIMB * 0.5 - STEP)).abs() < 1e-6, "half a rim higher: {up}");
        // Stepping down and back up to the old best pays nothing more: Progress keeps the best.
        let before = at(100.0, 0.75);
        let mut obs = placeholder();
        obs.rim = 1.5;
        obs.player.max_hp = 100.0;
        obs.player.hp = 100.0;
        obs.player.pile = 0.2;
        obs.player.wall_gap = 0.4;
        let after = Progress::after(&before, &obs);
        assert_eq!(after.best_pile, 0.75, "the best stands");
        assert_eq!(reward(&before, &after, &Events::default()), -STEP);
    }

    #[test]
    fn a_pile_out_in_the_middle_pays_nothing_and_one_at_a_wall_does() {
        let mut obs = placeholder();
        obs.rim = 1.5;
        obs.player.max_hp = 100.0;
        obs.player.hp = 100.0;
        obs.player.pile = 0.75;
        obs.player.wall_gap = 3.0;
        let before = at(100.0, 0.0);
        let middle = Progress::after(&before, &obs);
        assert_eq!(reward(&before, &middle, &Events::default()), -STEP, "the middle: no way out");
        obs.player.wall_gap = 0.5;
        let wall = Progress::after(&before, &obs);
        assert!((reward(&before, &wall, &Events::default()) - (CLIMB * 0.5 - STEP)).abs() < 1e-6);
    }

    #[test]
    fn escaping_pays_and_the_next_box_starts_its_climb_from_the_floor() {
        let mut obs = placeholder();
        obs.box_number = 2;
        obs.rim = 2.0;
        obs.player.max_hp = 100.0;
        obs.player.hp = 100.0;
        let before = at(80.0, 1.5);
        let after = Progress::after(&before, &obs);
        assert_eq!(after.best_pile, 0.0);
        let r = reward(&before, &after, &Events { escapes: 1, ..default() });
        assert!((r - (ESCAPE - STEP)).abs() < 1e-6, "the refill isn't rewarded, the drop in best isn't punished: {r}");
    }

    #[test]
    fn harm_costs_its_share_of_health_and_death_costs_more() {
        let r = reward(&at(100.0, 0.0), &at(75.0, 0.0), &Events::default());
        assert!((r - (-HURT * 0.25 - STEP)).abs() < 1e-6, "{r}");
        let r = reward(&at(10.0, 0.0), &at(0.0, 0.0), &Events { died: true, ..default() });
        assert!((r - (-HURT * 0.1 - DEATH - STEP)).abs() < 1e-6, "{r}");
    }
}
