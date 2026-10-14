//! Grab and spin-throw in 3D (rules: `game_core::grab`, its `flat` module for the floor). The grab verb (L, the middle
//! mouse button or B) picks up the nearest [`Grabbable`] thing in front: a live enemy or a debris part, and frozen
//! debris thaws. It rides overhead; an attack whirls it once and a half round the player (`player::control` aims it)
//! and lets go straight ahead, flying flat. In flight it hurts every enemy it passes through, once each, and a thrown
//! enemy is hurt by its first impact; then it lands as what it was (a stunned enemy, or loose debris back under the
//! debris rules). A lit bomb (`bomb`) grabs and throws the same way, but stops at the first enemy it hits and lies
//! there until its fuse runs out. Grab again while carrying to set the thing down in front.

use avian3d::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use game_core::aim::level;
use game_core::combat::{FINISHER_HITSTOP, HitOutcome, Move};
use game_core::debris::solid::{Settle, Solid};
use game_core::grab::flat::{knock, passes, pick, stops, swing, throw_velocity};
use game_core::grab::{FLIGHT, HITSTOP, Held, SPIN, impact};

use crate::arena::Arena;
use crate::bomb;
use crate::debris::Piece;
use crate::fighter::{Attack, Body, Combat, Defeated, FLASH, Flash, Hit, HitStopClock, Hp, Layer, Team};
use crate::input::Actions;
use crate::player::{self, Facing, HEIGHT, Holding, Player, RADIUS};
use crate::{on_floor, plane};

/// Something the player can pick up, and what it is when held. Enemies, debris parts and lit bombs carry it from their
/// bundles (a lit bomb is `Grabbable(Held::Bomb)`).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grabbable(pub Held);

/// Being held: out of the physics world (it collides with nothing and no query finds it) and out of the fight. Keeps
/// what it collided with, its size across the floor and its half height, for carrying it and letting go.
#[derive(Component, Clone, Copy, Debug)]
pub struct Carried {
    layers: CollisionLayers,
    radius: f32,
    half_height: f32,
}

/// In flight after a throw: a projectile flying flat (no gravity) along `heading` until `ttl` runs out.
#[derive(Component, Debug)]
pub struct Thrown {
    pub kind: Held,
    pub ttl: f32,
    heading: f32,
    radius: f32,
    /// Where it was last frame, so a fast throw hits what it passed between frames.
    from: Vec3,
    layers: CollisionLayers,
    victims: Vec<Entity>,
}

/// How far a thrown thing's centre may pass from an enemy's edge and still hit it, metres (a glancing hit counts).
const GRAZE: f32 = 0.05;
/// How hard a thrown debris part tumbles, rad/s.
const TUMBLE: f32 = 9.0;
/// How long a thrown thing's victims lose control, seconds (as in the 2.5D game).
const VICTIM_STUN: f32 = 0.35;

pub fn plugin(app: &mut App) {
    // Grabbing only while playing: L or a middle click on a screen must not pick anything up.
    let playing = in_state(crate::flow::Phase::Playing);
    app.add_systems(Update, (grab.run_if(playing), carry, fly).chain().after(player::control).before(Combat));
}

/// A thing's size for carrying: its radius across the floor and its half height, metres.
fn size(body: Option<&Body>, piece: Option<&Piece>) -> (f32, f32) {
    match (body, piece.map(|p| p.solid)) {
        (Some(b), _) => (b.radius, b.height / 2.0),
        (None, Some(Solid::Cube([x, y, z]))) => (x.max(z) / 2.0, y / 2.0),
        (None, Some(Solid::Ball(r))) => (r, r),
        // A lit bomb.
        (None, None) => (bomb::RADIUS, bomb::RADIUS),
    }
}

/// Where the held thing sits for a player standing at `me` facing `yaw`: overhead, or `spin` (0..=1) through the throw.
/// Never past the walls, so a throw or a set-down beside one doesn't start inside it.
fn hold_at(arena: &Arena, me: Vec3, yaw: f32, carried: &Carried, spin: Option<f32>) -> Vec3 {
    let overhead = HEIGHT / 2.0 + carried.half_height + 0.05;
    let (across, up) = swing(yaw, overhead, RADIUS + carried.radius + 0.1, spin);
    let at = me + on_floor(across) + Vec3::Y * up;
    let inside = arena.floor / 2.0 - Vec2::splat(carried.radius);
    at.with_x(at.x.clamp(-inside.x, inside.x)).with_z(at.z.clamp(-inside.y, inside.y))
}

/// Everything the player could pick up.
type Candidates = (
    Entity,
    &'static Grabbable,
    &'static Transform,
    &'static CollisionLayers,
    Option<&'static Body>,
    Option<&'static mut Piece>,
    Option<&'static Hp>,
    Option<&'static mut Attack>,
);

/// Grab pressed: pick up the nearest grabbable thing in front (thawing frozen debris), or, carrying, set it down.
#[allow(clippy::type_complexity)]
fn grab(
    mut commands: Commands,
    actions: Res<Actions>,
    time: Res<Time>,
    arena: Res<Arena>,
    players: Query<(Entity, &Transform, &Facing, &Hp, Option<&Holding>), With<Player>>,
    mut things: Query<Candidates, (Without<Player>, Without<Carried>, Without<Thrown>)>,
    mut held: Query<(&mut Transform, &mut Position, &Carried), Without<Player>>,
) {
    let Ok((me, at, facing, hp, holding)) = players.single() else { return };
    if !actions.verbs.grab || hp.0.stunned() {
        return;
    }
    let (here, yaw) = (at.translation, facing.0.yaw);
    if let Some(h) = holding {
        // Set it down in front, at the player's middle: it drops from there.
        if !h.spinning()
            && let Ok((mut t, mut p, carried)) = held.get_mut(h.target)
        {
            t.translation = hold_at(&arena, here, yaw, carried, Some(1.0));
            p.0 = t.translation;
            let layers = carried.layers;
            commands.entity(h.target).remove::<Carried>().insert((RigidBody::Dynamic, layers, LinearVelocity::ZERO));
            commands.entity(me).remove::<Holding>();
        }
        return;
    }
    // In front, within reach, and between the feet and the top of the head; nearest first.
    let feet = here.y - HEIGHT / 2.0;
    let reachable: Vec<(Entity, [f32; 2], f32)> = things
        .iter()
        .filter(|(.., hp, _)| hp.is_none_or(|h| !h.0.defeated()))
        .filter(|(_, _, t, ..)| (feet - 0.15..=feet + HEIGHT + 0.15).contains(&t.translation.y))
        .map(|(e, _, t, _, body, piece, ..)| (e, plane(t.translation), size(body, piece).0))
        .collect();
    let Some(i) = pick(plane(here), yaw, RADIUS, reachable.iter().map(|(_, p, r)| (*p, *r))) else { return };
    let target = reachable[i].0;
    let Ok((_, &Grabbable(kind), _, &layers, body, piece, _, attack)) = things.get_mut(target) else { return };
    let (radius, half_height) = size(body, piece.as_deref());
    if let Some(mut piece) = piece {
        // Thawed: it starts again as a fresh loose piece, so it freezes only after it lands and rests.
        piece.born = time.elapsed_secs();
        piece.settle = Settle::default();
    }
    if let Some(mut attack) = attack {
        *attack = Attack::default(); // a held enemy's swing stops
    }
    commands.entity(target).insert((
        Carried { layers, radius, half_height },
        RigidBody::Kinematic,
        CollisionLayers::NONE,
        LinearVelocity::ZERO,
        AngularVelocity::ZERO,
    ));
    commands.entity(me).insert(Holding { target, kind, spin: None });
}

/// Keeps the held thing overhead or whirling round the player, and at the end of the spin throws it straight ahead.
fn carry(
    mut commands: Commands,
    time: Res<Time>,
    arena: Res<Arena>,
    mut players: Query<(Entity, &Transform, &Facing, &mut Holding), With<Player>>,
    mut held: Query<(&mut Transform, &mut Position, &Carried, Option<&mut Hp>), Without<Player>>,
) {
    let Ok((me, at, facing, mut h)) = players.single_mut() else { return };
    let Ok((mut t, mut p, carried, hp)) = held.get_mut(h.target) else {
        commands.entity(me).remove::<Holding>(); // gone (the box emptied)
        return;
    };
    if let Some(s) = h.spin.as_mut() {
        *s += time.delta_secs();
    }
    let progress = h.spin.map(|s| s / SPIN);
    let yaw = facing.0.yaw;
    t.translation = hold_at(&arena, at.translation, yaw, carried, progress);
    p.0 = t.translation;
    if progress.is_none_or(|p| p < 1.0) {
        return;
    }
    // Let go: a projectile flying flat along the facing that passes through fighters (it hurts them in `fly`).
    let layers = carried.layers;
    let mut flying = layers;
    flying.filters.remove(Layer::Fighter);
    let mut thrown = commands.entity(h.target);
    thrown.remove::<Carried>().insert((
        RigidBody::Dynamic,
        flying,
        GravityScale(0.0),
        LinearVelocity(on_floor(throw_velocity(yaw))),
        Thrown {
            kind: h.kind,
            ttl: FLIGHT,
            heading: yaw,
            radius: carried.radius,
            from: t.translation,
            layers,
            victims: Vec::new(),
        },
    ));
    if h.kind == Held::Debris {
        thrown.insert(AngularVelocity(on_floor([-yaw.sin(), yaw.cos()]) * TUMBLE));
    }
    // A thrown enemy is helpless until it lands, so its brain doesn't stop the throw.
    if let Some(mut hp) = hp {
        hp.0.stun = hp.0.stun.max(FLIGHT);
    }
    commands.entity(me).remove::<Holding>();
}

/// What landing a hit sets off: hit-stop, impact effects (`Hit`) and, when it was the last of its health, a defeat.
#[derive(SystemParam)]
struct Impacts<'w> {
    stop: ResMut<'w, HitStopClock>,
    hits: MessageWriter<'w, Hit>,
    defeated: MessageWriter<'w, Defeated>,
}

/// A thrown thing hurts every enemy it passes through (once each), and a thrown enemy is hurt by its first impact.
/// When its flight ends it lands as what it was: gravity back, and fighters in its way again.
#[allow(clippy::type_complexity)]
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    mut thrown: Query<(Entity, &mut Thrown, &Transform)>,
    targets: Query<(Entity, &Team, &Body, &Transform), (Without<Thrown>, Without<Carried>)>,
    mut hurt: Query<(&mut Hp, &mut LinearVelocity, &mut Flash), Without<Thrown>>,
    mut self_hurt: Query<&mut Hp, With<Thrown>>,
    mut out: Impacts,
) {
    for (e, mut t, tf) in &mut thrown {
        let now = tf.translation;
        let spec = impact(t.kind);
        for (target, team, body, their) in &targets {
            let them = their.translation;
            if *team != Team::Enemy
                || t.victims.contains(&target)
                || !level(now.y, them.y)
                || !passes(plane(t.from), plane(now), plane(them), t.radius + body.radius + GRAZE)
            {
                continue;
            }
            let Ok((mut hp, mut v, mut flash)) = hurt.get_mut(target) else { continue };
            if hp.0.defeated() {
                continue;
            }
            t.victims.push(target);
            let (across, up) = knock(t.kind, t.heading);
            v.0 = on_floor(across).with_y(up);
            flash.0 = FLASH;
            hp.0.stun = hp.0.stun.max(VICTIM_STUN);
            let finisher = hp.0.hurt(spec.damage) == HitOutcome::Defeated;
            out.stop.0.trigger(if finisher { HITSTOP.max(FINISHER_HITSTOP) } else { HITSTOP });
            let dir = on_floor([t.heading.cos(), t.heading.sin()]);
            out.hits.write(Hit {
                at: (them - dir * body.radius).with_y(now.y),
                mv: Move::Kick,
                dir,
                victim: target,
                finisher,
            });
            if finisher {
                out.defeated.write(Defeated { entity: target, team: Team::Enemy, at: them });
            }
            // The thrown body takes its own knock on its first impact.
            if t.victims.len() == 1
                && spec.self_damage > 0.0
                && let Ok(mut own) = self_hurt.get_mut(e)
                && own.0.hurt(spec.self_damage) == HitOutcome::Defeated
            {
                out.defeated.write(Defeated { entity: e, team: Team::Enemy, at: now });
            }
            // A lit bomb stops dead at the first enemy it hits and drops there, in the group, until it blows.
            if stops(t.kind) {
                t.ttl = 0.0;
                commands.entity(e).insert(LinearVelocity::ZERO);
                break;
            }
        }
        t.from = now;
        t.ttl -= time.delta_secs();
        if t.ttl <= 0.0 {
            let layers = t.layers;
            commands.entity(e).remove::<(Thrown, GravityScale)>().insert(layers);
        }
    }
}
