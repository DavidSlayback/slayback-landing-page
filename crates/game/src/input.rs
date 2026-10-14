//! One set of actions from every input device (D11): keyboard, mouse and any standard gamepad map to the same verbs,
//! with nothing to configure. Gameplay reads [`Actions`]; only this module reads devices. The mapping table and the
//! gamepad rules are shared with the 3D game in `game_core::controls`; this side runs on A / D only.

use bevy::input::InputSystems;
use bevy::prelude::*;
use game_core::controls::{Pad, Verbs};

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

impl Actions {
    fn new(axis: f32, v: Verbs) -> Self {
        let Verbs { dash, punch, kick, grab, pause, begin, sound } = v;
        Self { axis, dash, punch, kick, grab, pause, begin, sound }
    }

    fn verbs(&self) -> Verbs {
        let Self { dash, punch, kick, grab, pause, begin, sound, .. } = *self;
        Verbs { dash, punch, kick, grab, pause, begin, sound }
    }
}

/// Where actions are gathered; anything that fakes input (the demo) runs before it.
#[derive(SystemSet, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ReadActions;

pub const DASH_KEYS: [KeyCode; 3] = [KeyCode::Space, KeyCode::ShiftLeft, KeyCode::ShiftRight];
pub const PUNCH_KEY: KeyCode = KeyCode::KeyJ;
pub const KICK_KEY: KeyCode = KeyCode::KeyK;
pub const GRAB_KEY: KeyCode = KeyCode::KeyL;
pub const BEGIN_KEYS: [KeyCode; 2] = [KeyCode::Enter, KeyCode::Space];

pub fn plugin(app: &mut App) {
    app.init_resource::<Actions>().add_systems(PreUpdate, read.in_set(ReadActions).after(InputSystems));
}

/// One gamepad's state, as the shared mapping needs it.
fn pad(g: &Gamepad) -> Pad {
    Pad {
        stick: g.left_stick().to_array(),
        dpad: g.dpad().to_array(),
        south: g.just_pressed(GamepadButton::South),
        east: g.just_pressed(GamepadButton::East),
        north: g.just_pressed(GamepadButton::North),
        west: g.just_pressed(GamepadButton::West),
        right_trigger: g.just_pressed(GamepadButton::RightTrigger) || g.just_pressed(GamepadButton::RightTrigger2),
        start: g.just_pressed(GamepadButton::Start),
        select: g.just_pressed(GamepadButton::Select),
    }
}

/// A gamepad's actions (the table in `game_core::controls`).
pub fn pad_actions(p: Pad) -> Actions {
    Actions::new(p.axis(), p.verbs())
}

/// Combines two devices: either one pressing counts; the axis comes from whichever is pushed.
pub fn merge(a: Actions, b: Actions) -> Actions {
    Actions::new(if a.axis != 0.0 { a.axis } else { b.axis }, a.verbs().or(b.verbs()))
}

fn read(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    pads: Query<&Gamepad>,
    mut actions: ResMut<Actions>,
) {
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
    for g in &pads {
        all = merge(all, pad_actions(pad(g)));
    }
    *actions = all;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pad_runs_on_its_x_axis_with_every_verb() {
        let a = pad_actions(Pad { stick: [-0.7, 0.9], west: true, ..default() });
        assert_eq!(a.axis, -1.0, "the stick's x only; up and down mean nothing in 2.5D");
        assert!(a.punch && !a.kick);
    }

    #[test]
    fn two_devices_combine() {
        let keys = Actions { punch: true, ..default() };
        let pad = Actions { axis: 1.0, dash: true, ..default() };
        let both = merge(keys, pad);
        assert!(both.punch && both.dash && both.axis == 1.0);
    }
}
