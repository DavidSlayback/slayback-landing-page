//! The run loop: box after box until the player dies. Rules live in `game_core::run`; this module keeps the clock,
//! checks the escape test, and clears and rebuilds the box between boxes and on death.

use avian2d::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use game_core::combat::{Health, HitOutcome};
use game_core::run::{BoxSpec, Run, TIMEOUT_PENALTY, Tick, escaped};

use crate::ARENA;
use crate::arena::WALL;
use crate::debris::DebrisPart;
use crate::enemy::{Brain, WallDoor, Waves};
use crate::fighter::{Defeated, Fighter, Hp, Team};
use crate::player::{MAX_HP, Player, SPAWN};
use game_core::doors::Door;

/// The run in progress.
#[derive(Resource, Default)]
pub struct RunState(pub Run);

/// A box begins (the first one, the next one after an escape, or box 1 again after a death).
#[derive(Message, Clone, Copy, Debug)]
pub struct BoxStarted(pub BoxSpec);

/// How the last box ended, for the screens (D5).
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub enum BoxEnded {
    Escaped,
    Died,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<RunState>()
        .add_message::<BoxStarted>()
        .add_message::<BoxEnded>()
        .add_systems(Startup, |run: Res<RunState>, mut started: MessageWriter<BoxStarted>| {
            started.write(BoxStarted(run.0.spec));
        })
        .add_systems(Update, (clock, escape, die).chain())
        .add_systems(Update, apply_box_to_waves);
}

/// Floor surface height, world units.
pub fn floor_top() -> f32 {
    -ARENA.y / 2.0 + WALL
}

fn clock(time: Res<Time>, mut run: ResMut<RunState>, mut players: Query<&mut Hp, With<Player>>, mut defeated: MessageWriter<Defeated>, player: Query<Entity, With<Player>>) {
    if run.0.tick(time.delta_secs()) == Tick::TimedOut
        && let Ok(mut hp) = players.single_mut()
        && hp.0.hurt(TIMEOUT_PENALTY) == HitOutcome::Defeated
        && let Ok(entity) = player.single()
    {
        defeated.write(Defeated { entity, team: Team::Player });
    }
}

/// What a box holds that the next one doesn't: enemies and debris.
type BoxContents = Or<(With<Brain>, With<DebrisPart>)>;

/// Ending a box: empty it (enemies, debris, pending door entries), put the player back at the start, announce it.
#[derive(SystemParam)]
struct BoxReset<'w, 's> {
    commands: Commands<'w, 's>,
    clear: Query<'w, 's, Entity, BoxContents>,
    doors: Query<'w, 's, &'static mut WallDoor>,
    started: MessageWriter<'w, BoxStarted>,
    ended: MessageWriter<'w, BoxEnded>,
}

impl BoxReset<'_, '_> {
    fn apply(&mut self, how: BoxEnded, next: BoxSpec, transform: &mut Transform, velocity: &mut LinearVelocity) {
        for e in &self.clear {
            self.commands.entity(e).despawn();
        }
        for mut d in &mut self.doors {
            d.door = Door::Closed;
        }
        transform.translation = SPAWN.extend(transform.translation.z);
        velocity.0 = Vec2::ZERO;
        self.ended.write(how);
        self.started.write(BoxStarted(next));
    }
}

/// The player stands on the pile at the rim's height: on to the next box.
fn escape(
    mut run: ResMut<RunState>,
    mut player: Query<(&mut Transform, &mut LinearVelocity, &mut Hp, &ShapeHits, &Fighter), With<Player>>,
    mut reset: BoxReset,
) {
    let Ok((mut transform, mut velocity, mut hp, ground, fighter)) = player.single_mut() else { return };
    let standing = transform.translation.y - fighter.half.y - floor_top();
    if ground.is_empty() || !escaped(standing, run.0.spec.rim) {
        return;
    }
    hp.0.hp = run.0.escape(hp.0.hp, hp.0.max);
    reset.apply(BoxEnded::Escaped, run.0.spec, &mut transform, &mut velocity);
}

/// The player's health ran out (a blow or a timeout): back to box 1 at full health.
fn die(
    mut defeats: MessageReader<Defeated>,
    mut run: ResMut<RunState>,
    mut player: Query<(&mut Transform, &mut LinearVelocity, &mut Hp), With<Player>>,
    mut reset: BoxReset,
) {
    if !defeats.read().any(|d| d.team == Team::Player) {
        return;
    }
    let Ok((mut transform, mut velocity, mut hp)) = player.single_mut() else { return };
    run.0.restart();
    hp.0 = Health::new(MAX_HP);
    reset.apply(BoxEnded::Died, run.0.spec, &mut transform, &mut velocity);
}

fn apply_box_to_waves(mut started: MessageReader<BoxStarted>, mut waves: ResMut<Waves>) {
    for BoxStarted(spec) in started.read() {
        waves.crowd = spec.crowd;
        waves.timer.reset();
    }
}
