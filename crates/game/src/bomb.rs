//! Bomb-heads (rules: `game_core::bomb`). A bomb-head enemy's head is a bomb: when it's defeated the bomb is lit and
//! drops free, blinking for 1.5 s, then blows: it hurts enemies and the player nearby, and knocks debris loose (thawed
//! and flung, never deleted). A lit bomb can be grabbed and thrown (see `grab`).

use avian2d::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use game_core::bomb::{Fuse, damage, push};
use game_core::combat::HitOutcome;

use crate::debris::DebrisPart;
use crate::fighter::{Defeated, FLASH, Fighter, Flash, HitStopClock, Hp, Layer, Team};
use crate::grab::Carried;

/// This enemy's head is a bomb.
#[derive(Component)]
pub struct BombHead;

/// A lit bomb, burning down.
#[derive(Component)]
pub struct LitBomb {
    pub fuse: Fuse,
}

/// A bomb went off here.
#[derive(Message, Clone, Copy, Debug)]
pub struct Exploded {
    pub at: Vec2,
}

/// Bomb radius, world units (a little bigger than a head, so it reads as a bomb).
pub const BOMB_RADIUS: f32 = 9.0;

pub fn plugin(app: &mut App) {
    app.add_message::<Exploded>().add_systems(Update, burn);
}

/// A lit bomb at `at`, flying off with `velocity`.
pub fn lit_bomb(at: Vec2, velocity: Vec2) -> impl Bundle {
    (
        LitBomb { fuse: Fuse::default() },
        Name::new("bomb"),
        RigidBody::Dynamic,
        Collider::circle(BOMB_RADIUS),
        CollisionLayers::new(Layer::Debris, [Layer::World, Layer::Fighter, Layer::Debris]),
        Restitution::new(0.3),
        Transform::from_translation(at.extend(0.0)),
        LinearVelocity(velocity),
    )
}

/// What a blast sets off besides the push: a freeze, defeats, and the flash and sparks (via `Exploded`).
#[derive(SystemParam)]
struct BlastEffects<'w> {
    stop: ResMut<'w, HitStopClock>,
    defeated: MessageWriter<'w, Defeated>,
    exploded: MessageWriter<'w, Exploded>,
}

#[allow(clippy::type_complexity)]
fn burn(
    mut commands: Commands,
    time: Res<Time>,
    mut bombs: Query<(Entity, &mut LitBomb, &Transform)>,
    mut fighters: Query<
        (Entity, &Team, &Transform, &mut Hp, &mut LinearVelocity, &mut Flash, Has<Carried>),
        (With<Fighter>, Without<LitBomb>),
    >,
    mut parts: Query<
        (Entity, &Transform, &mut DebrisPart, &mut LinearVelocity),
        (Without<Fighter>, Without<LitBomb>, Without<Carried>),
    >,
    mut out: BlastEffects,
) {
    for (bomb, mut lit, at) in &mut bombs {
        if !lit.fuse.tick(time.delta_secs()) {
            continue;
        }
        let centre = at.translation.truncate();
        commands.entity(bomb).despawn();
        out.exploded.write(Exploded { at: centre });
        out.stop.0.trigger(0.12);
        for (e, team, t, mut hp, mut v, mut flash, carried) in &mut fighters {
            let offset = t.translation.truncate() - centre;
            let (Some(hurt), Some((px, py))) = (damage(offset.length()), push((offset.x, offset.y))) else { continue };
            if hp.0.defeated() {
                continue;
            }
            flash.0 = FLASH;
            hp.0.stun = hp.0.stun.max(0.4);
            if !carried {
                v.0 = Vec2::new(px, py);
            }
            if hp.0.hurt(hurt) == HitOutcome::Defeated {
                out.defeated.write(Defeated { entity: e, team: *team });
            }
        }
        // Debris is knocked loose and flung, never deleted: the pile never loses progress outright.
        for (e, t, mut part, mut v) in &mut parts {
            let offset = t.translation.truncate() - centre;
            let Some((px, py)) = push((offset.x, offset.y)) else { continue };
            commands.entity(e).insert(RigidBody::Dynamic);
            part.born = time.elapsed_secs();
            v.0 = Vec2::new(px, py);
        }
    }
}
