//! Everything that fights: teams, hit points, swings, hit resolution, knockback, hit-stun and hit-stop.
//!
//! The rules live in `game_core::combat`; this module applies them to ECS entities and physics bodies. Visuals
//! (flash, telegraphs, sparks) are `look`'s job, driven by the state here and the [`Hit`] message.

use avian2d::prelude::*;
use bevy::prelude::*;
use game_core::combat::{Aabb, Health, HitOutcome, HitStop, Move, Swing, hitbox, knockback};
use game_core::movement::{MAX_STEP, step_hop};

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

/// Body half-size and facing (+1 right, -1 left).
#[derive(Component)]
pub struct Fighter {
    pub half: Vec2,
    pub facing: f32,
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

/// White hit flash, seconds remaining.
#[derive(Component, Default)]
pub struct Flash(pub f32);

/// A blow landed: where, and with which move (for sparks and sound).
#[derive(Message, Clone, Copy, Debug)]
pub struct Hit {
    pub at: Vec2,
    pub mv: Move,
    pub facing: f32,
}

/// A fighter's hit points ran out.
#[derive(Message, Clone, Copy, Debug)]
pub struct Defeated {
    pub entity: Entity,
    pub team: Team,
}

#[derive(Resource, Default)]
pub struct HitStopClock(pub HitStop);

/// Seconds a hit flashes white.
pub const FLASH: f32 = 0.12;

pub fn plugin(app: &mut App) {
    app.init_resource::<HitStopClock>()
        .add_message::<Hit>()
        .add_message::<Defeated>()
        .add_systems(Update, (advance_swings, resolve_hits, finish_swings, tick_health).chain())
        // Only while playing: a freeze ending must not unpause a paused game.
        .add_systems(Update, hit_stop.run_if(in_state(crate::flow::Phase::Playing)))
        // After the player's and enemies' controls have set this frame's run velocity.
        .add_systems(PostUpdate, step_up.before(PhysicsSystems::First));
}

/// Physics and combat components every fighter carries.
pub fn body(team: Team, half: Vec2, max_hp: f32) -> impl Bundle {
    (
        team,
        Fighter { half, facing: 1.0 },
        Hp(Health::new(max_hp)),
        Attack::default(),
        Flash::default(),
        RigidBody::Dynamic,
        // A capsule: its rounded bottom rides up the edges of debris, so fighters climb piles without a jump.
        Collider::capsule(half.x, half.y * 2.0 - half.x * 2.0),
        LockedAxes::ROTATION_LOCKED,
        // Frictionless so walls don't hold fighters up; horizontal control is set directly on velocity.
        Friction::ZERO.with_combine_rule(CoefficientCombine::Min),
        Restitution::ZERO,
        CollisionLayers::new(Layer::Fighter, [Layer::World, Layer::Debris]),
        // Thin downward cast just below the feet: any wall or debris hit means standing (not on other fighters).
        ShapeCaster::new(Collider::rectangle(half.x * 1.8, 2.0), Vec2::new(0.0, -half.y), 0.0, Dir2::NEG_Y)
            .with_max_distance(4.0)
            .with_query_filter(SpatialQueryFilter::from_mask([Layer::World, Layer::Debris])),
    )
}

/// Walking into a low ledge (debris, mostly) hops the fighter onto it: a capsule alone stalls against a step's edge.
/// A forward ray at the feet finds the ledge, a ray `MAX_STEP` higher checks it is low enough, and a downward ray
/// measures its height for the hop.
fn step_up(spatial: SpatialQuery, gravity: Res<Gravity>, mut fighters: Query<(&Fighter, &Transform, &ShapeHits, &mut LinearVelocity)>) {
    let filter = SpatialQueryFilter::from_mask([Layer::World, Layer::Debris]);
    for (fighter, transform, ground, mut velocity) in &mut fighters {
        if ground.is_empty() || velocity.x.abs() < 20.0 || velocity.y > 1.0 {
            continue;
        }
        let dir = velocity.x.signum();
        let Ok(forward) = Dir2::new(Vec2::X * dir) else { continue };
        let feet = transform.translation.truncate() - Vec2::Y * fighter.half.y;
        let reach = fighter.half.x + 6.0;
        if spatial.cast_ray(feet + Vec2::Y * 3.0, forward, reach, true, &filter).is_none()
            || spatial.cast_ray(feet + Vec2::Y * (MAX_STEP + 2.0), forward, reach, true, &filter).is_some()
        {
            continue;
        }
        let above = feet + Vec2::new(dir * reach, MAX_STEP + 2.0);
        let Some(top) = spatial.cast_ray(above, Dir2::NEG_Y, MAX_STEP + 2.0, true, &filter) else { continue };
        if let Some(hop) = step_hop(MAX_STEP + 2.0 - top.distance, gravity.0.length()) {
            velocity.y = hop;
        }
    }
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

fn resolve_hits(
    mut attackers: Query<(&Team, &Fighter, &Transform, &mut Attack)>,
    targets: Query<(Entity, &Team, &Fighter, &Transform)>,
    mut hurt: Query<(&mut Hp, &mut LinearVelocity, &mut Flash)>,
    mut stop: ResMut<HitStopClock>,
    mut hits_out: MessageWriter<Hit>,
    mut defeated: MessageWriter<Defeated>,
) {
    let bodies: Vec<(Entity, Team, Aabb)> = targets
        .iter()
        .map(|(e, team, f, t)| (e, *team, Aabb::around((t.translation.x, t.translation.y), (f.half.x, f.half.y))))
        .collect();
    let mut hits: Vec<(Entity, Move, f32, Vec2)> = Vec::new();
    for (team, fighter, transform, mut attack) in &mut attackers {
        let Some(swing) = attack.swing.filter(Swing::struck_this_frame) else { continue };
        let pos = (transform.translation.x, transform.translation.y);
        let reach = hitbox(swing.mv, pos, (fighter.half.x, fighter.half.y), fighter.facing);
        for (target, target_team, body) in &bodies {
            if target_team != team && !attack.victims.contains(target) && reach.overlaps(body) {
                attack.victims.push(*target);
                let at = Vec2::new((reach.min.0.max(body.min.0) + reach.max.0.min(body.max.0)) / 2.0, (reach.min.1 + reach.max.1) / 2.0);
                hits.push((*target, swing.mv, fighter.facing, at));
            }
        }
    }
    for (target, mv, facing, at) in hits {
        let Ok((mut hp, mut velocity, mut flash)) = hurt.get_mut(target) else { continue };
        if hp.0.defeated() {
            continue;
        }
        let (kx, ky) = knockback(mv, facing);
        velocity.0 = Vec2::new(kx, ky);
        flash.0 = FLASH;
        stop.0.trigger(mv.spec().hitstop);
        hits_out.write(Hit { at, mv, facing });
        if hp.0.take(mv) == HitOutcome::Defeated
            && let Ok((_, team, _, _)) = targets.get(target)
        {
            defeated.write(Defeated { entity: target, team: *team });
        }
    }
}

fn tick_health(time: Res<Time>, mut fighters: Query<(&mut Hp, &mut Flash)>) {
    for (mut hp, mut flash) in &mut fighters {
        hp.0.tick(time.delta_secs());
        flash.0 = (flash.0 - time.delta_secs()).max(0.0);
    }
}

/// Freezes the virtual clock (and with it physics and gameplay) for the hit-stop duration, using real time.
fn hit_stop(real: Res<Time<Real>>, mut virt: ResMut<Time<Virtual>>, mut stop: ResMut<HitStopClock>) {
    if stop.0.tick(real.delta_secs()) {
        virt.pause();
    } else if virt.is_paused() {
        virt.unpause();
    }
}
