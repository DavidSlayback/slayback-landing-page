//! The box: a floor and side walls whose top (the rim) rises box by box, inside invisible world bounds that keep
//! anything thrown over the rim in play. How it looks is up to `look`.

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::ARENA;
use crate::run::BoxStarted;

/// Wall thickness, world units.
pub const WALL: f32 = 24.0;

/// A visible wall or floor slab of the given size (world units).
#[derive(Component)]
pub struct Wall(pub Vec2);

/// Part of the current box's geometry, rebuilt when the next box starts.
#[derive(Component)]
pub struct BoxPiece;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn_bounds).add_systems(Update, build_box);
}

/// The edges of the world: beyond the side walls, and a lid at the top of the view.
fn spawn_bounds(mut commands: Commands) {
    let half = ARENA / 2.0;
    for (pos, size) in [
        (Vec2::new(-half.x - WALL / 2.0, 0.0), Vec2::new(WALL, ARENA.y * 2.0)),
        (Vec2::new(half.x + WALL / 2.0, 0.0), Vec2::new(WALL, ARENA.y * 2.0)),
        (Vec2::new(0.0, half.y + WALL / 2.0), Vec2::new(ARENA.x + 2.0 * WALL, WALL)),
    ] {
        commands.spawn((Name::new("bound"), Transform::from_translation(pos.extend(0.0)), RigidBody::Static, Collider::rectangle(size.x, size.y)));
    }
}

fn build_box(mut commands: Commands, mut started: MessageReader<BoxStarted>, old: Query<Entity, With<BoxPiece>>) {
    let Some(BoxStarted(spec)) = started.read().last() else { return };
    for e in &old {
        commands.entity(e).despawn();
    }
    let half = ARENA / 2.0;
    let floor_top = -half.y + WALL;
    // Side walls stand on the floor and reach the rim.
    let wall_h = spec.rim;
    let pieces = [
        ("floor", Vec2::new(0.0, -half.y + WALL / 2.0), Vec2::new(ARENA.x, WALL)),
        ("left wall", Vec2::new(-half.x + WALL / 2.0, floor_top + wall_h / 2.0), Vec2::new(WALL, wall_h)),
        ("right wall", Vec2::new(half.x - WALL / 2.0, floor_top + wall_h / 2.0), Vec2::new(WALL, wall_h)),
    ];
    for (name, pos, size) in pieces {
        commands.spawn((
            Name::new(name),
            Wall(size),
            BoxPiece,
            Transform::from_translation(pos.extend(0.0)),
            RigidBody::Static,
            Collider::rectangle(size.x, size.y),
        ));
    }
}
