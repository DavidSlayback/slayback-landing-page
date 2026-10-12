//! Escape the box: the portfolio's playable brawler (Bevy + Avian), served lazily at `/play`.
//!
//! Slice 2 (combat): punch and kick with knockback, hit-stun and hit-stop against fodder that telegraph and punch back.

mod arena;
mod enemy;
mod fighter;
mod hud;
mod player;

use avian2d::prelude::*;
use bevy::prelude::*;

/// Logical arena size in world units; the camera scales it to fit any window.
pub const ARENA: Vec2 = Vec2::new(1280.0, 720.0);

/// The facing indicator child; flipped to the leading side every frame.
#[derive(Component)]
pub struct Visor;

/// Collision layers. Live fighters pass through each other (a crowd must never pin the player) but stand on
/// the world; debris (slice 3) will be solid to everyone so it can be piled and climbed.
#[derive(PhysicsLayer, Clone, Copy, Debug, Default)]
pub enum Layer {
    #[default]
    World,
    Fighter,
    Debris,
}

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
        .add_plugins((arena::plugin, fighter::plugin, player::plugin, enemy::plugin, hud::plugin))
        .add_systems(Update, face_visors)
        .run();
}

fn face_visors(fighters: Query<(&fighter::Fighter, &Children)>, mut visors: Query<&mut Transform, With<Visor>>) {
    for (fighter, children) in &fighters {
        for child in children.iter() {
            if let Ok(mut t) = visors.get_mut(child) {
                t.translation.x = 6.0 * fighter.facing;
            }
        }
    }
}
