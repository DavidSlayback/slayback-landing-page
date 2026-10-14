//! The box: a floor about 3:2 (decision Q56) and four walls up to the rim. Every wall collides at full height; the
//! front one, nearest the camera, is drawn as glass (Q57). How it looks is up to `look`.

use avian3d::prelude::*;
use bevy::prelude::*;
use game_core::run::box_spec;

/// Floor size, metres: x across the screen, z into it. 3:2, about eight fighters across; a first guess to tune with
/// the pile (S2, G3).
pub const FLOOR: Vec2 = Vec2::new(15.0, 10.0);
/// Wall thickness, metres.
pub const WALL: f32 = 0.5;
/// Floor slab thickness, metres; its top is y = 0.
pub const SLAB: f32 = 1.0;
/// The 2.5D game's world units per metre (its physics length unit), for rules shared with it in `game_core::run`.
pub const UNITS_PER_METRE: f32 = 32.0;

/// The current box's shape.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Arena {
    /// Inner floor size, metres.
    pub floor: Vec2,
    /// Height of the rim above the floor, metres.
    pub rim: f32,
}

impl Default for Arena {
    /// Box 1: its rim from the run's rules (4.4 m, about 2.5 fighters tall).
    fn default() -> Self {
        Self { floor: FLOOR, rim: box_spec(1).rim / UNITS_PER_METRE }
    }
}

impl Arena {
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

pub fn plugin(app: &mut App) {
    app.init_resource::<Arena>().add_systems(Startup, build);
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

fn build(mut commands: Commands, arena: Res<Arena>) {
    for (name, slab, at) in slabs(&arena) {
        commands.spawn((
            Name::new(name),
            slab,
            Transform::from_translation(at),
            RigidBody::Static,
            Collider::cuboid(slab.size.x, slab.size.y, slab.size.z),
        ));
    }
}
