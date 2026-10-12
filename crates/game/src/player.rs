//! The player: run, jump, punch and kick. Control is lost while stunned and the feet plant during a swing.

use avian2d::prelude::*;
use bevy::prelude::*;
use game_core::combat::Move;
use game_core::movement::{JUMP_SPEED, run_velocity};

use crate::fighter::{Attack, Defeated, Fighter, Hp, Team, fighter_bundle, strike_fx};

#[derive(Component)]
pub struct Player;

const HALF: Vec2 = Vec2::new(14.0, 28.0);
const COLOR: Color = Color::srgb(0.9, 1.0, 0.8);
const SPAWN: Vec3 = Vec3::new(0.0, -200.0, 1.0);
pub const MAX_HP: f32 = 100.0;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn).add_systems(Update, (control, respawn_on_defeat));
}

fn spawn(mut commands: Commands) {
    commands
        .spawn((Player, Name::new("player"), fighter_bundle(Team::Player, HALF, COLOR, MAX_HP), Transform::from_translation(SPAWN)))
        .with_children(|c| {
            c.spawn(strike_fx(Color::srgb(0.65, 1.0, 0.0)));
            c.spawn(visor(COLOR));
        });
}

/// A small bar on the leading side of the head so facing is readable at a glance.
pub fn visor(_color: Color) -> impl Bundle {
    (crate::Visor, Sprite::from_color(Color::BLACK, Vec2::new(8.0, 4.0)), Transform::from_xyz(6.0, 16.0, 0.6))
}

#[allow(clippy::type_complexity)]
fn control(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut players: Query<(&mut LinearVelocity, &ShapeHits, &mut Fighter, &mut Attack, &Hp), With<Player>>,
) {
    let Ok((mut velocity, ground, mut fighter, mut attack, hp)) = players.single_mut() else { return };
    let dt = time.delta_secs();
    if hp.0.stunned() {
        return; // knockback plays out
    }
    let held = |a: KeyCode, b: KeyCode| keys.pressed(a) || keys.pressed(b);
    let axis = (held(KeyCode::KeyD, KeyCode::ArrowRight) as i8 - held(KeyCode::KeyA, KeyCode::ArrowLeft) as i8) as f32;
    if attack.busy() {
        velocity.x = run_velocity(velocity.x, 0.0, dt); // plant the feet mid-swing
    } else {
        velocity.x = run_velocity(velocity.x, axis, dt);
        if axis != 0.0 {
            fighter.facing = axis.signum();
        }
        if keys.any_just_pressed([KeyCode::KeyJ, KeyCode::KeyZ]) || mouse.just_pressed(MouseButton::Left) {
            attack.start(Move::Punch);
        } else if keys.any_just_pressed([KeyCode::KeyK, KeyCode::KeyX]) || mouse.just_pressed(MouseButton::Right) {
            attack.start(Move::Kick);
        }
    }
    if keys.any_just_pressed([KeyCode::Space, KeyCode::KeyW, KeyCode::ArrowUp]) && !ground.is_empty() {
        velocity.y = JUMP_SPEED;
    }
}

/// Slice 2 has no game-over yet: a defeated player is patched up and dropped back in.
fn respawn_on_defeat(
    mut defeated: MessageReader<Defeated>,
    mut players: Query<(&mut Hp, &mut Transform, &mut LinearVelocity), With<Player>>,
) {
    for d in defeated.read().filter(|d| d.team == Team::Player) {
        if let Ok((mut hp, mut transform, mut velocity)) = players.get_mut(d.entity) {
            hp.0 = game_core::combat::Health::new(MAX_HP);
            transform.translation = SPAWN;
            velocity.0 = Vec2::ZERO;
        }
    }
}
