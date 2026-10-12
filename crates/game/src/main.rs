//! Escape the box: the portfolio's playable brawler (Bevy + Avian), served lazily at `/play`.
//!
//! Slice 1 (tracer bullet): a box arena and a player that runs and jumps, built for the web.

mod arena;
mod player;

use avian2d::prelude::*;
use bevy::prelude::*;

/// Logical arena size in world units; the camera scales it to fit any window.
pub const ARENA: Vec2 = Vec2::new(1280.0, 720.0);

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Escape the box".into(),
                canvas: Some("#game".into()),
                fit_canvas_to_parent: true,
                prevent_default_event_handling: true,
                ..default()
            }),
            ..default()
        }))
        // ~32 world units per metre keeps Avian's internal tolerances sensible at pixel scale.
        .add_plugins(PhysicsPlugins::default().with_length_unit(32.0))
        .insert_resource(Gravity(Vec2::NEG_Y * 2000.0))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins((arena::plugin, player::plugin))
        .run();
}
