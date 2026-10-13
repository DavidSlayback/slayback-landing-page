//! Where the game is: playing, paused, at the start overlay, or over. Anything but playing stops the game clock.
//!
//! The gameplay starts in `Playing` (so headless scenarios just run); the UI puts a fresh page on the start overlay.

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::window::WindowFocused;

use crate::input::Actions;

#[derive(States, Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Phase {
    /// The start overlay: controls, and "click to start".
    Start,
    #[default]
    Playing,
    /// Esc, or the tab lost focus.
    Paused,
    /// The player died: score, best, and "again".
    Over,
}

/// Whether losing focus pauses the game (the scripted demo turns it off so recordings run unattended).
#[derive(Resource)]
pub struct PauseOnBlur(pub bool);

pub fn plugin(app: &mut App) {
    if !app.is_plugin_added::<StatesPlugin>() {
        app.add_plugins(StatesPlugin);
    }
    app.init_state::<Phase>()
        .insert_resource(PauseOnBlur(true))
        .add_systems(OnEnter(Phase::Playing), |mut time: ResMut<Time<Virtual>>| time.unpause())
        .add_systems(OnExit(Phase::Playing), |mut time: ResMut<Time<Virtual>>| time.pause())
        .add_systems(Update, (toggle_pause, begin))
        // Window focus only exists with a window (not in headless runs).
        .add_systems(Update, pause_on_blur.run_if(resource_exists::<Messages<WindowFocused>>));
    // In the browser, switching tabs doesn't blur the canvas, so also ask the page whether it has focus.
    #[cfg(target_arch = "wasm32")]
    app.add_systems(Update, pause_when_page_unfocused);
}

fn toggle_pause(actions: Res<Actions>, phase: Res<State<Phase>>, mut next: ResMut<NextState<Phase>>) {
    if actions.pause {
        match phase.get() {
            Phase::Playing => next.set(Phase::Paused),
            Phase::Paused => next.set(Phase::Playing),
            _ => {}
        }
    }
}

fn pause_on_blur(mut focus: MessageReader<WindowFocused>, on: Res<PauseOnBlur>, phase: Res<State<Phase>>, mut next: ResMut<NextState<Phase>>) {
    if focus.read().any(|f| !f.focused) && on.0 && *phase.get() == Phase::Playing {
        next.set(Phase::Paused);
    }
}

#[cfg(target_arch = "wasm32")]
fn pause_when_page_unfocused(on: Res<PauseOnBlur>, phase: Res<State<Phase>>, mut next: ResMut<NextState<Phase>>) {
    if !on.0 {
        return;
    }
    let focused = web_sys::window().and_then(|w| w.document()).is_none_or(|d| d.has_focus().unwrap_or(true));
    if !focused && *phase.get() == Phase::Playing {
        next.set(Phase::Paused);
    }
}

fn begin(actions: Res<Actions>, phase: Res<State<Phase>>, mut next: ResMut<NextState<Phase>>) {
    if actions.begin && matches!(phase.get(), Phase::Start | Phase::Over) {
        next.set(Phase::Playing);
    }
}
