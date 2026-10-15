//! One set of actions from every input device: keyboard, mouse and any standard gamepad, through the mapping shared
//! with the 2.5D game (`game_core::controls`, which has the table). Gameplay reads [`Actions`]; only this module reads
//! devices. A standard gamepad needs nothing set up: the left stick or d-pad moves, A or the right trigger dashes,
//! X punches, Y kicks, B grabs, Start pauses or starts, and Select toggles sound on the pause screen.

use bevy::input::InputSystems;
use bevy::prelude::*;
use game_core::controls::{Pad, Verbs, first_direction, keys_direction};

/// This frame's actions.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq)]
pub struct Actions {
    /// Move direction on the floor as a `game_core` plane vector: one of eight unit directions on the screen, turned
    /// by [`ViewYaw`] onto the floor, or zero.
    pub dir: [f32; 2],
    /// Verbs just pressed this frame.
    pub verbs: Verbs,
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

pub fn plugin(app: &mut App) {
    app.init_resource::<Actions>()
        .init_resource::<ViewYaw>()
        .add_systems(PreUpdate, read.in_set(ReadActions).after(InputSystems));
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
    *actions = Actions { dir: to_floor(dir, view.0), verbs };
}
