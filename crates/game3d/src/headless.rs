//! The game without a window or renderer, stepped by hand on a fixed clock: the headless scenarios
//! (`tests/scenarios.rs`) and the RL environment (`crates/box-env`) both build it here, so they run the same game.

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;

use crate::store::Store;

/// Frames per second of game time: each `App::update` advances the game by one frame of this.
pub const FPS: f32 = 60.0;

/// A game app without window or renderer, keeping its records (the best score, the sound setting) in `store`.
/// `extra` runs before the app finishes building (resources such as `enemy::Waves`, comfort settings, plugins).
/// Keyboard state is a plain resource no input plugin clears, so a caller can drive it.
pub fn app(store: Store, extra: impl FnOnce(&mut App)) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, crate::plugin)).insert_resource(store);
    extra(&mut app);
    app.init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / FPS)));
    app.finish();
    app.cleanup();
    app
}
