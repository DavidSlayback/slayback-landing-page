//! Carrying, from the grab to the throw's landing (rules: `game_core::grab`). This module owns the whole of it: the
//! player's [`Carry`] (holding, then whirling), the held thing's [`Carried`] and the thrown thing's [`Thrown`]. Nothing
//! else writes them; the player's control and the figure only read the carry's phase.
//!
//! The grab verb (L, the middle mouse button or B) picks up the nearest [`Grabbable`] thing in front: a live enemy or
//! a debris part, and frozen debris thaws. It is held out in front in the fists, an enemy lifted off its feet and
//! tipped back, swaying (Q77). An attack starts the throw: aimed at once toward the biggest enemy group ahead (Q62),
//! the player turned to face it, the thing whirls once round and lets go straight along that heading, flying flat. In
//! flight it strikes every enemy it passes through, once each (`fighter::land`), and a thrown enemy is hurt by its
//! first impact; then it lands as what it was (a stunned enemy, or loose debris back under the debris rules). A lit
//! bomb (`bomb`) grabs and throws the same way, but stops at the first enemy it hits and lies there until its fuse
//! runs out. Grab again while holding to set the thing down in front.

use avian3d::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use game_core::aim::level;
use game_core::combat::HitOutcome;
use game_core::debris::solid::{Settle, Solid};
use game_core::grab::{FLIGHT, Held, SPIN, impact};
use game_core::grab::{blow, passes, pick, stops, swing, throw_heading, throw_velocity};

use crate::arena::Arena;
use crate::bomb;
use crate::debris::Piece;
use crate::enemy::Brain;
use crate::fighter::{Attack, Body, Combat, Defeated, Hp, Layer, Strike, Team};
use crate::input::Actions;
use crate::player::{self, Facing, HEIGHT, Player, RADIUS};
use crate::{on_floor, plane};

/// The player has something in hand: what, and where in the carry it is.
#[derive(Component, Clone, Copy, Debug)]
pub struct Carry {
    pub target: Entity,
    pub kind: Held,
    pub phase: CarryPhase,
}

/// Where a carry is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CarryPhase {
    /// Out in front in the fists.
    Hold,
    /// `t` seconds into the throw's whirl, which lets go along `heading` (a yaw, radians) at [`SPIN`].
    Whirl { t: f32, heading: f32 },
}

impl Carry {
    pub fn whirling(&self) -> bool {
        matches!(self.phase, CarryPhase::Whirl { .. })
    }

    /// How far through the whirl (0 to 1) and along which heading, while whirling.
    pub fn whirl(&self) -> Option<(f32, f32)> {
        match self.phase {
            CarryPhase::Whirl { t, heading } => Some(((t / SPIN).min(1.0), heading)),
            CarryPhase::Hold => None,
        }
    }
}

/// Something the player can pick up, and what it is when held. Enemies, debris parts and lit bombs carry it from their
/// bundles (a lit bomb is `Grabbable(Held::Bomb)`).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grabbable(pub Held);

/// Being held: out of the physics world (it collides with nothing and no query finds it) and out of the fight. Keeps
/// what it collided with and its size across the floor, for carrying it and letting go.
#[derive(Component, Clone, Copy, Debug)]
pub struct Carried {
    layers: CollisionLayers,
    radius: f32,
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
/// How high above the player's centre a held thing is, metres: a held enemy's feet clear the floor, and a debris part
/// or a bomb sits in the fists at chest height.
const LIFT: f32 = 0.3;
/// How far back a held enemy is tipped, radians, and how far and how fast it sways about that, limp.
const TILT: f32 = 0.5;
const SWAY: f32 = 0.12;
const SWAY_RATE: f32 = 7.0;

pub fn plugin(app: &mut App) {
    // Grabbing and throwing only while playing: L or a click on a screen must not pick anything up or throw it. The
    // throw starts before the grab, so an attack in the frame a grab lands doesn't also throw.
    let playing = in_state(crate::flow::Phase::Playing);
    app.add_systems(
        Update,
        (throw.run_if(playing.clone()), grab.run_if(playing), carry, fly).chain().after(player::control).before(Combat),
    );
}

/// A thing's radius across the floor, for carrying it, metres.
fn size(body: Option<&Body>, piece: Option<&Piece>) -> f32 {
    match (body, piece.map(|p| p.solid)) {
        (Some(b), _) => b.radius,
        (None, Some(Solid::Cube([x, _, z]))) => x.max(z) / 2.0,
        (None, Some(Solid::Ball(r))) => r,
        // A lit bomb.
        (None, None) => bomb::RADIUS,
    }
}

/// Where the held thing sits for a player standing at `me` facing `yaw`: out in front in the fists, or `spin` (0..=1)
/// through the throw. Never past the walls, so a throw or a set-down beside one doesn't start inside it.
fn hold_at(arena: &Arena, me: Vec3, yaw: f32, carried: &Carried, spin: Option<f32>) -> Vec3 {
    // Pressed close to the fists: a body's near side, not its centre, is at arm's length.
    let hold = RADIUS + 0.2 + carried.radius * 0.6;
    let (across, up) = swing(yaw, LIFT, hold, RADIUS + carried.radius + 0.1, spin);
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
    players: Query<(Entity, &Transform, &Facing, &Hp, Option<&Carry>), With<Player>>,
    mut things: Query<Candidates, (Without<Player>, Without<Carried>, Without<Thrown>)>,
    mut held: Query<(&mut Transform, &mut Position, &mut Rotation, &Carried), Without<Player>>,
) {
    let Ok((me, at, facing, hp, holding)) = players.single() else { return };
    if !actions.verbs.grab || hp.0.stunned() {
        return;
    }
    let (here, yaw) = (at.translation, facing.0.yaw);
    if let Some(h) = holding {
        // Set it down in front, at the player's middle and upright: it drops from there.
        if !h.whirling()
            && let Ok((mut t, mut p, mut r, carried)) = held.get_mut(h.target)
        {
            t.translation = hold_at(&arena, here, yaw, carried, Some(1.0));
            p.0 = t.translation;
            (t.rotation, r.0) = (Quat::IDENTITY, Quat::IDENTITY);
            let layers = carried.layers;
            commands.entity(h.target).remove::<Carried>().insert((RigidBody::Dynamic, layers, LinearVelocity::ZERO));
            commands.entity(me).remove::<Carry>();
        }
        return;
    }
    // In front, within reach, and between the feet and the top of the head; nearest first.
    let feet = here.y - HEIGHT / 2.0;
    let reachable: Vec<(Entity, [f32; 2], f32)> = things
        .iter()
        .filter(|(.., hp, _)| hp.is_none_or(|h| !h.0.defeated()))
        .filter(|(_, _, t, ..)| (feet - 0.15..=feet + HEIGHT + 0.15).contains(&t.translation.y))
        .map(|(e, _, t, _, body, piece, ..)| (e, plane(t.translation), size(body, piece)))
        .collect();
    let Some(i) = pick(plane(here), yaw, RADIUS, reachable.iter().map(|(_, p, r)| (*p, *r))) else { return };
    let target = reachable[i].0;
    let Ok((_, &Grabbable(kind), _, &layers, body, piece, _, attack)) = things.get_mut(target) else { return };
    let radius = size(body, piece.as_deref());
    if let Some(mut piece) = piece {
        // Thawed: it starts again as a fresh loose piece, so it freezes only after it lands and rests.
        piece.born = time.elapsed_secs();
        piece.settle = Settle::default();
    }
    if let Some(mut attack) = attack {
        *attack = Attack::default(); // a held enemy's swing stops
    }
    commands.entity(target).insert((
        Carried { layers, radius },
        RigidBody::Kinematic,
        CollisionLayers::NONE,
        LinearVelocity::ZERO,
        AngularVelocity::ZERO,
    ));
    commands.entity(me).insert(Carry { target, kind, phase: CarryPhase::Hold });
}

/// An attack while holding starts the throw: aimed now, toward the biggest enemy group within 30° of the facing (Q62),
/// with the player turned to face it. The whirl then lets go along that heading whatever else happens meanwhile.
#[allow(clippy::type_complexity)]
fn throw(
    actions: Res<Actions>,
    mut players: Query<(&Transform, &mut Facing, &Hp, &mut Carry), With<Player>>,
    enemies: Query<&Transform, (With<Brain>, Without<Player>, Without<Carried>)>,
) {
    let Ok((at, mut facing, hp, mut carry)) = players.single_mut() else { return };
    if !(actions.verbs.punch || actions.verbs.kick) || hp.0.stunned() || carry.whirling() {
        return;
    }
    let me = at.translation;
    let enemies: Vec<[f32; 2]> =
        enemies.iter().map(|t| t.translation).filter(|e| level(me.y, e.y)).map(plane).collect();
    let heading = throw_heading(plane(me), facing.0.yaw, &enemies);
    facing.0.snap(heading);
    carry.phase = CarryPhase::Whirl { t: 0.0, heading };
}

/// Keeps the held thing in the fists or whirling round the player, and at the end of the spin throws it straight
/// ahead. A held enemy hangs tipped back, away from the player, and sways limply.
#[allow(clippy::type_complexity)]
fn carry(
    mut commands: Commands,
    time: Res<Time>,
    arena: Res<Arena>,
    mut players: Query<(Entity, &Transform, &Facing, &mut Carry), With<Player>>,
    mut held: Query<
        (&mut Transform, &mut Position, &mut Rotation, &Carried, Option<&mut Hp>, Option<&mut Facing>),
        Without<Player>,
    >,
) {
    let Ok((me, at, facing, mut h)) = players.single_mut() else { return };
    let Ok((mut t, mut p, mut r, carried, hp, held_facing)) = held.get_mut(h.target) else {
        commands.entity(me).remove::<Carry>(); // gone (the box emptied)
        return;
    };
    if let CarryPhase::Whirl { t, .. } = &mut h.phase {
        *t += time.delta_secs();
    }
    // Held out along the facing; whirling, round from the throw's heading and back to it.
    let (progress, yaw) = match h.phase {
        CarryPhase::Hold => (None, facing.0.yaw),
        CarryPhase::Whirl { t, heading } => (Some(t / SPIN), heading),
    };
    t.translation = hold_at(&arena, at.translation, yaw, carried, progress);
    p.0 = t.translation;
    if h.kind == Held::Enemy {
        let out = (t.translation - at.translation).with_y(0.0).normalize_or(on_floor([yaw.cos(), yaw.sin()]));
        // It faces the player who holds it.
        if let Some(mut f) = held_facing
            && let Some(towards) = game_core::plane::heading(plane(-out))
        {
            f.0.snap(towards);
        }
        let sway = SWAY * (time.elapsed_secs() * SWAY_RATE).sin();
        r.0 = Quat::from_axis_angle(Vec3::Y.cross(out).normalize(), TILT + sway);
        t.rotation = r.0;
    }
    if progress.is_none_or(|p| p < 1.0) {
        return;
    }
    // Let go: a projectile flying flat along the heading that passes through fighters (it strikes them in `fly`).
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
    commands.entity(me).remove::<Carry>();
}

/// What a throw sets off: a strike on each enemy it passes through, and the thrown enemy's own defeat if its first
/// impact finishes it.
#[derive(SystemParam)]
struct Impacts<'w> {
    strikes: MessageWriter<'w, Strike>,
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
    standing: Query<&Hp, Without<Thrown>>,
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
            if standing.get(target).is_ok_and(|hp| hp.0.defeated()) {
                continue;
            }
            t.victims.push(target);
            let dir = on_floor([t.heading.cos(), t.heading.sin()]);
            let at = (them - dir * body.radius).with_y(now.y);
            out.strikes.write(Strike { target, blow: blow(t.kind, t.heading), at, dir });
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
            let mut landed = commands.entity(e);
            landed.remove::<(Thrown, GravityScale)>().insert(layers);
            // A thrown enemy left the fists tipped back; it lands on its feet.
            if t.kind == Held::Enemy {
                landed.insert(Rotation::IDENTITY);
            }
        }
    }
}
