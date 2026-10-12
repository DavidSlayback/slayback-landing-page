//! The box: camera fitted to the arena, and its static walls.

use avian2d::prelude::*;
use bevy::camera::ScalingMode;
use bevy::prelude::*;

use crate::ARENA;

/// Wall thickness, world units.
pub const WALL: f32 = 24.0;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, (spawn_camera, spawn_walls));
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin { min_width: ARENA.x, min_height: ARENA.y },
            ..OrthographicProjection::default_2d()
        }),
    ));
}

fn spawn_walls(mut commands: Commands) {
    let half = ARENA / 2.0;
    let color = Color::srgb(0.62, 1.0, 0.0);
    let pieces = [
        ("floor", Vec2::new(0.0, -half.y + WALL / 2.0), Vec2::new(ARENA.x, WALL)),
        ("ceiling", Vec2::new(0.0, half.y - WALL / 2.0), Vec2::new(ARENA.x, WALL)),
        ("left wall", Vec2::new(-half.x + WALL / 2.0, 0.0), Vec2::new(WALL, ARENA.y)),
        ("right wall", Vec2::new(half.x - WALL / 2.0, 0.0), Vec2::new(WALL, ARENA.y)),
    ];
    for (name, pos, size) in pieces {
        commands.spawn((
            Name::new(name),
            Sprite::from_color(color, size),
            Transform::from_translation(pos.extend(0.0)),
            RigidBody::Static,
            Collider::rectangle(size.x, size.y),
        ));
    }
}
