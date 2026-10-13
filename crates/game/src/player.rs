//! The player: a dynamic body with snappy horizontal control and a dash (no jump: climbing is walking up debris).

use avian2d::prelude::*;
use bevy::prelude::*;
use game_core::movement::{Input, Mover};

/// The player, with its engine-free movement state.
#[derive(Component, Default)]
pub struct Player(pub Mover);

/// Player collider size, world units.
pub const SIZE: Vec2 = Vec2::new(28.0, 56.0);

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn).add_systems(Update, movement);
}

/// Spawn point, world units.
pub const SPAWN: Vec2 = Vec2::new(0.0, -200.0);

fn spawn(mut commands: Commands) {
    commands.spawn((
        Player::default(),
        Name::new("player"),
        Sprite::from_color(Color::srgb(0.9, 1.0, 0.8), SIZE),
        Transform::from_translation(SPAWN.extend(1.0)),
        RigidBody::Dynamic,
        Collider::rectangle(SIZE.x, SIZE.y),
        LockedAxes::ROTATION_LOCKED,
        // Frictionless so walls don't hold the player up; horizontal control is set directly on velocity.
        Friction::ZERO.with_combine_rule(CoefficientCombine::Min),
        Restitution::ZERO,
        // Thin downward cast just below the feet: any hit means standing on something.
        ShapeCaster::new(Collider::rectangle(SIZE.x * 0.9, 2.0), Vec2::new(0.0, -SIZE.y / 2.0), 0.0, Dir2::NEG_Y)
            .with_max_distance(4.0),
    ));
}

/// Dash keys: Space (free since there is no jump) or either Shift.
pub const DASH_KEYS: [KeyCode; 3] = [KeyCode::Space, KeyCode::ShiftLeft, KeyCode::ShiftRight];

fn movement(time: Res<Time>, keys: Res<ButtonInput<KeyCode>>, mut players: Query<(&mut Player, &mut LinearVelocity)>) {
    let Ok((mut player, mut velocity)) = players.single_mut() else { return };
    let held = |a: KeyCode, b: KeyCode| keys.pressed(a) || keys.pressed(b);
    let axis = held(KeyCode::KeyD, KeyCode::ArrowRight) as i8 - held(KeyCode::KeyA, KeyCode::ArrowLeft) as i8;
    let input = Input { axis: axis as f32, dash: keys.any_just_pressed(DASH_KEYS) };
    velocity.x = player.0.step(velocity.x, input, time.delta_secs());
}
