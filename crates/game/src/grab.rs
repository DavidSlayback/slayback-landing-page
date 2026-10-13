//! Grab and spin-throw (rules: `game_core::grab`). L or the middle mouse button grabs the nearest live enemy or debris
//! part in front (frozen debris thaws); an attack while holding spins it once round and throws it. In flight it hurts
//! every enemy it passes through; then it lands as what it was. L again sets the held thing down.

use avian2d::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use game_core::combat::{Aabb, HitOutcome, Move};
use game_core::grab::{FLIGHT, HITSTOP, Held, REACH, SPIN, hold_offset, impact, throw_velocity};

use crate::debris::DebrisPart;
use crate::enemy::Brain;
use crate::fighter::{Defeated, FLASH, Fighter, Flash, Hit, HitStopClock, Hp, Team};
use crate::player::{KICK_KEY, PUNCH_KEY, Player};

pub const GRAB_KEY: KeyCode = KeyCode::KeyL;

/// The player is holding something; `spin` counts up from 0 once a throw starts.
#[derive(Component, Debug)]
pub struct Holding {
    pub target: Entity,
    pub kind: Held,
    pub spin: Option<f32>,
}

/// Being held: no physics contact, no thinking.
#[derive(Component)]
pub struct Carried;

/// In flight after a throw: a projectile until `ttl` runs out.
#[derive(Component, Debug)]
pub struct Thrown {
    pub kind: Held,
    pub ttl: f32,
    victims: Vec<Entity>,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (grab, start_throw, carry, fly).chain().after(crate::player::control));
}

#[allow(clippy::type_complexity)]
fn grab(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    time: Res<Time>,
    player: Query<(Entity, &Transform, &Fighter, &Hp, Option<&Holding>), With<Player>>,
    enemies: Query<(Entity, &Transform, &Fighter), (With<Brain>, Without<Carried>, Without<Player>)>,
    mut parts: Query<(Entity, &Transform, &ColliderAabb, &mut DebrisPart), (Without<Carried>, Without<Player>)>,
) {
    let pressed = keys.just_pressed(GRAB_KEY) || mouse.as_ref().is_some_and(|m| m.just_pressed(MouseButton::Middle));
    let Ok((me, at, fighter, hp, holding)) = player.single() else { return };
    if !pressed || hp.0.stunned() {
        return;
    }
    if let Some(h) = holding {
        // Set it down in front: it just drops.
        if h.spin.is_none() {
            release(&mut commands, h.target, Vec2::ZERO, None);
            commands.entity(me).remove::<Holding>();
        }
        return;
    }
    // In front of the player, within reach, roughly at body height; nearest wins.
    let p = at.translation.truncate();
    let front = |q: Vec2, half_w: f32| {
        let ahead = (q.x - p.x) * fighter.facing;
        ahead > 0.0 && ahead <= fighter.half.x + REACH + half_w && (q.y - p.y).abs() <= fighter.half.y + 12.0
    };
    let mut best: Option<(f32, Entity, Held)> = None;
    let mut consider = |d: f32, e: Entity, k: Held| {
        if best.is_none_or(|(bd, _, _)| d < bd) {
            best = Some((d, e, k));
        }
    };
    for (e, t, f) in &enemies {
        let q = t.translation.truncate();
        if front(q, f.half.x) {
            consider(q.distance(p), e, Held::Enemy);
        }
    }
    for (e, t, aabb, _) in &parts {
        let q = t.translation.truncate();
        if front(q, (aabb.max.x - aabb.min.x) / 2.0) {
            consider(q.distance(p), e, Held::Debris);
        }
    }
    let Some((_, target, kind)) = best else { return };
    if let Ok((_, _, _, mut part)) = parts.get_mut(target) {
        part.born = time.elapsed_secs(); // thawed: it freezes again only after it lands
    }
    commands.entity(target).insert((Carried, RigidBody::Kinematic, ColliderDisabled, LinearVelocity::ZERO, AngularVelocity::ZERO));
    commands.entity(me).insert(Holding { target, kind, spin: None });
}

fn start_throw(keys: Res<ButtonInput<KeyCode>>, mouse: Option<Res<ButtonInput<MouseButton>>>, mut holding: Query<&mut Holding>) {
    let clicked = mouse.as_ref().is_some_and(|m| m.any_just_pressed([MouseButton::Left, MouseButton::Right]));
    if !(keys.any_just_pressed([PUNCH_KEY, KICK_KEY]) || clicked) {
        return;
    }
    for mut h in &mut holding {
        if h.spin.is_none() {
            h.spin = Some(0.0);
        }
    }
}

/// Keeps the held thing overhead (or swinging round during the spin) and lets go at the end of the spin.
fn carry(
    mut commands: Commands,
    time: Res<Time>,
    mut player: Query<(Entity, &Transform, &Fighter, &mut Holding), Without<Carried>>,
    mut held: Query<&mut Transform, With<Carried>>,
) {
    for (me, at, fighter, mut h) in &mut player {
        let Ok(mut t) = held.get_mut(h.target) else {
            commands.entity(me).remove::<Holding>(); // gone (despawned with the box)
            continue;
        };
        if let Some(s) = h.spin.as_mut() {
            *s += time.delta_secs();
        }
        let progress = h.spin.map(|s| s / SPIN);
        let (dx, dy, depth) = hold_offset(fighter.facing, fighter.half.y, progress);
        t.translation = (at.translation.truncate() + Vec2::new(dx, dy)).extend(depth);
        if progress.is_some_and(|p| p >= 1.0) {
            let (vx, vy) = throw_velocity(fighter.facing);
            release(&mut commands, h.target, Vec2::new(vx, vy), Some(h.kind));
            commands.entity(me).remove::<Holding>();
        }
    }
}

/// Lets go: physics again, with `velocity`; a throw (`kind`) makes it a projectile.
fn release(commands: &mut Commands, target: Entity, velocity: Vec2, kind: Option<Held>) {
    let mut e = commands.entity(target);
    e.remove::<(Carried, ColliderDisabled)>().insert((RigidBody::Dynamic, LinearVelocity(velocity)));
    if let Some(kind) = kind {
        e.insert(Thrown { kind, ttl: FLIGHT, victims: Vec::new() });
        // A thrown enemy is helpless until it lands, so its brain doesn't stop the throw.
        e.queue(|mut entity: EntityWorldMut| {
            if let Some(mut hp) = entity.get_mut::<Hp>() {
                hp.0.stun = hp.0.stun.max(FLIGHT);
            }
        });
    }
}

/// What landing a hit sets off: hit-stop, sparks (`Hit`) and, when it was the last of its health, a defeat.
#[derive(SystemParam)]
struct Impacts<'w> {
    stop: ResMut<'w, HitStopClock>,
    hits: MessageWriter<'w, Hit>,
    defeated: MessageWriter<'w, Defeated>,
}

/// A thrown thing hurts every enemy it passes through (once each), and a thrown enemy is hurt by its first impact.
#[allow(clippy::type_complexity)]
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    mut thrown: Query<(Entity, &mut Thrown, &ColliderAabb, &LinearVelocity)>,
    targets: Query<(Entity, &Team, &Fighter, &Transform), Without<Thrown>>,
    mut hurt: Query<(&mut Hp, &mut LinearVelocity, &mut Flash), Without<Thrown>>,
    mut self_hurt: Query<&mut Hp, With<Thrown>>,
    mut out: Impacts,
) {
    for (e, mut t, aabb, velocity) in &mut thrown {
        t.ttl -= time.delta_secs();
        if t.ttl <= 0.0 {
            commands.entity(e).remove::<Thrown>();
            continue;
        }
        let body = Aabb { min: (aabb.min.x, aabb.min.y), max: (aabb.max.x, aabb.max.y) };
        let facing = velocity.x.signum();
        let spec = impact(t.kind);
        for (target, team, fighter, tf) in &targets {
            let other = Aabb::around((tf.translation.x, tf.translation.y), (fighter.half.x, fighter.half.y));
            if *team != Team::Enemy || t.victims.contains(&target) || !body.overlaps(&other) {
                continue;
            }
            let Ok((mut hp, mut v, mut flash)) = hurt.get_mut(target) else { continue };
            if hp.0.defeated() {
                continue;
            }
            t.victims.push(target);
            v.0 = Vec2::new(spec.knockback.0 * facing, spec.knockback.1);
            flash.0 = FLASH;
            hp.0.stun = hp.0.stun.max(0.35);
            out.stop.0.trigger(HITSTOP);
            out.hits.write(Hit { at: (tf.translation.truncate() + Vec2::new(aabb.center().x, aabb.center().y)) / 2.0, mv: Move::Kick, facing });
            if hp.0.hurt(spec.damage) == HitOutcome::Defeated {
                out.defeated.write(Defeated { entity: target, team: Team::Enemy });
            }
            // The thrown body takes its own knock on the first impact.
            if t.victims.len() == 1
                && spec.self_damage > 0.0
                && let Ok(mut own) = self_hurt.get_mut(e)
                && own.0.hurt(spec.self_damage) == HitOutcome::Defeated
            {
                out.defeated.write(Defeated { entity: e, team: Team::Enemy });
            }
        }
    }
}
