//! The run loop: box after box until the player dies (rules: `game_core::run`, as in the 2.5D game). This module keeps
//! the clock, checks the escape test, and empties the box between boxes and on death; `arena` rebuilds the box at the
//! next rim and `score` keeps or resets the score.
//!
//! Escape (Q51): the pile under the player has reached the rim. The pile's height under the player's footprint comes
//! from the debris pieces as data (`game_core::climb::pile_height`), so only debris counts: not the floor, the walls
//! or anything above the player's feet.

use avian3d::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use game_core::climb::{Placed, pile_height};
use game_core::combat::{Health, HitOutcome};
use game_core::doors::Door;
use game_core::run::{BoxSpec, Run, TIMEOUT_PENALTY, Tick, escaped};

use crate::arena::Arena;
use crate::debris::Piece;
use crate::enemy::{Brain, WallDoor, Waves};
use crate::fighter::{Combat, Defeated, Hp, Team};
use crate::player::{HEIGHT, MAX_HP, Player, SPAWN};

/// The run in progress.
#[derive(Resource, Default)]
pub struct RunState(pub Run);

/// A box begins: the first one, the next one after an escape, or box 1 again after a death.
#[derive(Message, Clone, Copy, Debug)]
pub struct BoxStarted(pub BoxSpec);

/// How a box ended, for the score (and later the screens).
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub enum BoxEnded {
    /// Escaped box `number` with `time_left` on the clock.
    Escaped { number: usize, time_left: f32 },
    /// Died in box `reached`.
    Died { reached: usize },
}

/// Radius of the footprint the escape test measures the pile under, metres: most of the player's capsule.
pub const FOOTPRINT: f32 = 0.25;
/// A pile top this far above the feet still counts as under them (the controller keeps a skin's gap, and the feet
/// may rest on an edge below the footprint's highest point).
const FEET: f32 = 0.08;
/// Pieces centred further than this across the floor from the player can't be under its footprint, metres: the
/// footprint plus the reach of the largest piece (a demo stair, about 0.7 m from its centre), with room to spare.
const NEAR: f32 = 1.5;

pub fn plugin(app: &mut App) {
    app.init_resource::<RunState>()
        .add_message::<BoxStarted>()
        .add_message::<BoxEnded>()
        .add_systems(Startup, |run: Res<RunState>, mut started: MessageWriter<BoxStarted>| {
            started.write(BoxStarted(run.0.spec));
        })
        .add_systems(Update, ((clock, escape, die).chain().after(Combat), announce));
}

/// The box clock: running out costs health (decision Q27) and restarts it, which can end the run.
fn clock(
    time: Res<Time>,
    mut run: ResMut<RunState>,
    mut players: Query<(Entity, &Transform, &mut Hp), With<Player>>,
    mut defeated: MessageWriter<Defeated>,
) {
    if run.0.tick(time.delta_secs()) == Tick::TimedOut
        && let Ok((entity, tf, mut hp)) = players.single_mut()
        && hp.0.hurt(TIMEOUT_PENALTY) == HitOutcome::Defeated
    {
        defeated.write(Defeated { entity, team: Team::Player, at: tf.translation });
    }
}

/// What one box holds that the next doesn't: enemies and debris.
type BoxContents = Or<(With<Brain>, With<Piece>)>;
/// The player's parts a box's end touches.
type PlayerParts = (&'static mut Transform, &'static mut Position, &'static mut LinearVelocity, &'static mut Hp);

/// Ending a box: empty it (enemies, debris, doors about to open), put the player back at the start, and announce
/// the next box.
#[derive(SystemParam)]
struct BoxReset<'w, 's> {
    commands: Commands<'w, 's>,
    contents: Query<'w, 's, Entity, BoxContents>,
    doors: Query<'w, 's, &'static mut WallDoor>,
    player: Query<'w, 's, PlayerParts, With<Player>>,
    started: MessageWriter<'w, BoxStarted>,
}

impl BoxReset<'_, '_> {
    fn apply(&mut self, next: BoxSpec) {
        for e in &self.contents {
            self.commands.entity(e).try_despawn();
        }
        for mut d in &mut self.doors {
            d.door = Door::Closed;
        }
        if let Ok((mut tf, mut position, mut velocity, _)) = self.player.single_mut() {
            // Both: the controller moves `Position`, and a changed `Transform` resets the easing (a teleport).
            tf.translation = SPAWN;
            position.0 = SPAWN;
            velocity.0 = Vec3::ZERO;
        }
        self.started.write(BoxStarted(next));
    }
}

/// The pile under the player has reached the rim: on to the next box.
fn escape(
    mut run: ResMut<RunState>,
    arena: Res<Arena>,
    pieces: Query<(&Piece, &Transform), Without<Player>>,
    mut reset: BoxReset,
    mut ended: MessageWriter<BoxEnded>,
) {
    let Ok((_, &Position(at), _, hp)) = reset.player.single() else { return };
    let hp = hp.0;
    let under: Vec<Placed> = pieces
        .iter()
        .filter(|(_, t)| t.translation.xz().distance(at.xz()) < NEAR)
        .map(|(p, t)| Placed { solid: p.solid, centre: t.translation.to_array(), rotation: t.rotation.to_array() })
        .collect();
    let feet = at.y - HEIGHT / 2.0;
    if !escaped(pile_height(&under, [at.x, at.z], FOOTPRINT, feet + FEET), arena.rim) {
        return;
    }
    ended.write(BoxEnded::Escaped { number: run.0.spec.number, time_left: run.0.time_left });
    let refilled = run.0.escape(hp.hp, hp.max);
    if let Ok((.., mut hp)) = reset.player.single_mut() {
        hp.0.hp = refilled;
    }
    reset.apply(run.0.spec);
}

/// The player's health ran out (a blow or the clock): the run ends. Until the screens (G8), a fresh one starts at
/// box 1 with full health.
fn die(
    mut defeats: MessageReader<Defeated>,
    mut run: ResMut<RunState>,
    mut reset: BoxReset,
    mut ended: MessageWriter<BoxEnded>,
) {
    if defeats.read().filter(|d| d.team == Team::Player).count() == 0 {
        return;
    }
    ended.write(BoxEnded::Died { reached: run.0.spec.number });
    run.0.restart();
    if let Ok((.., mut hp)) = reset.player.single_mut() {
        hp.0 = Health::new(MAX_HP);
    }
    reset.apply(run.0.spec);
}

/// Each box sets the crowd at the doors, and is logged: the browser harness's scripted run waits for "box 2 started".
fn announce(mut started: MessageReader<BoxStarted>, mut waves: ResMut<Waves>) {
    for BoxStarted(spec) in started.read() {
        info!(
            "box {} started (rim {:.2} m, {} s, crowd {})",
            spec.number,
            game_core::run::rim_metres(spec),
            spec.timer,
            spec.crowd
        );
        waves.crowd = spec.crowd;
        waves.timer.reset();
    }
}
