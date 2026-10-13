//! The box: its static walls. How they look is up to `look`.

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::ARENA;

/// Wall thickness, world units.
pub const WALL: f32 = 24.0;

/// A wall, floor or ceiling slab of the given size (world units).
#[derive(Component)]
pub struct Wall(pub Vec2);

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn_walls);
}

fn spawn_walls(mut commands: Commands) {
    let half = ARENA / 2.0;
    let pieces = [
        ("floor", Vec2::new(0.0, -half.y + WALL / 2.0), Vec2::new(ARENA.x, WALL)),
        ("ceiling", Vec2::new(0.0, half.y - WALL / 2.0), Vec2::new(ARENA.x, WALL)),
        ("left wall", Vec2::new(-half.x + WALL / 2.0, 0.0), Vec2::new(WALL, ARENA.y)),
        ("right wall", Vec2::new(half.x - WALL / 2.0, 0.0), Vec2::new(WALL, ARENA.y)),
    ];
    for (name, pos, size) in pieces {
        commands.spawn((
            Name::new(name),
            Wall(size),
            Transform::from_translation(pos.extend(0.0)),
            RigidBody::Static,
            Collider::rectangle(size.x, size.y),
        ));
    }
}
