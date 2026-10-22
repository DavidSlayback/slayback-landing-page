//! The run loop: box after box until the player dies (rules: `game_core::run`, as in the 2.5D game). This module keeps
//! the clock, checks the escape test, empties the box between boxes, and on death shows the game-over screen; leaving
//! it starts a fresh run at box 1. `arena` rebuilds the box at each rim and `score` keeps the score and the best.
//!
//! Escape (Q51, Q81): the pile under the player has reached the rim, with the player against a wall. The pile's height under the player's footprint comes
//! from the debris pieces as data (`game_core::climb::pile_height`), so only debris counts: not the floor, the walls
//! or anything above the player's feet.

use avian3d::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use game_core::combat::{Health, HitOutcome};
use game_core::doors::Door;
use game_core::run::{AT_WALL, BoxSpec, Run, TIMEOUT_PENALTY, Tick, escaped};

use crate::Frame;
use crate::arena::Arena;
use crate::debris::{ANY_HEIGHT, Piece, pile_under};
use crate::enemy::{WallDoor, Waves};
use crate::fighter::{Combat, Defeated, Hp, Team};
use crate::flow::Phase;
use crate::player::{HEIGHT, MAX_HP, Player, SPAWN};

/// The run in progress.
#[derive(Resource, Default)]
pub struct RunState(pub Run);

/// How the climb out stands, for the HUD's climb gauge (playtest feedback, Q80): the pile under the player's feet, the
/// tallest settled heap against a wall, and the rim they must reach, all metres above the floor; and whether the
/// player stands at a wall, where a pile at the rim gets them out (Q81).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct Climb {
    pub under: f32,
    pub tallest: f32,
    pub rim: f32,
    pub at_wall: bool,
}

/// How far `spot` (floor x and z) is from the nearest wall's inner face, metres.
pub fn from_wall(arena: &Arena, spot: Vec2) -> f32 {
    let half = arena.floor / 2.0;
    (half.x - spot.x.abs()).min(half.y - spot.y.abs())
}

/// A box begins: the first one, the next one after an escape, or box 1 again after a death.
#[derive(Message, Clone, Copy, Debug)]
pub struct BoxStarted(pub BoxSpec);

/// How a box ended, for the score and the screens.
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

/// Belongs to the box it's in, and goes when the box ends: every enemy, debris piece and lit bomb carries it from its
/// bundle (`enemy::enemy`, `debris::piece`, `bomb::lit_bomb`), so a new kind of thing in the box only needs it there.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct InBox;

pub fn plugin(app: &mut App) {
    app.init_resource::<RunState>()
        .init_resource::<Climb>()
        .add_message::<BoxStarted>()
        .add_message::<BoxEnded>()
        .add_systems(Startup, |run: Res<RunState>, mut started: MessageWriter<BoxStarted>| {
            started.write(BoxStarted(run.0.spec));
        })
        .add_systems(Update, (clock, escape, die).chain().in_set(Frame::Resolve).after(Combat))
        .add_systems(Update, (announce, measure).in_set(Frame::Show))
        // Leaving the game-over screen starts a fresh run.
        .add_systems(OnExit(Phase::Over), restart);
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

/// The player's parts a box's end touches.
type PlayerParts = (&'static mut Transform, &'static mut Position, &'static mut LinearVelocity, &'static mut Hp);

/// Ending a box: empty it (enemies, debris, doors about to open), put the player back at the start, and announce
/// the next box.
#[derive(SystemParam)]
struct BoxReset<'w, 's> {
    commands: Commands<'w, 's>,
    contents: Query<'w, 's, Entity, With<InBox>>,
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
    if !escaped(standing_on(pieces.iter(), at), arena.rim, from_wall(&arena, at.xz())) {
        return;
    }
    ended.write(BoxEnded::Escaped { number: run.0.spec.number, time_left: run.0.time_left });
    let refilled = run.0.escape(hp.hp, hp.max);
    if let Ok((.., mut hp)) = reset.player.single_mut() {
        hp.0.hp = refilled;
    }
    reset.apply(run.0.spec);
}

/// How high the pile under a player whose capsule is centred at `at` stands, metres above the floor: what the escape
/// test measures (agents see it too, `observe`).
pub fn standing_on<'a>(pieces: impl IntoIterator<Item = (&'a Piece, &'a Transform)>, at: Vec3) -> f32 {
    let feet = at.y - HEIGHT / 2.0;
    pile_under(pieces, at.xz(), FOOTPRINT, feet + FEET)
}

/// The climb gauge's numbers. The tallest heap counts only frozen debris against a wall, so a piece flung through the
/// air doesn't flash a peak and a heap out in the middle doesn't promise a way out; it's each piece's top at its centre
/// (`pile_under` over that one piece).
fn measure(
    arena: Res<Arena>,
    pieces: Query<(&Piece, &Transform, &RigidBody), Without<Player>>,
    player: Query<&Position, With<Player>>,
    mut climb: ResMut<Climb>,
) {
    let (under, at_wall) = player.single().map_or((0.0, false), |p| {
        (standing_on(pieces.iter().map(|(p, t, _)| (p, t)), p.0), from_wall(&arena, p.0.xz()) <= AT_WALL)
    });
    let tallest = pieces
        .iter()
        .filter(|(_, t, body)| **body == RigidBody::Static && from_wall(&arena, t.translation.xz()) <= AT_WALL)
        .map(|(p, t, _)| pile_under(std::iter::once((p, t)), t.translation.xz(), 0.05, ANY_HEIGHT))
        .fold(if at_wall { under } else { 0.0 }, f32::max);
    let next = Climb { under, tallest, rim: arena.rim, at_wall };
    if *climb != next {
        *climb = next;
    }
}

/// The player's health ran out (a blow or the clock): the run ends on the game-over screen.
fn die(
    mut defeats: MessageReader<Defeated>,
    run: Res<RunState>,
    mut ended: MessageWriter<BoxEnded>,
    mut next: ResMut<NextState<Phase>>,
) {
    if defeats.read().any(|d| d.team == Team::Player) {
        ended.write(BoxEnded::Died { reached: run.0.spec.number });
        next.set(Phase::Over);
    }
}

/// Again: box 1, full health, an empty box.
fn restart(mut run: ResMut<RunState>, mut reset: BoxReset) {
    run.0.restart();
    if let Ok((.., mut hp)) = reset.player.single_mut() {
        hp.0 = Health::new(MAX_HP);
    }
    reset.apply(run.0.spec);
}

/// Each box sets how often the doors are called (the crowd itself comes from `RunState`), and is logged: the browser
/// harness's scripted run waits for "box 2 started".
fn announce(mut started: MessageReader<BoxStarted>, mut waves: ResMut<Waves>) {
    for BoxStarted(spec) in started.read() {
        info!(
            "box {} started (rim {:.2} m, {} s, crowd {} to {}, doors every {:.1} s, {} at once)",
            spec.number, spec.rim, spec.timer, spec.crowd, spec.peak, spec.interval, spec.burst
        );
        waves.timer = Timer::from_seconds(spec.interval, TimerMode::Repeating);
    }
}
