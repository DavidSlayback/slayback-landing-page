//! The game's window and renderer around the gameplay and look in the `game` library.

use bevy::prelude::*;
use bevy::window::WindowResolution;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Escape the box".into(),
                canvas: Some("#game".into()),
                fit_canvas_to_parent: true,
                prevent_default_event_handling: true,
                // Ignore the display's pixel ratio: the host page caps the canvas at 1280x720 and scales it up.
                resolution: WindowResolution::default().with_scale_factor_override(1.0),
                ..default()
            }),
            ..default()
        }))
        .add_plugins((game::plugin, game::look::plugin))
        .run();
}
