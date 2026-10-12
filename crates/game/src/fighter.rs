//! Everything that fights: facing, hit points, swings, hit resolution, knockback, hit-stun and hit-stop.
//!
//! The rules live in `game_core::combat`; this module applies them to ECS entities and physics bodies.

use avian2d::prelude::*;
use bevy::prelude::*;
use game_core::combat::{Aabb, HitOutcome, HitStop, Move, Swing, hitbox, knockback};

use crate::Layer;

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
    pub color: Color,
}

#[derive(Component)]
pub struct Hp(pub game_core::combat::Health);

/// The swing in progress, if any, and who it has already hit (one hit per target per swing).
#[derive(Component, Default)]
pub struct Attack {
    pub swing: Option<Swing>,
    pub victims: Vec<Entity>,
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

/// Marks the child sprite that shows a swing's hitbox while it is active.
#[derive(Component)]
pub struct StrikeFx;

/// Sent when a fighter's hit points run out.
#[derive(Message)]
pub struct Defeated {
    pub entity: Entity,
    pub team: Team,
}

#[derive(Resource, Default)]
pub struct HitStopClock(pub HitStop);

pub fn plugin(app: &mut App) {
    app.init_resource::<HitStopClock>()
        .add_message::<Defeated>()
        .add_systems(Update, (advance_swings, resolve_hits, tick_health, show_strikes, flash).chain())
        .add_systems(Update, hit_stop);
}

/// Physics + combat components every fighter carries.
pub fn fighter_bundle(team: Team, half: Vec2, color: Color, max_hp: f32) -> impl Bundle {
    (
        team,
        Fighter { half, facing: 1.0, color },
        Hp(game_core::combat::Health::new(max_hp)),
        Attack::default(),
        Flash::default(),
        Sprite::from_color(color, half * 2.0),
        RigidBody::Dynamic,
        Collider::rectangle(half.x * 2.0, half.y * 2.0),
        LockedAxes::ROTATION_LOCKED,
        // Frictionless so walls don't hold fighters up; horizontal control is set directly on velocity.
        Friction::ZERO.with_combine_rule(CoefficientCombine::Min),
        Restitution::ZERO,
        CollisionLayers::new(Layer::Fighter, [Layer::World, Layer::Debris]),
        // Thin downward cast just below the feet: any world or debris hit means grounded (not other fighters).
        ShapeCaster::new(Collider::rectangle(half.x * 1.8, 2.0), Vec2::new(0.0, -half.y), 0.0, Dir2::NEG_Y)
            .with_max_distance(4.0)
            .with_query_filter(SpatialQueryFilter::from_mask([Layer::World, Layer::Debris])),
    )
}

/// The strike-box child every fighter carries (hidden until a swing is active).
pub fn strike_fx(color: Color) -> impl Bundle {
    (StrikeFx, Sprite::from_color(color, Vec2::ONE), Transform::from_xyz(0.0, 0.0, 0.5), Visibility::Hidden)
}

fn advance_swings(time: Res<Time>, mut attacks: Query<&mut Attack>) {
    for mut attack in &mut attacks {
        if let Some(swing) = attack.swing.as_mut()
            && !swing.advance(time.delta_secs())
        {
            attack.swing = None;
        }
    }
}

fn resolve_hits(
    mut attackers: Query<(&Team, &Fighter, &Transform, &mut Attack)>,
    targets: Query<(Entity, &Team, &Fighter, &Transform)>,
    mut hurt: Query<(&mut Hp, &mut LinearVelocity, &mut Flash)>,
    mut stop: ResMut<HitStopClock>,
    mut defeated: MessageWriter<Defeated>,
) {
    let bodies: Vec<(Entity, Team, Aabb)> = targets
        .iter()
        .map(|(e, team, f, t)| (e, *team, Aabb::around((t.translation.x, t.translation.y), (f.half.x, f.half.y))))
        .collect();
    let mut hits: Vec<(Entity, Move, f32)> = Vec::new();
    for (team, fighter, transform, mut attack) in &mut attackers {
        let Some(swing) = attack.swing.filter(Swing::is_active) else { continue };
        let pos = (transform.translation.x, transform.translation.y);
        let reach = hitbox(swing.mv, pos, (fighter.half.x, fighter.half.y), fighter.facing);
        for (target, target_team, body) in &bodies {
            if target_team != team && !attack.victims.contains(target) && reach.overlaps(body) {
                attack.victims.push(*target);
                hits.push((*target, swing.mv, fighter.facing));
            }
        }
    }
    for (target, mv, facing) in hits {
        let Ok((mut hp, mut velocity, mut flash)) = hurt.get_mut(target) else { continue };
        if hp.0.defeated() {
            continue;
        }
        let (kx, ky) = knockback(mv, facing);
        velocity.0 = Vec2::new(kx, ky);
        flash.0 = 0.12;
        stop.0.trigger(mv.spec().hitstop);
        if hp.0.take(mv) == HitOutcome::Defeated
            && let Ok((_, team, _, _)) = targets.get(target)
        {
            defeated.write(Defeated { entity: target, team: *team });
        }
    }
}

fn tick_health(time: Res<Time>, mut hps: Query<&mut Hp>) {
    for mut hp in &mut hps {
        hp.0.tick(time.delta_secs());
    }
}

/// Positions and shows each fighter's strike box during the active part of its swing.
fn show_strikes(fighters: Query<(&Fighter, &Attack, &Children)>, mut fx: Query<(&mut Transform, &mut Visibility), With<StrikeFx>>) {
    for (fighter, attack, children) in &fighters {
        for child in children.iter() {
            let Ok((mut transform, mut visible)) = fx.get_mut(child) else { continue };
            match attack.swing.filter(Swing::is_active) {
                Some(swing) => {
                    let b = hitbox(swing.mv, (0.0, 0.0), (fighter.half.x, fighter.half.y), fighter.facing);
                    transform.translation.x = (b.min.0 + b.max.0) / 2.0;
                    transform.translation.y = (b.min.1 + b.max.1) / 2.0;
                    transform.scale = Vec3::new(b.max.0 - b.min.0, b.max.1 - b.min.1, 1.0);
                    *visible = Visibility::Inherited;
                }
                None => *visible = Visibility::Hidden,
            }
        }
    }
}

fn flash(time: Res<Time>, mut fighters: Query<(&Fighter, &mut Flash, &mut Sprite)>) {
    for (fighter, mut flash, mut sprite) in &mut fighters {
        flash.0 = (flash.0 - time.delta_secs()).max(0.0);
        sprite.color = if flash.0 > 0.0 { Color::WHITE } else { fighter.color };
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
