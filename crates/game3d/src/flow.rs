//! Where the game is: at the start overlay, playing, paused, or over. Anything but playing stops the game clock
//! (`Time<Virtual>`), and with it physics, the box clock, swings and the score's streak. As in the 2.5D game.
//!
//! The gameplay starts in `Playing`, so headless scenarios just run; the screens put a fresh page on the start
//! overlay, and the demo skips it.

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::window::WindowFocused;

use crate::input::Actions;

#[derive(States, Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Phase {
    /// The start overlay: the controls, and "click to start".
    Start,
    #[default]
    Playing,
    /// Esc, or the page lost focus.
    Paused,
    /// The player died: score, box reached, best, and "again".
    Over,
}

/// Whether losing focus pauses the game (the demo turns it off so scripted runs go on unattended).
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
        .add_systems(Update, (toggle_pause, begin, log.run_if(state_changed::<Phase>)))
        // Window focus only exists with a window (not in headless runs).
        .add_systems(Update, pause_on_blur.run_if(resource_exists::<Messages<WindowFocused>>));
    // In the browser, switching tabs doesn't blur the canvas, so also ask the page whether it has focus.
    #[cfg(target_arch = "wasm32")]
    app.add_systems(Update, pause_when_page_unfocused);
}

fn toggle_pause(actions: Res<Actions>, phase: Res<State<Phase>>, mut next: ResMut<NextState<Phase>>) {
    if actions.verbs.pause {
        match phase.get() {
            Phase::Playing => next.set(Phase::Paused),
            Phase::Paused => next.set(Phase::Playing),
            _ => {}
        }
    }
}

/// Click, Enter (or Space, or Start) leaves the start overlay or the game-over screen for a fresh run.
fn begin(actions: Res<Actions>, phase: Res<State<Phase>>, mut next: ResMut<NextState<Phase>>) {
    if actions.verbs.begin && matches!(phase.get(), Phase::Start | Phase::Over) {
        next.set(Phase::Playing);
    }
}

fn pause_on_blur(
    mut focus: MessageReader<WindowFocused>,
    on: Res<PauseOnBlur>,
    phase: Res<State<Phase>>,
    mut next: ResMut<NextState<Phase>>,
) {
    if focus.read().any(|f| !f.focused) && on.0 && *phase.get() == Phase::Playing {
        next.set(Phase::Paused);
    }
}

#[cfg(target_arch = "wasm32")]
fn pause_when_page_unfocused(on: Res<PauseOnBlur>, phase: Res<State<Phase>>, mut next: ResMut<NextState<Phase>>) {
    if !on.0 || *phase.get() != Phase::Playing {
        return;
    }
    let focused = web_sys::window().and_then(|w| w.document()).is_none_or(|d| d.has_focus().unwrap_or(true));
    if !focused {
        next.set(Phase::Paused);
    }
}

/// Logged for the browser harness, which waits for "phase start" and then "phase playing". In the browser it is also
/// set on the page as `html[data-phase]`, which shows the tour clip on the start and pause screens (`screens`).
fn log(phase: Res<State<Phase>>) {
    let name = format!("{:?}", phase.get()).to_lowercase();
    info!("phase {name}");
    #[cfg(target_arch = "wasm32")]
    if let Some(root) = web_sys::window().and_then(|w| w.document()).and_then(|d| d.document_element()) {
        let _ = root.set_attribute("data-phase", &name);
    }
}
