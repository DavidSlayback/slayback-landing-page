//! The player: moves in eight directions over the floor and faces the way it last moved, turning smoothly (Q49); dashes;
//! and punches, kicks and drop-kicks (any attack during a dash), with aim assist turning a swing toward the nearest
//! enemy inside its cone. Carrying something (`carry`), an attack is the throw instead, which `carry` aims and runs;
//! the player plants its feet for the whirl.
//!
//! The body is a kinematic capsule moved by a move-and-slide controller ([`walk`], avian's `MoveAndSlide`): it slides
//! along walls and frozen debris, falls and lands, steps up ledges up to `game_core::climb::MAX_STEP` (no jump, Q59)
//! and walks up slopes up to `climb::MAX_SLOPE`, which is how it climbs the pile. Its `LinearVelocity` is the controller's
//! velocity: input sets the floor part, knockback sets all of it, and gravity pulls it down while airborne.

use avian3d::character_controller::move_and_slide::MoveHitData;
use avian3d::prelude::*;
use bevy::prelude::*;
use game_core::aim::{Cone, level};
use game_core::climb::{MAX_STEP, walkable};
use game_core::combat::Move;
use game_core::plane::{self, Dash, run_velocity};

use crate::Frame;
use crate::carry::{Carried, Carry};
use crate::enemy::Brain;
use crate::fighter::{Attack, Hp, Layer, Team, body};
use crate::input::Actions;
use crate::{on_floor, plane as to_plane};

/// The player.
#[derive(Component, Default)]
pub struct Player;

/// The player's dash (`game_core::plane::Dash`).
#[derive(Component, Default)]
pub struct Dasher(pub Dash);

/// Which way a fighter faces on the floor (`game_core::plane::Facing`).
#[derive(Component, Default, Clone, Copy, Debug)]
pub struct Facing(pub plane::Facing);

impl Facing {
    /// The world rotation for this facing, for a figure built looking along +x.
    pub fn rotation(&self) -> Quat {
        Quat::from_rotation_y(self.0.yaw)
    }
}

/// Capsule radius and full height, metres.
pub const RADIUS: f32 = 0.35;
pub const HEIGHT: f32 = 1.75;
pub const MAX_HP: f32 = 100.0;
/// Spawn point (the capsule's centre): mid-floor, a little nearer the camera, just above the floor.
pub const SPAWN: Vec3 = Vec3::new(0.0, HEIGHT / 2.0 + 0.3, 1.5);

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn)
        // Only while playing (`Frame::Decide`): the click or Space that leaves a screen must not also punch or dash.
        .add_systems(Update, control.in_set(Frame::Decide))
        .add_systems(FixedUpdate, walk);
}

fn spawn(mut commands: Commands) {
    commands
        .spawn((
            Player,
            Name::new("player"),
            Dasher::default(),
            Facing::default(),
            body(Team::Player, RADIUS, HEIGHT, MAX_HP),
            Transform::from_translation(SPAWN),
        ))
        // Kinematic, and moved only by `walk`: the solver neither pushes it nor integrates its velocity, but it still
        // shoves loose debris out of its way.
        .insert((RigidBody::Kinematic, CustomPositionIntegration));
}

/// The player's parts `control` steers.
type Controls = (
    &'static Transform,
    &'static mut LinearVelocity,
    &'static mut Facing,
    &'static mut Dasher,
    &'static mut Attack,
    &'static Hp,
    Option<&'static Carry>,
);

/// Turns this frame's actions (any device; see `input`) into floor velocity, facing, attacks and throws. Public so
/// other systems can order themselves after it.
#[allow(clippy::type_complexity)]
pub fn control(
    time: Res<Time>,
    actions: Res<Actions>,
    enemies: Query<&Transform, (With<Brain>, Without<Player>, Without<Carried>)>,
    mut players: Query<Controls, With<Player>>,
) {
    let Ok((transform, mut velocity, mut facing, mut dash, mut attack, hp, carry)) = players.single_mut() else {
        return;
    };
    let dt = time.delta_secs();
    if hp.0.stunned() {
        return; // knockback plays out
    }
    let button = if actions.verbs.punch {
        Some(Move::Punch)
    } else if actions.verbs.kick {
        Some(Move::Kick)
    } else {
        None
    };
    let me = transform.translation;
    // Carrying, an attack is the throw (`carry` starts it); otherwise it's a swing.
    if let (Some(button), None) = (button, carry)
        && !attack.busy()
    {
        let mv = Move::for_button(button, dash.0.dashing());
        // Aim assist: turn toward the nearest enemy in the swing's cone (a drop-kick's dash bends toward it too).
        let cone = Cone::of(mv, to_plane(me), facing.0.yaw, RADIUS);
        let level_enemies = enemies.iter().map(|t| t.translation).filter(|e| level(me.y, e.y)).map(to_plane);
        if let Some((_, yaw)) = cone.assist(level_enemies, crate::enemy::RADIUS) {
            facing.0.snap(yaw);
            dash.0.aim(facing.0.forward());
        }
        attack.start(mv);
    }
    // A punch, a kick or the whirl plants the feet; a drop-kick rides the dash.
    if attack.swing.is_some_and(|s| s.mv != Move::DropKick) || carry.is_some_and(Carry::whirling) {
        let v = run_velocity(to_plane(velocity.0), [0.0, 0.0], dt);
        velocity.x = v[0];
        velocity.z = -v[1];
        return;
    }
    let v = on_floor(dash.0.step(to_plane(velocity.0), actions.dir, actions.verbs.dash, facing.0.forward(), dt));
    velocity.x = v.x;
    velocity.z = v.z;
    facing.0.step(if dash.0.dashing() { dash.0.direction() } else { actions.dir }, dt);
}

/// How far below the feet the ground still counts as underfoot, metres.
const PROBE: f32 = 0.05;

/// The move-and-slide controller, once per physics step: gravity while airborne, the floor velocity slid along
/// whatever it meets (stepping up a ledge it would otherwise stop at), then the fall or the landing, and on the ground
/// a snap down onto lower footing, so walking down the pile doesn't turn into hops.
#[allow(clippy::type_complexity)]
fn walk(
    time: Res<Time>,
    gravity: Res<Gravity>,
    mut world: ParamSet<(
        MoveAndSlide,
        Query<(Entity, &Collider, &Position, &Rotation, &LinearVelocity), With<Player>>,
        Query<(&mut Position, &mut LinearVelocity), With<Player>>,
    )>,
) {
    let dt = time.delta_secs();
    let Some((entity, shape, at, turn, v)) =
        world.p1().single().ok().map(|(e, c, p, r, v)| (e, c.clone(), p.0, r.0, v.0))
    else {
        return;
    };
    if dt == 0.0 {
        return;
    }
    let (at, v) = {
        let mover = world.p0();
        let filter = SpatialQueryFilter::from_mask([Layer::World, Layer::Debris]).with_excluded_entities([entity]);
        Walker { mover: &mover, shape: &shape, turn, filter }.step(at, v, gravity.0.length(), dt)
    };
    if let Ok((mut position, mut velocity)) = world.p2().single_mut() {
        position.0 = at;
        velocity.0 = v;
    }
}

/// One body's move-and-slide queries: its shape and turn, and what it collides with.
struct Walker<'a, 'w, 's> {
    mover: &'a MoveAndSlide<'w, 's>,
    shape: &'a Collider,
    turn: Quat,
    filter: SpatialQueryFilter,
}

impl Walker<'_, '_, '_> {
    /// Moves a body at `at` with velocity `v` for `dt` seconds; returns where it ends up and its new velocity.
    fn step(&self, mut at: Vec3, mut v: Vec3, gravity: f32, dt: f32) -> (Vec3, Vec3) {
        let grounded = self.ground(at, PROBE).is_some();
        if grounded && v.y <= 0.0 {
            v.y = 0.0;
        } else {
            v.y -= gravity * dt;
        }
        // Across: slide along what's in the way; if that stops it short, try stepping up onto it.
        let across = v.with_y(0.0);
        if across.length_squared() > 1e-8 {
            let (mut to, mut slid) = self.slide(at, across, dt);
            let progress = (to - at).xz().length();
            if grounded
                && progress < across.length() * dt * 0.9
                && let Some((up, up_slid)) = self.step_up(at, across, dt)
                && (up - at).xz().length() > progress + 1e-3
            {
                (to, slid) = (up, up_slid);
            }
            at = to;
            v = slid.with_y(v.y);
        }
        // Up or down: a fall lands on footing, or slides off an edge or a round piece too steep to stand on.
        if v.y != 0.0 {
            let mut landed = false;
            let out = self.mover.move_and_slide(
                self.shape,
                at,
                self.turn,
                Vec3::Y * v.y,
                std::time::Duration::from_secs_f32(dt),
                &MoveAndSlideConfig::default(),
                &self.filter,
                |hit| {
                    if v.y < 0.0 && self.stands_on(*hit.position, hit.point, **hit.normal) {
                        landed = true;
                        return MoveAndSlideHitResponse::Abort;
                    }
                    MoveAndSlideHitResponse::Accept
                },
            );
            at = out.position;
            v.y = if landed { 0.0 } else { out.projected_velocity.y };
        } else if grounded && let Some(drop) = self.ground(at, MAX_STEP + PROBE) {
            at.y -= drop;
        }
        (at, v)
    }

    /// Slides `velocity` (along the floor) for `dt` from `at`; returns where it stops and the slid velocity. Slopes
    /// too steep to walk count as walls, so sliding never carries the body up them.
    fn slide(&self, at: Vec3, velocity: Vec3, dt: f32) -> (Vec3, Vec3) {
        let out = self.mover.move_and_slide(
            self.shape,
            at,
            self.turn,
            velocity,
            std::time::Duration::from_secs_f32(dt),
            &MoveAndSlideConfig::default(),
            &self.filter,
            |hit| {
                let n = **hit.normal;
                if n.y > 0.0
                    && !walkable(n.y)
                    && let Ok(wall) = Dir3::new(n.with_y(0.0))
                {
                    *hit.normal = wall;
                }
                MoveAndSlideHitResponse::Accept
            },
        );
        (out.position, out.projected_velocity.with_y(0.0))
    }

    /// Stepping up a ledge: rise up to `MAX_STEP` (less under something), slide across from there, then settle back
    /// down onto footing. `None` if there is nothing to stand on at the end.
    fn step_up(&self, at: Vec3, velocity: Vec3, dt: f32) -> Option<(Vec3, Vec3)> {
        let rise = self.cast(at, Vec3::Y * MAX_STEP).map_or(MAX_STEP, |h| h.distance);
        if rise < 0.02 {
            return None;
        }
        let (over, slid) = self.slide(at + Vec3::Y * rise, velocity, dt);
        let hit = self.cast(over, Vec3::NEG_Y * (rise + PROBE))?;
        let landed = over - Vec3::Y * hit.distance;
        (landed.y > at.y + 0.01 && self.stands_on(landed, hit.point1, hit.normal1)).then_some((landed, slid))
    }

    /// How far below `at` (within `reach`) the body would come to stand on something, if it would.
    fn ground(&self, at: Vec3, reach: f32) -> Option<f32> {
        let hit = self.cast(at, Vec3::NEG_Y * reach)?;
        self.stands_on(at - Vec3::Y * hit.distance, hit.point1, hit.normal1).then_some(hit.distance)
    }

    /// Whether a body centred at `centre`, touching a surface at `point` with outward `normal`, stands on it: the
    /// surface is walkable, or the touch is on the edge of a walkable top (a step's or a block's), found by a short
    /// ray down just past the edge.
    fn stands_on(&self, centre: Vec3, point: Vec3, normal: Vec3) -> bool {
        if walkable(normal.y) {
            return true;
        }
        let out = (point - centre).with_y(0.0).normalize_or_zero();
        let above = point + out * 0.03 + Vec3::Y * 0.05;
        self.mover
            .spatial_query
            .cast_ray(above, Dir3::NEG_Y, 0.15, true, &self.filter)
            .is_some_and(|h| walkable(h.normal.y))
    }

    fn cast(&self, at: Vec3, movement: Vec3) -> Option<MoveHitData> {
        let skin = MoveAndSlideConfig::default().skin_width;
        self.mover.cast_move(self.shape, at, self.turn, movement, skin, &self.filter)
    }
}
