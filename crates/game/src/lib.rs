//! Escape the box: the portfolio's playable brawler (Bevy + Avian), served lazily at `/play`.
//!
//! The library holds the gameplay ([`plugin`]) and, separately, its 2.5D look ([`look::plugin`]); `main.rs` adds the
//! window and renderer. Keeping them apart lets
//! tests build a headless app from the real game systems (see `tests/scenarios.rs`).

pub mod arena;
pub mod demo;
pub mod enemy;
pub mod fighter;
pub mod look;
pub mod player;

use avian2d::prelude::*;
use bevy::prelude::*;

/// Logical arena size in world units; the camera scales it to fit any window.
pub const ARENA: Vec2 = Vec2::new(1280.0, 720.0);

/// Gravity, world units/s².
pub const GRAVITY: f32 = 2000.0;

/// Everything that makes up the game except windowing and rendering: physics, the arena, fighting, the player and fodder.
pub fn plugin(app: &mut App) {
    // ~32 world units per metre keeps Avian's internal tolerances sensible at pixel scale.
    app.add_plugins(PhysicsPlugins::default().with_length_unit(32.0))
        .insert_resource(Gravity(Vec2::NEG_Y * GRAVITY))
        .add_plugins((arena::plugin, fighter::plugin, player::plugin, enemy::plugin));
}
