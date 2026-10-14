//! The run's score (rules: `game_core::score`): each kill is worth 100 times a streak multiplier. The best score and the
//! time bonus on escape come with the run loop (G3) and the screens.

use bevy::prelude::*;
use game_core::score::Score;

use crate::fighter::{Combat, Defeated, Team};

/// The current run's score.
#[derive(Resource, Default)]
pub struct RunScore(pub Score);

/// A kill scored `points` at `at`, as the `streak`th of a streak (for floating score numbers).
#[derive(Message, Clone, Copy, Debug)]
pub struct Scored {
    pub at: Vec3,
    pub points: u64,
    pub streak: u32,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<RunScore>().add_message::<Scored>().add_systems(Update, count.after(Combat));
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
