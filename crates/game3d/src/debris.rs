//! Debris: a defeated enemy bursts into the cubes it is built from, which tumble, rest, and freeze into solid,
//! climbable terrain. Rules and budget (`game_core::debris::solid`, spike S2): a piece freezes to `RigidBody::Static`
//! once it has rested ~1.5 s; at most 300 pieces move at once (past that the oldest moving ones freeze early) and 1,500
//! stay frozen (past that the oldest frozen ones go). Frozen heaps can't wake as a whole (research section 7).

use avian3d::prelude::*;
use bevy::prelude::*;
use game_core::debris::solid::{Settle, Solid, at_rest, figure, frozen_over_cap, moving_over_cap};
use game_core::grab::Held;

use crate::bomb::{BombHead, lit_bomb};
use crate::enemy::Brain;
use crate::fighter::{Combat, Defeated, Layer, Team};
use crate::grab::Grabbable;
use crate::player::Facing;

/// One piece of a broken figure: what it is, which figure part it was, and its settling clock.
#[derive(Component, Clone, Copy, Debug)]
pub struct Piece {
    /// `Time` seconds when it broke off: the oldest go first.
    pub born: f32,
    pub solid: Solid,
    /// Which part of the figure it was (an index into its kind's figure, `game_core::debris::solid::figure`).
    pub part: usize,
    pub settle: Settle,
}

/// How hard a burst throws pieces out from the body and up, m/s, and how much of the body's own velocity they keep.
const BURST_OUT: f32 = 2.4;
const BURST_UP: f32 = 2.8;
const CARRY: f32 = 0.3;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (burst, settle, cap_moving, cap_frozen).chain().after(Combat));
}

/// A loose piece of debris (as a burst spawns it; tests build piles with it too).
pub fn piece(solid: Solid, part: usize, at: Vec3, rotation: Quat, born: f32) -> impl Bundle {
    let collider = match solid {
        Solid::Cube([x, y, z]) => Collider::cuboid(x, y, z),
        Solid::Ball(r) => Collider::sphere(r),
    };
    (
        Piece { born, solid, part, settle: Settle::default() },
        // Frozen or loose, a piece can be picked up and thrown (frozen ones thaw).
        Grabbable(Held::Debris),
        Name::new("debris"),
        RigidBody::Dynamic,
        collider,
        CollisionLayers::new(Layer::Debris, [Layer::World, Layer::Fighter, Layer::Debris]),
        Friction::new(0.8),
        Restitution::new(0.05),
        // So the ball head stops rolling and settles like the cubes, and bursts stay near where the figure fell.
        LinearDamping(0.4),
        AngularDamping(2.0),
        Transform::from_translation(at).with_rotation(rotation),
    )
}

/// Defeated enemies burst into their own kind's pieces: each keeps some of the blow's momentum and flies out from the body's
/// centre with an uneven spin, so the figure explodes into tumbling blocks rather than falling apart.
fn burst(
    mut commands: Commands,
    time: Res<Time>,
    mut defeated: MessageReader<Defeated>,
    enemies: Query<(&Brain, &Transform, &Facing, &LinearVelocity, Has<BombHead>)>,
) {
    for d in defeated.read().filter(|d| d.team == Team::Enemy) {
        let Ok((brain, transform, facing, velocity, bomb_head)) = enemies.get(d.entity) else { continue };
        let centre = transform.translation;
        let turn = facing.rotation();
        commands.entity(d.entity).despawn();
        for (i, p) in figure(brain.0.style().kind).iter().enumerate() {
            let offset = turn * Vec3::from_array(p.at);
            let out = offset.with_y(offset.y * 0.5).normalize_or_zero();
            let fling = velocity.0 * CARRY + out * BURST_OUT + Vec3::Y * BURST_UP;
            // A bomb-head's head doesn't join the pile: it drops free as a lit bomb.
            if bomb_head && matches!(p.solid, Solid::Ball(_)) {
                commands.spawn(lit_bomb(centre + offset, fling));
                continue;
            }
            // Alternating, uneven spins about varied axes, so the blocks tumble rather than turn in step. The ball head
            // gets none: spun, it would roll on across the floor long after the blocks have settled.
            let axis = Vec3::new(1.0 + (i % 3) as f32, 0.5 * (i % 2) as f32, 1.0 - (i % 4) as f32 * 0.6).normalize();
            let spin = match p.solid {
                Solid::Ball(_) => Vec3::ZERO,
                Solid::Cube(_) => axis * (6.0 + 2.0 * (i % 3) as f32) * if i % 2 == 0 { 1.0 } else { -1.0 },
            };
            commands.spawn((
                piece(p.solid, i, centre + offset, turn, time.elapsed_secs()),
                LinearVelocity(fling),
                AngularVelocity(spin),
            ));
        }
    }
}

/// Moving pieces freeze once they have rested long enough (asleep, or barely moving).
#[allow(clippy::type_complexity)]
fn settle(
    mut commands: Commands,
    time: Res<Time>,
    mut pieces: Query<(Entity, &mut Piece, &RigidBody, &LinearVelocity, &AngularVelocity, Has<Sleeping>)>,
) {
    let dt = time.delta_secs();
    if dt == 0.0 {
        return; // hit-stop: nothing moved
    }
    for (e, mut piece, body, v, w, asleep) in &mut pieces {
        if *body == RigidBody::Dynamic && piece.settle.step(dt, asleep || at_rest(v.length(), w.length())) {
            commands.entity(e).insert(RigidBody::Static);
        }
    }
}

/// The oldest of `pieces` (entity, born) beyond `excess`.
fn oldest(mut pieces: Vec<(Entity, f32)>, excess: usize) -> impl Iterator<Item = Entity> {
    if excess > 0 {
        pieces.sort_by(|a, b| a.1.total_cmp(&b.1));
    }
    pieces.into_iter().take(excess).map(|(e, _)| e)
}

/// Past the moving cap, the oldest moving pieces freeze where they are.
fn cap_moving(mut commands: Commands, pieces: Query<(Entity, &Piece, &RigidBody)>) {
    let moving: Vec<_> =
        pieces.iter().filter(|(_, _, b)| **b == RigidBody::Dynamic).map(|(e, p, _)| (e, p.born)).collect();
    let excess = moving_over_cap(moving.len());
    for e in oldest(moving, excess) {
        commands.entity(e).insert(RigidBody::Static);
    }
}

/// Past the frozen cap, the oldest frozen pieces go.
fn cap_frozen(mut commands: Commands, pieces: Query<(Entity, &Piece, &RigidBody)>) {
    let frozen: Vec<_> =
        pieces.iter().filter(|(_, _, b)| **b == RigidBody::Static).map(|(e, p, _)| (e, p.born)).collect();
    let excess = frozen_over_cap(frozen.len());
    for e in oldest(frozen, excess) {
        commands.entity(e).despawn();
    }
}
