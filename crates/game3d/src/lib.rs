//! Escape the box in 3D (spec #58): an open-topped box seen from above at a 3/4 angle, on the engine-free `game-core`,
//! and served at `/play` by `bash scripts/build.sh`. It replaced the 2.5D game in G10.
//!
//! The library holds the gameplay ([`plugin`]) and, separately, its look ([`look::plugin`]) and camera
//! ([`camera::plugin`]); `main.rs` adds the window and renderer. Keeping them apart lets tests build a headless app
//! from the real game systems (see `tests/scenarios.rs`). Units are metres; y is up, and the camera looks toward -z.

pub mod arena;
pub mod bomb;
pub mod camera;
pub mod comfort;
pub mod debris;
pub mod demo;
pub mod enemy;
pub mod fighter;
#[cfg(feature = "render")]
pub mod figure;
pub mod flow;
pub mod grab;
#[cfg(feature = "render")]
pub mod hud;
#[cfg(feature = "render")]
pub mod impact;
pub mod input;
#[cfg(feature = "render")]
pub mod look;
#[cfg(feature = "render")]
pub mod outline;
pub mod player;
pub mod pose;
pub mod run;
pub mod score;
#[cfg(feature = "render")]
pub mod screens;
pub mod sound;
pub mod store;

use avian3d::prelude::*;
use bevy::prelude::*;

/// Gravity, m/s²: heavier than Earth's, so falls and tumbles read snappily at this camera distance.
pub const GRAVITY: f32 = 15.0;

/// Everything that makes up the game except windowing and rendering: physics, the phases, the box, fighting, the
/// player, grabbing and throwing, enemies and their bomb-heads, their debris, the run, the score and best, the sound
/// setting, and comfort (reduced motion, quality scaling).
pub fn plugin(app: &mut App) {
    app.add_plugins(PhysicsPlugins::default()).insert_resource(Gravity(Vec3::NEG_Y * GRAVITY)).add_plugins((
        input::plugin,
        comfort::plugin,
        flow::plugin,
        arena::plugin,
        fighter::plugin,
        player::plugin,
        grab::plugin,
        bomb::plugin,
        enemy::plugin,
        debris::plugin,
        run::plugin,
        score::plugin,
        sound::plugin,
    ));
}

/// A plane vector from `game_core` (`[x, y]`, y away from the camera) on the floor, as a world direction.
pub fn on_floor(v: [f32; 2]) -> Vec3 {
    Vec3::new(v[0], 0.0, -v[1])
}

/// A world vector's floor part as a `game_core` plane vector.
pub fn plane(v: Vec3) -> [f32; 2] {
    [v.x, -v.z]
}
