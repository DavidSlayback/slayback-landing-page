//! Everything that fights: teams, bodies, hit points, swings, hit resolution in a cone, knockback, hit-stun and
//! hit-stop (including the finisher's longer freeze).
//!
//! The rules live in `game_core::combat` (timings, damage, hit-stop) and `game_core::aim` (the cone and knockback);
//! this module applies them to entities and physics bodies. Visuals (flash, telegraphs, bursts) belong to the render
//! modules, driven by the state here and the [`Hit`] message.

use avian3d::prelude::*;
use bevy::prelude::*;
use game_core::aim::{Cone, launch, level};
use game_core::combat::{Health, HitOutcome, HitStop, Move, Swing, hitstop};
use game_core::enemy::staggers;
use game_core::plane::heading;

use crate::enemy::Brain;
use crate::player::Facing;
use crate::{on_floor, plane};

/// Collision layers. Fighters pass through each other (a crowd can't pin the player) but stand on walls and debris.
#[derive(PhysicsLayer, Clone, Copy, Debug, Default)]
pub enum Layer {
    #[default]
    World,
    Fighter,
    Debris,
}

/// Which side an entity fights for; swings never hit their own side.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Team {
    Player,
    Enemy,
}

/// A fighter's capsule: radius and full height, metres.
#[derive(Component, Clone, Copy, Debug)]
pub struct Body {
    pub radius: f32,
    pub height: f32,
}

#[derive(Component)]
pub struct Hp(pub Health);

/// The swing in progress, if any, and who it has already hit (one hit per target per swing).
#[derive(Component, Default)]
pub struct Attack {
    pub swing: Option<Swing>,
    victims: Vec<Entity>,
}

impl Attack {
    pub fn busy(&self) -> bool {
        self.swing.is_some()
    }

    pub fn start(&mut self, mv: Move) {
        if self.swing.is_none() {
            self.swing = Some(Swing::new(mv));
            self.victims.clear();
        }
    }
}

/// Hit flash, seconds remaining.
#[derive(Component, Default)]
pub struct Flash(pub f32);

/// Seconds a hit flashes.
pub const FLASH: f32 = 0.12;

/// A blow landed: where (on the victim's body, at the blow's height), with which move, which way it pushed, on whom,
/// and whether it finished them.
#[derive(Message, Clone, Copy, Debug)]
pub struct Hit {
    pub at: Vec3,
    pub mv: Move,
    pub dir: Vec3,
    pub victim: Entity,
    pub finisher: bool,
}

/// A fighter's hit points ran out with its centre at `at`.
#[derive(Message, Clone, Copy, Debug)]
pub struct Defeated {
    pub entity: Entity,
    pub team: Team,
    pub at: Vec3,
}

#[derive(Resource, Default)]
pub struct HitStopClock(pub HitStop);

/// Swings advance, land and end here; whatever starts a swing (the player's control, enemy brains) runs before it,
/// and whatever reacts to hits and defeats (debris, score) after it.
#[derive(SystemSet, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Combat;

pub fn plugin(app: &mut App) {
    app.init_resource::<HitStopClock>().add_message::<Hit>().add_message::<Defeated>().add_systems(
        Update,
        (
            advance_swings,
            resolve_hits,
            finish_swings,
            tick_health,
            // Only while playing: the screens hold the clock paused, and the hit-stop would let it go.
            hit_stop.run_if(in_state(crate::flow::Phase::Playing)),
        )
            .chain()
            .in_set(Combat),
    );
}

/// Physics and combat components every fighter carries (with a `Facing` of its own): a dynamic capsule that stays upright, slides along walls,
/// and passes through other fighters.
pub fn body(team: Team, radius: f32, height: f32, max_hp: f32) -> impl Bundle {
    (
        (team, Body { radius, height }, Hp(Health::new(max_hp)), Attack::default(), Flash::default()),
        RigidBody::Dynamic,
        Collider::capsule(radius, height - 2.0 * radius),
        LockedAxes::ROTATION_LOCKED,
        // Frictionless, so walls don't hold fighters; floor movement is set on the velocity.
        Friction::ZERO.with_combine_rule(CoefficientCombine::Min),
        Restitution::ZERO,
        CollisionLayers::new(Layer::Fighter, [Layer::World, Layer::Debris]),
        SleepingDisabled,
        TransformInterpolation,
    )
}

fn advance_swings(time: Res<Time>, mut attacks: Query<&mut Attack>) {
    for mut attack in &mut attacks {
        if let Some(swing) = attack.swing.as_mut() {
            swing.advance(time.delta_secs());
        }
    }
}

/// After hits are resolved: a swing that finished this frame still got its chance to land.
fn finish_swings(mut attacks: Query<&mut Attack>) {
    for mut attack in &mut attacks {
        if attack.swing.is_some_and(|s| s.phase().is_none()) {
            attack.swing = None;
        }
    }
}

/// Every swing whose active window overlapped this frame strikes every opposing body inside its cone and level with
/// the attacker, once per swing.
fn resolve_hits(
    mut attackers: Query<(&Team, &Body, &Transform, &Facing, &mut Attack)>,
    targets: Query<(Entity, &Team, &Body, &Transform)>,
    mut hurt: Query<(&mut Hp, &mut LinearVelocity, &mut Flash, Option<&Brain>)>,
    mut stop: ResMut<HitStopClock>,
    mut hits_out: MessageWriter<Hit>,
    mut defeated: MessageWriter<Defeated>,
) {
    let mut hits: Vec<(Entity, Move, Vec3, Vec3)> = Vec::new();
    for (team, body, transform, facing, mut attack) in &mut attackers {
        let Some(swing) = attack.swing.filter(Swing::struck_this_frame) else { continue };
        let me = transform.translation;
        let cone = Cone::of(swing.mv, plane(me), facing.0.yaw, body.radius);
        for (target, their_team, their_body, their_tf) in &targets {
            let them = their_tf.translation;
            if their_team == team
                || attack.victims.contains(&target)
                || !level(me.y, them.y)
                || !cone.contains(plane(them), their_body.radius)
            {
                continue;
            }
            attack.victims.push(target);
            // Pushed away from the attacker (along the facing if they overlap), struck on the near side of the body.
            let yaw = heading(plane(them - me)).unwrap_or(facing.0.yaw);
            let dir = on_floor([yaw.cos(), yaw.sin()]);
            let height = swing.mv.spec().offset_y;
            let at = (them - dir * their_body.radius).with_y(me.y + height);
            hits.push((target, swing.mv, dir, at));
        }
    }
    for (target, mv, dir, at) in hits {
        let Ok((mut hp, mut velocity, mut flash, brain)) = hurt.get_mut(target) else { continue };
        if hp.0.defeated() {
            continue;
        }
        flash.0 = FLASH;
        // Armour shrugs off some blows: the damage lands, but no knockback or stun, so a telegraph keeps going.
        let outcome = if brain.is_none_or(|b| staggers(mv, b.0.style().armour)) {
            let (across, up) = launch(mv, heading(plane(dir)).unwrap_or(0.0));
            velocity.0 = on_floor(across).with_y(up);
            hp.0.take(mv)
        } else {
            hp.0.hurt(mv.spec().damage)
        };
        let finisher = outcome == HitOutcome::Defeated;
        stop.0.trigger(hitstop(mv, finisher));
        hits_out.write(Hit { at, mv, dir, victim: target, finisher });
        if finisher && let Ok((_, team, _, tf)) = targets.get(target) {
            defeated.write(Defeated { entity: target, team: *team, at: tf.translation });
        }
    }
}

fn tick_health(time: Res<Time>, mut fighters: Query<(&mut Hp, &mut Flash)>) {
    for (mut hp, mut flash) in &mut fighters {
        hp.0.tick(time.delta_secs());
        flash.0 = (flash.0 - time.delta_secs()).max(0.0);
    }
}

/// Freezes the virtual clock (and with it physics and gameplay) for the hit-stop, timed in real time.
fn hit_stop(real: Res<Time<Real>>, mut virt: ResMut<Time<Virtual>>, mut stop: ResMut<HitStopClock>) {
    if stop.0.tick(real.delta_secs()) {
        virt.pause();
    } else if virt.is_paused() {
        virt.unpause();
    }
}
