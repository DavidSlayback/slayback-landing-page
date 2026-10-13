//! The player: a fighter with snappy horizontal control, a dash, and punch / kick / drop-kick (no jump: climbing is
//! walking up debris).

use avian2d::prelude::*;
use bevy::prelude::*;
use game_core::combat::Move;
use game_core::movement::{Input, Mover, run_velocity};

use crate::fighter::{Attack, Fighter, Hp, Team, body};

/// The player, with its engine-free movement state.
#[derive(Component, Default)]
pub struct Player(pub Mover);

/// Player collider size, world units.
pub const SIZE: Vec2 = Vec2::new(28.0, 56.0);
/// Spawn point, world units.
pub const SPAWN: Vec2 = Vec2::new(0.0, -200.0);
pub const MAX_HP: f32 = 100.0;

/// Dash keys: Space (free since there is no jump) or either Shift.
pub const DASH_KEYS: [KeyCode; 3] = [KeyCode::Space, KeyCode::ShiftLeft, KeyCode::ShiftRight];
/// Punch: J or the left mouse button. Kick: K or the right mouse button.
pub const PUNCH_KEY: KeyCode = KeyCode::KeyJ;
pub const KICK_KEY: KeyCode = KeyCode::KeyK;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn).add_systems(Update, control);
}

fn spawn(mut commands: Commands) {
    commands.spawn((
        Player::default(),
        Name::new("player"),
        body(Team::Player, SIZE / 2.0, MAX_HP),
        Transform::from_translation(SPAWN.extend(0.0)),
    ));
}

fn control(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    mut players: Query<(&mut Player, &mut Fighter, &mut Attack, &Hp, &mut LinearVelocity)>,
) {
    let Ok((mut player, mut fighter, mut attack, hp, mut velocity)) = players.single_mut() else { return };
    let dt = time.delta_secs();
    if hp.0.stunned() {
        return; // knockback plays out
    }
    let clicked = |b: MouseButton| mouse.as_ref().is_some_and(|m| m.just_pressed(b));
    let button = if keys.just_pressed(PUNCH_KEY) || clicked(MouseButton::Left) {
        Some(Move::Punch)
    } else if keys.just_pressed(KICK_KEY) || clicked(MouseButton::Right) {
        Some(Move::Kick)
    } else {
        None
    };
    if let Some(button) = button {
        attack.start(Move::for_button(button, player.0.dashing()));
    }
    // A punch or kick plants the feet; a drop-kick rides the dash.
    let planted = attack.swing.is_some_and(|s| s.mv != Move::DropKick);
    if planted {
        velocity.x = run_velocity(velocity.x, 0.0, dt);
        return;
    }
    let held = |a: KeyCode, b: KeyCode| keys.pressed(a) || keys.pressed(b);
    let axis = held(KeyCode::KeyD, KeyCode::ArrowRight) as i8 - held(KeyCode::KeyA, KeyCode::ArrowLeft) as i8;
    let input = Input { axis: axis as f32, dash: keys.any_just_pressed(DASH_KEYS) };
    velocity.x = player.0.step(velocity.x, input, dt);
    fighter.facing = player.0.facing;
}
