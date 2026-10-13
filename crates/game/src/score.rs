//! The run's score (rules: `game_core::score`) and the best score on record, kept in the browser's localStorage
//! (`slayback-best`) or, natively, a file in the temp directory.

use bevy::prelude::*;
use game_core::score::{Best, Score, iso_date};

use crate::fighter::{Defeated, Team};
use crate::flow::Phase;
use crate::run::BoxEnded;

/// The current run's score.
#[derive(Resource, Default)]
pub struct RunScore(pub Score);

/// The best run on record, if any.
#[derive(Resource, Default)]
pub struct BestScore(pub Option<Best>);

/// How the last run ended, for the game-over screen.
#[derive(Resource, Default, Clone, Copy)]
pub struct LastRun {
    pub score: u64,
    pub reached: usize,
    pub new_best: bool,
}

/// A kill scored `points` at `at`, as the `streak`th of a streak (for floating score numbers).
#[derive(Message, Clone, Copy, Debug)]
pub struct Scored {
    pub at: Vec2,
    pub points: u64,
    pub streak: u32,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<RunScore>()
        .init_resource::<LastRun>()
        .add_message::<Scored>()
        .insert_resource(BestScore(load()))
        .add_systems(Update, count.run_if(in_state(Phase::Playing)))
        // In every phase: a death's message arrives as the game switches to the game-over screen.
        .add_systems(Update, tally)
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

fn tally(
    mut ended: MessageReader<BoxEnded>,
    mut score: ResMut<RunScore>,
    mut best: ResMut<BestScore>,
    mut last: ResMut<LastRun>,
) {
    for e in ended.read() {
        match *e {
            BoxEnded::Escaped { number, time_left } => {
                score.0.escape(time_left, number);
            }
            BoxEnded::Died { reached } => {
                let points = score.0.points;
                let new_best = best.0.as_ref().is_none_or(|b| b.beaten_by(points)) && points > 0;
                if new_best {
                    let record = Best { score: points, reached, date: today() };
                    save(&record);
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

const KEY: &str = "slayback-best";

#[cfg(target_arch = "wasm32")]
fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

fn load() -> Option<Best> {
    #[cfg(target_arch = "wasm32")]
    let text = storage()?.get_item(KEY).ok()??;
    #[cfg(not(target_arch = "wasm32"))]
    let text = std::fs::read_to_string(std::env::temp_dir().join(KEY)).ok()?;
    Best::from_record(&text)
}

fn save(best: &Best) {
    // Losing the record (private browsing, blocked storage) only costs the "best" line, so failures are ignored.
    #[cfg(target_arch = "wasm32")]
    if let Some(s) = storage() {
        let _ = s.set_item(KEY, &best.to_record());
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = std::fs::write(std::env::temp_dir().join(KEY), best.to_record());
}
