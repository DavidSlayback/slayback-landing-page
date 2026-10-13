//! Debris: a defeated enemy falls apart into its own parts (the figure's primitives), which settle, freeze and stay
//! as solid, climbable terrain. Rules and budget: `game_core::debris` (from the B1 spike).

use avian2d::prelude::*;
use bevy::prelude::*;
use game_core::debris::{Shape, humanoid_parts, over_cap, should_freeze};

use crate::bomb::{BombHead, lit_bomb};
use crate::fighter::{Defeated, Fighter, Layer, Team};

/// One part of a broken enemy.
#[derive(Component, Clone, Copy, Debug)]
pub struct DebrisPart {
    /// `Time` seconds when it broke off.
    pub born: f32,
    pub shape: Shape,
}

/// An enemy broke apart here (for the look's shatter burst).
#[derive(Message, Clone, Copy, Debug)]
pub struct Shattered {
    pub at: Vec2,
}

pub fn plugin(app: &mut App) {
    app.add_message::<Shattered>().add_systems(Update, (break_apart, freeze, enforce_cap).chain());
}

/// A loose debris part, as spawned by a defeat (also used by tests to build piles).
pub fn part(shape: Shape, at: Vec2, born: f32) -> impl Bundle {
    let collider = match shape {
        Shape::Box { w, h } => Collider::rectangle(w, h),
        Shape::Ball { r } => Collider::circle(r),
    };
    (
        DebrisPart { born, shape },
        Name::new("debris"),
        RigidBody::Dynamic,
        collider,
        CollisionLayers::new(Layer::Debris, [Layer::World, Layer::Fighter, Layer::Debris]),
        Friction::new(0.8),
        Restitution::new(0.05),
        Transform::from_translation(at.extend(0.0)),
    )
}

fn break_apart(
    mut commands: Commands,
    time: Res<Time>,
    mut defeated: MessageReader<Defeated>,
    enemies: Query<(&Transform, &Fighter, &LinearVelocity, Has<BombHead>)>,
    mut shattered: MessageWriter<Shattered>,
) {
    for d in defeated.read().filter(|d| d.team == Team::Enemy) {
        let Ok((transform, fighter, velocity, bomb_head)) = enemies.get(d.entity) else { continue };
        let centre = transform.translation.truncate();
        commands.entity(d.entity).despawn();
        shattered.write(Shattered { at: centre });
        for (i, p) in humanoid_parts((fighter.half.x, fighter.half.y)).into_iter().enumerate() {
            let offset = Vec2::new(p.offset.0 * fighter.facing, p.offset.1);
            // The parts keep the blow's momentum and burst a little outward from the body's centre.
            let burst = offset.normalize_or_zero() * 140.0 + Vec2::Y * 120.0;
            let fling = velocity.0 * 0.6 + burst;
            // A bomb-head's head doesn't join the pile: it drops free as a lit bomb.
            if bomb_head && matches!(p.shape, Shape::Ball { .. }) {
                commands.spawn(lit_bomb(centre + offset, fling));
                continue;
            }
            let spin = if i % 2 == 0 { 6.0 } else { -6.0 };
            commands.spawn((part(p.shape, centre + offset, time.elapsed_secs()), LinearVelocity(fling), AngularVelocity(spin)));
        }
    }
}

fn freeze(mut commands: Commands, time: Res<Time>, parts: Query<(Entity, &DebrisPart, &RigidBody, Has<Sleeping>)>) {
    for (e, part, body, asleep) in &parts {
        if *body == RigidBody::Dynamic && should_freeze(time.elapsed_secs() - part.born, asleep) {
            commands.entity(e).insert(RigidBody::Static);
        }
    }
}

fn enforce_cap(mut commands: Commands, parts: Query<(Entity, &DebrisPart)>) {
    let excess = over_cap(parts.iter().len());
    if excess == 0 {
        return;
    }
    let mut oldest: Vec<_> = parts.iter().map(|(e, p)| (p.born, e)).collect();
    oldest.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, e) in oldest.into_iter().take(excess) {
        commands.entity(e).despawn();
    }
}
