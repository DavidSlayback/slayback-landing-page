//! One set of actions from every input device: keyboard, mouse and any standard gamepad, through the mapping shared
//! with the 2.5D game (`game_core::controls`, which has the table). Gameplay reads [`Actions`]; only this module reads
//! devices. A standard gamepad needs nothing set up: the left stick or d-pad moves, A or the right trigger dashes,
//! X punches, Y kicks, B grabs, Start pauses or starts, and Select toggles sound on the pause screen. A touch screen
//! gets the twin sticks of `game_core::touch` (Q82): the left moves, the right aims and punches, with Kick, Grab and
//! Dash above it; a tap on the HUD pauses, and a tap anywhere leaves a screen.

use bevy::input::InputSystems;
use bevy::input::touch::{TouchInput, TouchPhase};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use game_core::controls::{Pad, Verbs, first_direction, keys_direction};
use game_core::touch::TouchPad;

use crate::flow::Phase;

/// This frame's actions.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq)]
pub struct Actions {
    /// Move direction on the floor as a `game_core` plane vector: one of eight unit directions on the screen, turned
    /// by [`ViewYaw`] onto the floor, or zero.
    pub dir: [f32; 2],
    /// Verbs just pressed this frame.
    pub verbs: Verbs,
    /// Where to face, apart from the move (a `game_core` plane vector, unit length, turned like `dir`): the touch
    /// aim stick, while it's pushed. None for every other device, which faces the way it moves.
    pub aim: Option<[f32; 2]>,
}

/// Which way the camera looks across the floor (Q65): its yaw, radians, turning the view's "up the screen" from -z
/// toward -x. The camera writes it; movement is read relative to it, so up on the keys or the stick is always up the
/// screen. Headless scenarios leave it at zero, where screen up is -z.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq)]
pub struct ViewYaw(pub f32);

/// A move direction read off the screen, turned onto the floor for a camera at `yaw` (both `game_core` plane
/// vectors, y away from the camera).
pub fn to_floor([x, y]: [f32; 2], yaw: f32) -> [f32; 2] {
    let (s, c) = yaw.sin_cos();
    [x * c - y * s, x * s + y * c]
}

/// Where actions are gathered; anything that fakes input runs before it.
#[derive(SystemSet, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ReadActions;

pub const DASH_KEYS: [KeyCode; 3] = [KeyCode::Space, KeyCode::ShiftLeft, KeyCode::ShiftRight];
pub const PUNCH_KEY: KeyCode = KeyCode::KeyJ;
pub const KICK_KEY: KeyCode = KeyCode::KeyK;
pub const GRAB_KEY: KeyCode = KeyCode::KeyL;
pub const BEGIN_KEYS: [KeyCode; 2] = [KeyCode::Enter, KeyCode::Space];

/// The touch controls, and whether to show them: from the start on a touch screen, else from the first touch.
#[derive(Resource)]
pub struct Touch {
    pub pad: TouchPad,
    pub used: bool,
    /// The phase the last frame was in and when it began (real time, seconds): a touch that leaves a screen does
    /// nothing in the next, and a screen ignores taps for its first [`SETTLE`], so a fight's last frantic taps don't
    /// skip the game-over screen.
    phase: Option<Phase>,
    since: f64,
}

/// See [`Touch::since`].
const SETTLE: f64 = 0.6;

/// How far down the pause screen the sound toggle's band starts, as a fraction of the height.
pub const SOUND_BAND: f32 = 0.75;

impl Default for Touch {
    fn default() -> Self {
        Self { pad: TouchPad::new([1280.0, 720.0]), used: touch_screen(), phase: None, since: 0.0 }
    }
}

/// Whether the page's main pointer is a finger (`pointer: coarse`), or natively `TOUCH=1`.
fn touch_screen() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window().and_then(|w| w.match_media("(pointer: coarse)").ok().flatten()).is_some_and(|m| m.matches())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var("TOUCH").is_ok_and(|v| v == "1")
    }
}

pub fn plugin(app: &mut App) {
    // Registered here too, so the headless scenarios (no input plugin) can send touches.
    app.add_message::<TouchInput>()
        .init_resource::<Actions>()
        .init_resource::<ViewYaw>()
        .init_resource::<Touch>()
        .add_systems(PreUpdate, (read, read_touch).chain().in_set(ReadActions).after(InputSystems));
}

/// One gamepad's state, as the shared mapping needs it. The d-pad is read as buttons (pressed or not), so it works
/// whether or not the backend also reports them as analog values.
fn pad(g: &Gamepad) -> Pad {
    let axis = |plus, minus| (g.pressed(plus) as i8 - g.pressed(minus) as i8) as f32;
    Pad {
        stick: g.left_stick().to_array(),
        dpad: [
            axis(GamepadButton::DPadRight, GamepadButton::DPadLeft),
            axis(GamepadButton::DPadUp, GamepadButton::DPadDown),
        ],
        south: g.just_pressed(GamepadButton::South),
        east: g.just_pressed(GamepadButton::East),
        north: g.just_pressed(GamepadButton::North),
        west: g.just_pressed(GamepadButton::West),
        right_trigger: g.just_pressed(GamepadButton::RightTrigger) || g.just_pressed(GamepadButton::RightTrigger2),
        start: g.just_pressed(GamepadButton::Start),
        select: g.just_pressed(GamepadButton::Select),
    }
}

fn read(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    pads: Query<&Gamepad>,
    view: Res<ViewYaw>,
    mut actions: ResMut<Actions>,
) {
    let held = |a: KeyCode, b: KeyCode| keys.pressed(a) || keys.pressed(b);
    let click = |b: MouseButton| mouse.as_ref().is_some_and(|m| m.just_pressed(b));
    let mut dir = keys_direction(
        held(KeyCode::KeyD, KeyCode::ArrowRight),
        held(KeyCode::KeyA, KeyCode::ArrowLeft),
        held(KeyCode::KeyW, KeyCode::ArrowUp),
        held(KeyCode::KeyS, KeyCode::ArrowDown),
    );
    let mut verbs = Verbs {
        dash: keys.any_just_pressed(DASH_KEYS),
        punch: keys.just_pressed(PUNCH_KEY) || click(MouseButton::Left),
        kick: keys.just_pressed(KICK_KEY) || click(MouseButton::Right),
        grab: keys.just_pressed(GRAB_KEY) || click(MouseButton::Middle),
        pause: keys.just_pressed(KeyCode::Escape),
        begin: keys.any_just_pressed(BEGIN_KEYS) || click(MouseButton::Left),
        sound: keys.just_pressed(KeyCode::KeyM),
    };
    for g in &pads {
        let p = pad(g);
        dir = first_direction(dir, p.direction());
        verbs = verbs.or(p.verbs());
    }
    *actions = Actions { dir: to_floor(dir, view.0), verbs, aim: None };
}

/// Adds the touch controls' frame to the actions: in play, their move, aim and verbs; on the pause screen, a tap resumes (or,
/// low down, toggles sound); on the start and game-over screens, a tap starts.
fn read_touch(
    mut touches: MessageReader<TouchInput>,
    mut touch: ResMut<Touch>,
    windows: Query<&Window, With<PrimaryWindow>>,
    time: Res<Time<Real>>,
    phase: Option<Res<State<Phase>>>,
    view: Res<ViewYaw>,
    mut actions: ResMut<Actions>,
) {
    let now = time.elapsed_secs_f64();
    if let Ok(w) = windows.single() {
        touch.pad.layout.size = [w.width(), w.height()];
    }
    let phase = phase.map(|p| *p.get());
    if touch.phase != phase {
        touch.pad.clear();
        touch.phase = phase;
        touch.since = now;
    }
    for t in touches.read() {
        let (id, at) = (t.id, t.position.to_array());
        match t.phase {
            TouchPhase::Started => {
                touch.used = true;
                touch.pad.down(id, at);
            }
            TouchPhase::Moved => touch.pad.moved(id, at),
            TouchPhase::Ended => touch.pad.up(id),
            TouchPhase::Canceled => touch.pad.cancel(id),
        }
    }
    let frame = touch.pad.frame();
    let settled = now - touch.since >= SETTLE;
    let verbs = &mut actions.verbs;
    match phase {
        None | Some(Phase::Playing) => {
            *verbs = verbs.or(frame.verbs);
            if actions.dir == [0.0, 0.0] {
                actions.dir = to_floor(frame.dir, view.0);
            }
            actions.aim = frame.aim.map(|a| to_floor(a, view.0));
        }
        Some(Phase::Paused) => {
            if let Some([_, y]) = frame.landed {
                let low = y > touch.pad.layout.size[1] * SOUND_BAND;
                verbs.sound |= low;
                verbs.pause |= !low;
            }
        }
        Some(Phase::Start | Phase::Over) => verbs.begin |= frame.landed.is_some() && settled,
    }
}
