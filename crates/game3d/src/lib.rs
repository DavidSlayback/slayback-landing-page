//! Escape the box in 3D (spec #58): an open-topped box seen from above at a 3/4 angle, built beside the 2.5D `game`
//! on the shared engine-free `game-core`, and served at `/play` by `GAME=game3d bash scripts/build.sh`.
//!
//! The library holds the gameplay ([`plugin`]) and, separately, its look ([`look::plugin`]) and camera
//! ([`camera::plugin`]); `main.rs` adds the window and renderer. Keeping them apart lets tests build a headless app
//! from the real game systems (see `tests/scenarios.rs`). Units are metres; y is up, and the camera looks toward -z.

pub mod arena;
pub mod camera;
pub mod debris;
pub mod enemy;
pub mod fighter;
#[cfg(feature = "render")]
pub mod figure;
#[cfg(feature = "render")]
pub mod hud;
#[cfg(feature = "render")]
pub mod impact;
pub mod input;
#[cfg(feature = "render")]
pub mod look;
pub mod player;
pub mod score;

use avian3d::prelude::*;
use bevy::prelude::*;

/// Gravity, m/s²: heavier than Earth's, so falls and tumbles read snappily at this camera distance.
pub const GRAVITY: f32 = 15.0;

/// Everything that makes up the game except windowing and rendering: physics, the box, fighting, the player, enemies,
/// their debris and the score.
pub fn plugin(app: &mut App) {
    app.add_plugins(PhysicsPlugins::default()).insert_resource(Gravity(Vec3::NEG_Y * GRAVITY)).add_plugins((
        input::plugin,
        arena::plugin,
        fighter::plugin,
        player::plugin,
        enemy::plugin,
        debris::plugin,
        score::plugin,
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
