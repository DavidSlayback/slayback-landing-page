//! The player: a dynamic body with snappy horizontal control and grounded jumps.

use avian2d::prelude::*;
use bevy::prelude::*;
use game_core::movement::{JUMP_SPEED, run_velocity};

#[derive(Component)]
pub struct Player;

/// Player collider size, world units.
pub const SIZE: Vec2 = Vec2::new(28.0, 56.0);

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn).add_systems(Update, movement);
}

/// Spawn point, world units.
pub const SPAWN: Vec2 = Vec2::new(0.0, -200.0);

fn spawn(mut commands: Commands) {
    commands.spawn((
        Player,
        Name::new("player"),
        Sprite::from_color(Color::srgb(0.9, 1.0, 0.8), SIZE),
        Transform::from_translation(SPAWN.extend(1.0)),
        RigidBody::Dynamic,
        Collider::rectangle(SIZE.x, SIZE.y),
        LockedAxes::ROTATION_LOCKED,
        // Frictionless so walls don't hold the player up; horizontal control is set directly on velocity.
        Friction::ZERO.with_combine_rule(CoefficientCombine::Min),
        Restitution::ZERO,
        // Thin downward cast just below the feet: any hit means grounded.
        ShapeCaster::new(Collider::rectangle(SIZE.x * 0.9, 2.0), Vec2::new(0.0, -SIZE.y / 2.0), 0.0, Dir2::NEG_Y)
            .with_max_distance(4.0),
    ));
}

fn movement(time: Res<Time>, keys: Res<ButtonInput<KeyCode>>, mut players: Query<(&mut LinearVelocity, &ShapeHits), With<Player>>) {
    let Ok((mut velocity, ground)) = players.single_mut() else { return };
    let held = |a: KeyCode, b: KeyCode| keys.pressed(a) || keys.pressed(b);
    let axis = held(KeyCode::KeyD, KeyCode::ArrowRight) as i8 - held(KeyCode::KeyA, KeyCode::ArrowLeft) as i8;
    velocity.x = run_velocity(velocity.x, axis as f32, time.delta_secs());
    let jump = keys.any_just_pressed([KeyCode::Space, KeyCode::KeyW, KeyCode::ArrowUp]);
    if jump && !ground.is_empty() {
        velocity.y = JUMP_SPEED;
    }
}
