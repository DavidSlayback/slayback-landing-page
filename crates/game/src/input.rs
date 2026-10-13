//! One set of actions from every input device (D11): keyboard, mouse and any standard gamepad map to the same verbs,
//! with nothing to configure. Gameplay reads [`Actions`]; only this module reads devices.
//!
//! | Action | Keyboard | Mouse | Gamepad |
//! |---|---|---|---|
//! | Run | A / D, arrows | | left stick, d-pad |
//! | Dash | Space, Shift | | A (south), right trigger |
//! | Punch | J | left | X (west) |
//! | Kick | K | right | Y (north) |
//! | Grab / set down | L | middle | B (east) |
//! | Pause | Esc | | Start |
//! | Start / again | Enter, Space | left | Start, A |
//! | Sound (paused) | M | | Select |

use bevy::input::InputSystems;
use bevy::prelude::*;

/// This frame's actions. Everything but `axis` is "just pressed".
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq)]
pub struct Actions {
    /// Run direction, -1..=1.
    pub axis: f32,
    pub dash: bool,
    pub punch: bool,
    pub kick: bool,
    pub grab: bool,
    pub pause: bool,
    pub begin: bool,
    pub sound: bool,
}

/// Where actions are gathered; anything that fakes input (the demo) runs before it.
#[derive(SystemSet, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ReadActions;

pub const DASH_KEYS: [KeyCode; 3] = [KeyCode::Space, KeyCode::ShiftLeft, KeyCode::ShiftRight];
pub const PUNCH_KEY: KeyCode = KeyCode::KeyJ;
pub const KICK_KEY: KeyCode = KeyCode::KeyK;
pub const GRAB_KEY: KeyCode = KeyCode::KeyL;
pub const BEGIN_KEYS: [KeyCode; 2] = [KeyCode::Enter, KeyCode::Space];

/// Stick travel below this is ignored (worn sticks drift).
const DEADZONE: f32 = 0.3;

pub fn plugin(app: &mut App) {
    app.init_resource::<Actions>().add_systems(PreUpdate, read.in_set(ReadActions).after(InputSystems));
}

/// One gamepad's state, as the mapping needs it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pad {
    pub stick_x: f32,
    pub dpad_x: f32,
    /// Buttons just pressed this frame.
    pub south: bool,
    pub east: bool,
    pub north: bool,
    pub west: bool,
    pub right_trigger: bool,
    pub start: bool,
    pub select: bool,
}

impl Pad {
    fn from(g: &Gamepad) -> Self {
        Self {
            stick_x: g.left_stick().x,
            dpad_x: g.dpad().x,
            south: g.just_pressed(GamepadButton::South),
            east: g.just_pressed(GamepadButton::East),
            north: g.just_pressed(GamepadButton::North),
            west: g.just_pressed(GamepadButton::West),
            right_trigger: g.just_pressed(GamepadButton::RightTrigger) || g.just_pressed(GamepadButton::RightTrigger2),
            start: g.just_pressed(GamepadButton::Start),
            select: g.just_pressed(GamepadButton::Select),
        }
    }
}

/// A gamepad's actions (the table above).
pub fn pad_actions(p: Pad) -> Actions {
    let axis = if p.dpad_x != 0.0 { p.dpad_x.signum() } else if p.stick_x.abs() >= DEADZONE { p.stick_x.signum() } else { 0.0 };
    Actions {
        axis,
        dash: p.south || p.right_trigger,
        punch: p.west,
        kick: p.north,
        grab: p.east,
        pause: p.start,
        begin: p.start || p.south,
        sound: p.select,
    }
}

/// Combines two devices: either one pressing counts; the axis comes from whichever is pushed.
pub fn merge(a: Actions, b: Actions) -> Actions {
    Actions {
        axis: if a.axis != 0.0 { a.axis } else { b.axis },
        dash: a.dash || b.dash,
        punch: a.punch || b.punch,
        kick: a.kick || b.kick,
        grab: a.grab || b.grab,
        pause: a.pause || b.pause,
        begin: a.begin || b.begin,
        sound: a.sound || b.sound,
    }
}

fn read(keys: Res<ButtonInput<KeyCode>>, mouse: Option<Res<ButtonInput<MouseButton>>>, pads: Query<&Gamepad>, mut actions: ResMut<Actions>) {
    let held = |a: KeyCode, b: KeyCode| keys.pressed(a) || keys.pressed(b);
    let click = |b: MouseButton| mouse.as_ref().is_some_and(|m| m.just_pressed(b));
    let mut all = Actions {
        axis: (held(KeyCode::KeyD, KeyCode::ArrowRight) as i8 - held(KeyCode::KeyA, KeyCode::ArrowLeft) as i8) as f32,
        dash: keys.any_just_pressed(DASH_KEYS),
        punch: keys.just_pressed(PUNCH_KEY) || click(MouseButton::Left),
        kick: keys.just_pressed(KICK_KEY) || click(MouseButton::Right),
        grab: keys.just_pressed(GRAB_KEY) || click(MouseButton::Middle),
        pause: keys.just_pressed(KeyCode::Escape),
        begin: keys.any_just_pressed(BEGIN_KEYS) || click(MouseButton::Left),
        sound: keys.just_pressed(KeyCode::KeyM),
    };
    for pad in &pads {
        all = merge(all, pad_actions(Pad::from(pad)));
    }
    *actions = all;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stick_has_a_deadzone_and_the_dpad_wins() {
        assert_eq!(pad_actions(Pad { stick_x: 0.2, ..default() }).axis, 0.0);
        assert_eq!(pad_actions(Pad { stick_x: -0.7, ..default() }).axis, -1.0);
        assert_eq!(pad_actions(Pad { stick_x: -0.7, dpad_x: 1.0, ..default() }).axis, 1.0);
    }

    #[test]
    fn every_verb_has_a_button() {
        let a = |p: Pad| pad_actions(p);
        assert!(a(Pad { south: true, ..default() }).dash && a(Pad { right_trigger: true, ..default() }).dash);
        assert!(a(Pad { west: true, ..default() }).punch);
        assert!(a(Pad { north: true, ..default() }).kick);
        assert!(a(Pad { east: true, ..default() }).grab);
        assert!(a(Pad { start: true, ..default() }).pause && a(Pad { start: true, ..default() }).begin);
        assert!(a(Pad { select: true, ..default() }).sound);
    }

    #[test]
    fn two_devices_combine() {
        let keys = Actions { punch: true, ..default() };
        let pad = Actions { axis: 1.0, dash: true, ..default() };
        let both = merge(keys, pad);
        assert!(both.punch && both.dash && both.axis == 1.0);
    }
}
