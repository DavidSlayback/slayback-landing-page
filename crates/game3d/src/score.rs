//! The run's score (rules: `game_core::score`): each kill is worth 100 times a streak multiplier, and escaping a box
//! adds a bonus for the time left. It carries from box to box and starts again with a new run. The best run on record
//! is kept in the 2.5D game's format (`game_core::score::Best` under `slayback-best`, see [`Store`]), so a best set in
//! either game counts in both.

use bevy::prelude::*;
use game_core::score::{Best, Score, iso_date};

use crate::fighter::{Combat, Defeated, Team};
use crate::flow::Phase;
use crate::run::BoxEnded;
use crate::store::Store;

/// The current run's score.
#[derive(Resource, Default)]
pub struct RunScore(pub Score);

/// The best run on record, if any.
#[derive(Resource, Default)]
pub struct BestScore(pub Option<Best>);

/// How the last run ended, for the game-over screen.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq)]
pub struct LastRun {
    pub score: u64,
    pub reached: usize,
    pub new_best: bool,
}

/// A kill scored `points` at `at`, as the `streak`th of a streak (for floating score numbers).
#[derive(Message, Clone, Copy, Debug)]
pub struct Scored {
    pub at: Vec3,
    pub points: u64,
    pub streak: u32,
}

/// The best score's key in the [`Store`], shared with the 2.5D game.
pub const BEST_KEY: &str = "slayback-best";

pub fn plugin(app: &mut App) {
    app.init_resource::<RunScore>()
        .init_resource::<BestScore>()
        .init_resource::<LastRun>()
        .init_resource::<Store>()
        .add_message::<Scored>()
        // At startup rather than now, so a test (or anything else) can point the store elsewhere first.
        .add_systems(Startup, |store: Res<Store>, mut best: ResMut<BestScore>| {
            best.0 = store.get(BEST_KEY).as_deref().and_then(Best::from_record);
        })
        .add_systems(Update, count.after(Combat).run_if(in_state(Phase::Playing)))
        // In every phase: a death's message arrives as the game switches to the game-over screen.
        .add_systems(Update, tally.after(Combat))
        .add_systems(OnExit(Phase::Over), |mut score: ResMut<RunScore>| score.0 = Score::default());
}

fn count(
    time: Res<Time>,
    mut score: ResMut<RunScore>,
    mut defeats: MessageReader<Defeated>,
    mut scored: MessageWriter<Scored>,
) {
    score.0.tick(time.delta_secs());
    for d in defeats.read().filter(|d| d.team == Team::Enemy) {
        let points = score.0.kill();
        scored.write(Scored { at: d.at, points, streak: score.0.streak });
    }
}

/// An escape adds its time bonus; a death settles the run: its score against the best on record.
fn tally(
    mut ended: MessageReader<BoxEnded>,
    mut score: ResMut<RunScore>,
    mut best: ResMut<BestScore>,
    mut last: ResMut<LastRun>,
    store: Res<Store>,
) {
    for end in ended.read() {
        match *end {
            BoxEnded::Escaped { number, time_left } => {
                score.0.escape(time_left, number);
            }
            BoxEnded::Died { reached } => {
                let points = score.0.points;
                let new_best = points > 0 && best.0.as_ref().is_none_or(|b| b.beaten_by(points));
                if new_best {
                    let record = Best { score: points, reached, date: today() };
                    store.set(BEST_KEY, &record.to_record());
                    best.0 = Some(record);
                }
                *last = LastRun { score: points, reached, new_best };
            }
        }
    }
}

fn today() -> String {
    #[cfg(target_arch = "wasm32")]
    let seconds = (js_sys::Date::now() / 1000.0) as u64;
    #[cfg(not(target_arch = "wasm32"))]
    let seconds = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    iso_date(seconds)
}
