//! Everything that fights: teams, bodies, hit points, swings, hit resolution in a cone, and where every blow lands.
//!
//! A swing, a thrown thing (`grab`) and a bomb blast (`bomb`) each send a [`Strike`]: a `game_core::combat::Blow` and
//! whom it reaches. [`land`] applies them all the same way through `combat::land` (damage, armour, stun, launch,
//! flash, hit-stop including the finisher's longer freeze), then raises [`Hit`] and [`Defeated`]. The cone and the
//! knockback rules live in `game_core::aim`. Visuals (flash, telegraphs, bursts) belong to the render modules, driven
//! by the state here and the [`Hit`] message.

use avian3d::prelude::*;
use bevy::prelude::*;
use game_core::aim::{Cone, level};
use game_core::combat::{Blow, Health, HitStop, Move, Swing};
use game_core::enemy::Armour;
use game_core::impact::Feel;
use game_core::plane::heading;

use crate::enemy::Brain;
use crate::grab::Carried;
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

/// A blow on its way to `target`: [`land`] applies it. `at` (on the target's body, at the blow's height) and `dir`
/// (the way it pushes) are where its [`Hit`] shows.
#[derive(Message, Clone, Copy, Debug)]
pub struct Strike {
    pub target: Entity,
    pub blow: Blow,
    pub at: Vec3,
    pub dir: Vec3,
}

/// A blow landed: where (on the victim's body, at the blow's height), how it reads, which way it pushed, on whom, and
/// whether it finished them.
#[derive(Message, Clone, Copy, Debug)]
pub struct Hit {
    pub at: Vec3,
    pub feel: Feel,
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

/// Swings advance, land and end here, and every [`Strike`] lands; whatever starts a swing (the player's control,
/// enemy brains) or sends a strike (a throw, a blast) runs before it, and whatever reacts to hits and defeats (debris,
/// score) after it.
#[derive(SystemSet, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Combat;

pub fn plugin(app: &mut App) {
    app.init_resource::<HitStopClock>()
        .add_message::<Strike>()
        .add_message::<Hit>()
        .add_message::<Defeated>()
        .add_systems(
            Update,
            (
                advance_swings,
                resolve_hits,
                land,
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
    mut strikes: MessageWriter<Strike>,
) {
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
            let at = (them - dir * their_body.radius).with_y(me.y + swing.mv.spec().offset_y);
            strikes.write(Strike { target, blow: Blow::swing(swing.mv, yaw), at, dir });
        }
    }
}

/// What a strike can land on: a fighter's side and place, health, velocity and flash, its brain (for armour), and
/// whether the player holds it.
type Struck = (
    &'static Team,
    &'static Transform,
    &'static mut Hp,
    &'static mut LinearVelocity,
    &'static mut Flash,
    Option<&'static Brain>,
    Has<Carried>,
);

/// Every strike lands here, whatever sent it: through `combat::land` (nothing lands on the defeated, nor on what the
/// player holds unless the blow reaches held things), then the flash, the launch, the hit-stop, the [`Hit`] and, for
/// the last of a fighter's health, its [`Defeated`].
fn land(
    mut strikes: MessageReader<Strike>,
    mut fighters: Query<Struck>,
    mut stop: ResMut<HitStopClock>,
    mut hits: MessageWriter<Hit>,
    mut defeated: MessageWriter<Defeated>,
) {
    for s in strikes.read() {
        let Ok((team, tf, mut hp, mut velocity, mut flash, brain, held)) = fighters.get_mut(s.target) else { continue };
        let armour = brain.map_or(Armour::None, |b| b.0.style().armour);
        let Some(landed) = game_core::combat::land(&s.blow, &mut hp.0, armour, held) else { continue };
        flash.0 = FLASH;
        if let Some((across, up)) = landed.launch {
            velocity.0 = on_floor(across).with_y(up);
        }
        if let Some(freeze) = landed.freeze {
            stop.0.trigger(freeze);
        }
        let finisher = landed.finisher();
        if let Some(feel) = s.blow.feel {
            hits.write(Hit { at: s.at, feel, dir: s.dir, victim: s.target, finisher });
        }
        if finisher {
            defeated.write(Defeated { entity: s.target, team: *team, at: tf.translation });
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
