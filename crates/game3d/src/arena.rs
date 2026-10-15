//! The box: a floor about 3:2 (decision Q56) and four walls up to the rim, rebuilt at each box's rim as the run goes
//! (`run::BoxStarted`). Every wall collides at full height; the front one, nearest the camera, is drawn as glass
//! (Q57). An invisible fence stands on every wall, so neither the player nor a flung piece leaves over the rim: the
//! way out is the pile (Q51). How it looks is up to `look`.

use avian3d::prelude::*;
use bevy::prelude::*;
use game_core::run::{BoxSpec, box_spec};

use crate::run::BoxStarted;

/// Floor size, metres: x across the screen, z into it. 3:2, about eight fighters across; a first guess to tune with
/// the pile (S2, G3).
pub const FLOOR: Vec2 = Vec2::new(15.0, 10.0);
/// Wall thickness, metres.
pub const WALL: f32 = 0.5;
/// Floor slab thickness, metres; its top is y = 0.
pub const SLAB: f32 = 1.0;
/// Height of the invisible fence above the rim, metres: more than a fighter, and more than a burst flings a piece.
pub const FENCE: f32 = 3.0;

/// The current box's shape.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Arena {
    /// Inner floor size, metres.
    pub floor: Vec2,
    /// Height of the rim above the floor, metres.
    pub rim: f32,
}

impl Default for Arena {
    /// Box 1.
    fn default() -> Self {
        Self::for_box(&box_spec(1))
    }
}

impl Arena {
    /// The box for `spec`: the same floor every time, and its rim from the run's rules (`game_core::run::BoxSpec::rim`).
    pub fn for_box(spec: &BoxSpec) -> Self {
        Self { floor: FLOOR, rim: spec.rim }
    }

    /// Half the outer footprint (floor plus walls), metres.
    pub fn outer_half(&self) -> Vec2 {
        self.floor / 2.0 + Vec2::splat(WALL)
    }
}

/// What a slab of the box is, for the look.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Floor,
    Wall,
    /// The front wall: solid, but see-through.
    Glass,
}

/// A visible slab of the box: its kind and size (metres).
#[derive(Component, Clone, Copy, Debug)]
pub struct Slab {
    pub kind: Kind,
    pub size: Vec3,
}

/// Part of the current box's geometry, rebuilt when the next box starts.
#[derive(Component)]
pub struct BoxPiece;

pub fn plugin(app: &mut App) {
    app.init_resource::<Arena>().add_systems(Update, build);
}

/// The slabs of `arena`: the floor, the back, left and right walls, and the glass front (+z), with their centres.
pub fn slabs(arena: &Arena) -> [(&'static str, Slab, Vec3); 5] {
    let half = arena.floor / 2.0;
    let outer = arena.outer_half() * 2.0;
    let h = arena.rim;
    let slab = |kind, x: f32, y: f32, z: f32| Slab { kind, size: Vec3::new(x, y, z) };
    [
        ("floor", slab(Kind::Floor, outer.x, SLAB, outer.y), Vec3::new(0.0, -SLAB / 2.0, 0.0)),
        ("back wall", slab(Kind::Wall, outer.x, h, WALL), Vec3::new(0.0, h / 2.0, -half.y - WALL / 2.0)),
        ("left wall", slab(Kind::Wall, WALL, h, arena.floor.y), Vec3::new(-half.x - WALL / 2.0, h / 2.0, 0.0)),
        ("right wall", slab(Kind::Wall, WALL, h, arena.floor.y), Vec3::new(half.x + WALL / 2.0, h / 2.0, 0.0)),
        ("front wall", slab(Kind::Glass, outer.x, h, WALL), Vec3::new(0.0, h / 2.0, half.y + WALL / 2.0)),
    ]
}

/// Builds the box for each box that starts, replacing the last one's.
fn build(
    mut commands: Commands,
    mut started: MessageReader<BoxStarted>,
    mut arena: ResMut<Arena>,
    old: Query<Entity, With<BoxPiece>>,
) {
    let Some(BoxStarted(spec)) = started.read().last() else { return };
    for e in &old {
        commands.entity(e).despawn();
    }
    *arena = Arena::for_box(spec);
    for (name, slab, at) in slabs(&arena) {
        let collider = Collider::cuboid(slab.size.x, slab.size.y, slab.size.z);
        commands.spawn((Name::new(name), slab, BoxPiece, Transform::from_translation(at), RigidBody::Static, collider));
        if slab.kind != Kind::Floor {
            let fence = slab.size.with_y(FENCE);
            commands.spawn((
                Name::new("fence"),
                BoxPiece,
                Transform::from_translation(at.with_y(arena.rim + FENCE / 2.0)),
                RigidBody::Static,
                Collider::cuboid(fence.x, fence.y, fence.z),
            ));
        }
    }
}
