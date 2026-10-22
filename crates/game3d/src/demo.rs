//! Demo mode (`/play/?demo=1`, or `DEMO=1` natively), driving the real game with the same `input::Actions` a player's
//! devices produce (a direction on the floor and the verbs pressed), so what it shows is what playing looks like. It goes straight into play (no start overlay), doesn't pause when the page loses
//! focus, and sends no enemies through the doors. By default it plays a scripted fight on a loop: fodder steps up to
//! the player, who punches, kicks, drop-kicks and spin-throws them (the throw into a second fodder further on), each
//! beat ending in a kill, so the box fills with debris. Uses: checking the look of a fight, recording the touch-device
//! clip (`tests/e2e/record-clip.mjs`), and loading the box to measure frame time.
//!
//! Knobs, from the page query (`?demo=1&slow=0.25`) or natively the environment (`DEMO=1 SLOW=0.25`):
//! - `slow=0.25`: slow motion (the game clock's speed), so short effects can be captured frame by frame.
//! - `debris=1000`: that many loose pieces across the floor at the start, to check the frame time of a full box.
//! - `enemy=thug|ninja`: who steps up each beat (fodder by default).
//! - `bombs=1`: they step up as bomb-heads (G7), as in the 2.5D demo.
//! - `timer=6`, `hp=20`: the box clock and the player's health at the start, to end a run on cue (e.g. to check the
//!   game-over screen and the best).
//! - `stairs=1`: no fight; each box gets a staircase of debris up past its rim and the player walks right, up it and
//!   out, box after box (the browser harness's scripted climb).
//! - `tour=1`: the clip on the start and pause screens. A mini box (box 1 with a low rim, [`TOUR_RIM`]) where the
//!   player runs round, punches, kicks, drop-kicks and spin-throws, heaps more kills in one spot, grabs a block and
//!   sets it on the heap, then climbs the heap and out. `rim=0.8` sets the first box's rim for any demo.
//!
//! One knob works without the demo too, for reviewing the look: `stage=1` starts the run's colours that many stages on
//! (`look`; box 1 in box 2's orange checker).

use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;
use game_core::climb::MAX_STEP;
use game_core::combat::Health;
use game_core::controls::Verbs;
use game_core::debris::solid::{FODDER, FODDER_HEIGHT, Solid};
use game_core::enemy::Kind;

use crate::arena::Arena;
use crate::bomb::BombHead;
use crate::carry::Carry;
use crate::debris::{ANY_HEIGHT, Piece, piece, pile_under};
use crate::enemy::{Brain, RADIUS, Waves, enemy};
use crate::fighter::Hp;
use crate::flow::{PauseOnBlur, Phase};
use crate::hint::GrabTip;
use crate::input::{Actions, ReadActions};
use crate::player::{Facing, Player, SPAWN};
use crate::run::{BoxEnded, BoxStarted, FOOTPRINT, RunState};
use game_core::run::{BoxSpec, Run, box_spec};

/// A demo setting from the page query (`demo=1&stairs=1`) or, natively, the environment (`DEMO=1 STAIRS=1`).
pub(crate) fn setting(key: &str) -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        let query = web_sys::window()?.location().search().ok()?;
        query.trim_start_matches('?').split('&').find_map(|kv| kv.strip_prefix(&format!("{key}="))).map(str::to_owned)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var(key.to_uppercase()).ok()
    }
}

/// Whether the page or environment asked for the demo.
pub fn requested() -> bool {
    setting("demo").as_deref() == Some("1")
}

/// The demo, if the page or environment asked for it; otherwise nothing.
pub fn plugin(app: &mut App) {
    if !requested() {
        return;
    }
    // After the screens' Startup, which puts a fresh page on the start overlay.
    app.insert_resource(PauseOnBlur(false))
        .add_systems(PostStartup, |mut next: ResMut<NextState<Phase>>| next.set(Phase::Playing))
        .add_plugins(Demo::page());
}

/// The demo's settings (see the module docs). Scenarios build one directly.
#[derive(Resource, Clone, Copy, Debug)]
pub struct Demo {
    pub stairs: bool,
    /// The game clock's speed.
    pub slow: f32,
    pub debris: usize,
    pub foe: Kind,
    /// The fight's enemies are bomb-heads.
    pub bombs: bool,
    pub timer: Option<f32>,
    pub hp: Option<f32>,
    /// The tour of every move, out of a mini box.
    pub tour: bool,
    /// The first box's rim, metres, if not its own.
    pub rim: Option<f32>,
}

impl Default for Demo {
    fn default() -> Self {
        Self {
            stairs: false,
            slow: 1.0,
            debris: 0,
            foe: Kind::Fodder,
            bombs: false,
            timer: None,
            hp: None,
            tour: false,
            rim: None,
        }
    }
}

impl Demo {
    /// The page's (or environment's) settings.
    pub fn page() -> Self {
        let number = |key| setting(key).and_then(|s| s.parse::<f32>().ok());
        let foe = match setting("enemy").as_deref() {
            Some("thug") => Kind::Thug,
            Some("ninja") => Kind::Ninja,
            _ => Kind::Fodder,
        };
        Self {
            stairs: setting("stairs").as_deref() == Some("1"),
            slow: number("slow").filter(|s| *s > 0.0).unwrap_or(1.0),
            debris: setting("debris").and_then(|s| s.parse().ok()).unwrap_or(0),
            foe,
            bombs: setting("bombs").as_deref() == Some("1"),
            timer: number("timer"),
            hp: number("hp"),
            tour: setting("tour").as_deref() == Some("1"),
            rim: number("rim").filter(|r| *r > 0.0),
        }
    }
}

impl Plugin for Demo {
    fn build(&self, app: &mut App) {
        let demo = *self;
        app.insert_resource(demo)
            .insert_resource(Waves { enabled: false, ..default() })
            .add_systems(Startup, (slow_motion, drop_debris))
            .add_systems(PostStartup, start_on_cue);
        // A first box of its own height: the run starts there, so the walls and the escape test both follow it.
        if let Some(rim) = demo.rim.or(demo.tour.then_some(TOUR_RIM)) {
            let spec = box_spec(1);
            app.insert_resource(RunState(Run { spec: BoxSpec { rim, ..spec }, time_left: spec.timer, elapsed: 0.0 }));
        }
        if demo.stairs {
            app.add_plugins(stairs);
        } else if demo.tour {
            app.add_systems(PreUpdate, tour.after(ReadActions).run_if(in_state(Phase::Playing)));
        } else {
            // Only while playing: after a death the script must not act on the game-over screen.
            app.add_systems(PreUpdate, fight.after(ReadActions).run_if(in_state(Phase::Playing)));
        }
    }
}

fn slow_motion(demo: Res<Demo>, mut time: ResMut<Time<Virtual>>, mut tip: ResMut<GrabTip>) {
    time.set_relative_speed(demo.slow);
    // The script knows the moves: no tip over the clip.
    tip.learned = true;
}

/// `timer` and `hp`: the box clock and the player's health to start with.
fn start_on_cue(demo: Res<Demo>, mut run: ResMut<RunState>, mut player: Query<&mut Hp, With<Player>>) {
    if let Some(t) = demo.timer {
        run.0.time_left = t;
    }
    if let (Some(h), Ok(mut p)) = (demo.hp, player.single_mut()) {
        p.0.hp = h;
    }
}

/// Grid spacing for `debris`, metres, and the clear patch round the player's spawn.
const DEBRIS_GRID: f32 = 0.5;
const DEBRIS_CLEAR: f32 = 0.8;

/// `debris`: that many fodder pieces in rows across the floor, resting on it (further layers on top, to fall onto
/// the first), leaving the player's spawn clear. They freeze as debris does.
fn drop_debris(mut commands: Commands, demo: Res<Demo>, arena: Res<Arena>, time: Res<Time>) {
    let half = arena.floor / 2.0 - Vec2::splat(0.4);
    let cells: Vec<Vec2> = (0..=(2.0 * half.y / DEBRIS_GRID) as usize)
        .flat_map(|row| {
            (0..=(2.0 * half.x / DEBRIS_GRID) as usize)
                .map(move |col| Vec2::new(-half.x + col as f32 * DEBRIS_GRID, -half.y + row as f32 * DEBRIS_GRID))
        })
        .filter(|c| c.distance(SPAWN.xz()) > DEBRIS_CLEAR)
        .collect();
    for i in 0..demo.debris {
        let (cell, layer) = (cells[i % cells.len()], (i / cells.len()) as f32);
        let solid = FODDER[i % FODDER.len()].solid;
        let half_height = match solid {
            Solid::Cube([_, y, _]) => y / 2.0,
            Solid::Ball(r) => r,
        };
        let at = Vec3::new(cell.x, layer * 0.6 + half_height + 0.01, cell.y);
        commands.spawn(piece(solid, i % FODDER.len(), at, Quat::IDENTITY, time.elapsed_secs()));
    }
}

/// A verb the script uses.
#[derive(Clone, Copy)]
enum Verb {
    Dash,
    Punch,
    Kick,
    Grab,
}

impl Verb {
    fn set(self, verbs: &mut Verbs) {
        match self {
            Verb::Dash => verbs.dash = true,
            Verb::Punch => verbs.punch = true,
            Verb::Kick => verbs.kick = true,
            Verb::Grab => verbs.grab = true,
        }
    }
}

/// One beat of the fight: what it is, which verbs (a dash or a grab first, then the attack: a drop-kick or a
/// spin-throw) and when, seconds into the beat (before the fodder's own punch comes out), how far in front of the
/// player its fodder stands, metres, and for the throw a second fodder further on for it to hit. The second verb
/// follows the first after `follow` seconds.
struct Beat {
    name: &'static str,
    verbs: &'static [Verb],
    strike: f32,
    follow: f32,
    gap: f32,
    beyond: Option<f32>,
}

const BEATS: [Beat; 4] = [
    Beat { name: "punch", verbs: &[Verb::Punch], strike: 0.5, follow: 0.0, gap: 1.1, beyond: None },
    Beat { name: "kick", verbs: &[Verb::Kick], strike: 0.35, follow: 0.0, gap: 1.3, beyond: None },
    Beat { name: "drop-kick", verbs: &[Verb::Dash, Verb::Punch], strike: 0.5, follow: 0.05, gap: 2.2, beyond: None },
    // Held up for a moment before the whirl, so the carry shows (Q77).
    Beat {
        name: "spin-throw",
        verbs: &[Verb::Grab, Verb::Punch],
        strike: 0.35,
        follow: 0.5,
        gap: 0.95,
        beyond: Some(2.2),
    },
];
/// Seconds per beat; the fodder is placed this far into it.
const BEAT: f32 = 1.4;
const PLACE: f32 = 0.2;
/// The room a beat needs in front of the player, metres: past it, the player turns round first.
const ROOM: f32 = 4.5;
/// Weak fodder, so every beat ends in a kill.
const DEMO_FODDER_HP: f32 = 8.0;

/// Where the fight script is: seconds into the beat, which beat, whether its fodder is placed, and which way (+1
/// right, -1 left) the player fights.
#[derive(Default)]
struct Script {
    t: f32,
    beat: usize,
    placed: bool,
    side: f32,
}

/// The fight goes on for as long as the page is open: unless `hp` or `timer` asked for a run that ends, the player's
/// health is topped up when it runs low (fodder land the odd punch), and so is the box clock.
const TOP_UP_HP: f32 = 0.4;
const TOP_UP_CLOCK: f32 = 10.0;

#[allow(clippy::too_many_arguments)]
fn fight(
    mut commands: Commands,
    time: Res<Time>,
    demo: Res<Demo>,
    arena: Res<Arena>,
    mut run: ResMut<RunState>,
    mut actions: ResMut<Actions>,
    mut player: Query<(&Transform, &Facing, &LinearVelocity, &mut Hp), With<Player>>,
    left_over: Query<Entity, With<Brain>>,
    pieces: Query<(&Piece, &Transform)>,
    mut script: Local<Script>,
) {
    // The script is the only input: in world terms (a direction on the floor), whatever the camera does.
    *actions = Actions::default();
    let Ok((at, facing, v, mut hp)) = player.single_mut() else { return };
    let s = &mut *script;
    let me = at.translation;
    let half = arena.floor / 2.0;
    if s.t == 0.0 && !s.placed {
        // Beats start once the player has landed, so the first fodder isn't placed in mid-air.
        if v.y.abs() > 0.01 {
            return;
        }
        // Fight toward whichever side has room, turning round (a step that way) first if need be.
        let room = |side: f32| half.x - side * me.x;
        if s.side == 0.0 || (room(s.side) < ROOM && room(-s.side) > room(s.side)) {
            s.side = if s.side == 0.0 { 1.0 } else { -s.side };
        }
        if facing.0.forward()[0] * s.side < 0.99 {
            actions.dir = [s.side, 0.0];
            return;
        }
    }
    s.t += time.delta_secs();
    let beat = &BEATS[s.beat % BEATS.len()];
    if !s.placed && s.t > PLACE {
        // A fresh start each beat: whoever is left goes, and the beat's fodder stands in front, facing the player.
        for e in &left_over {
            commands.entity(e).despawn();
        }
        place_beat(&mut commands, &demo, &arena, &pieces, beat, me, s.side);
        s.placed = true;
        if demo.hp.is_none() && hp.0.hp < TOP_UP_HP * hp.0.max {
            hp.0.hp = hp.0.max;
        }
        if demo.timer.is_none() && run.0.time_left < TOP_UP_CLOCK {
            run.0.time_left = run.0.spec.timer;
        }
    }
    let crossed = |mark: f32| s.t > mark && s.t - time.delta_secs() <= mark;
    if crossed(beat.strike) {
        info!("demo: {}", beat.name);
        beat.verbs[0].set(&mut actions.verbs);
    }
    if let Some(&then) = beat.verbs.get(1)
        && crossed(beat.strike + beat.follow)
    {
        then.set(&mut actions.verbs);
    }
    if s.t > BEAT {
        (s.t, s.placed) = (0.0, false);
        s.beat += 1;
    }
}

/// A beat's fodder in front of the player at `me`, toward `side` (+1 right, -1 left), facing them: on top of whatever
/// debris lies there, not inside it (which would fling it out), weak enough to die of one blow. The throw's target
/// stands dazed until the throw comes, rather than walking in to punch first.
fn place_beat(
    commands: &mut Commands,
    demo: &Demo,
    arena: &Arena,
    pieces: &Query<(&Piece, &Transform)>,
    beat: &Beat,
    me: Vec3,
    side: f32,
) {
    let inside = arena.floor / 2.0 - Vec2::splat(0.5);
    let yaw = if side > 0.0 { std::f32::consts::PI } else { 0.0 };
    for (i, gap) in std::iter::once(beat.gap).chain(beat.beyond).enumerate() {
        let spot = Vec2::new((me.x + side * gap).clamp(-inside.x, inside.x), me.z.clamp(-inside.y, inside.y));
        let ground = pile_under(pieces.iter(), spot, RADIUS + 0.15, ANY_HEIGHT);
        let at = Vec3::new(spot.x, ground + FODDER_HEIGHT / 2.0 + 0.05, spot.y);
        let mut health = Health::new(DEMO_FODDER_HP);
        if i > 0 {
            health.stun = 1.0;
        }
        let mut foe = commands.spawn(enemy(demo.foe, at, yaw));
        foe.insert(Hp(health));
        if demo.bombs {
            foe.insert(BombHead);
        }
    }
}

/// The tour's mini box: box 1's rim lowered to this, metres, so a few kills heaped in one spot reach it.
pub const TOUR_RIM: f32 = 0.7;
/// Extra punches into one spot after the four moves, to build the heap.
const HEAP_KILLS: usize = 3;
/// The climb's search grid spacing, metres, and how long it tries before heaping more.
const CLIMB_GRID: f32 = 0.2;
const CLIMB_FOR: f32 = 3.0;
/// Seconds of the run-round, then each leg of it (right, away, left, toward), then the turn back to face right.
const RUN_ROUND: f32 = 1.75;
const LEG: f32 = 0.4;

/// Where the tour is: its stage, seconds into it, and for the fights which beat and whether its fodder is placed.
#[derive(Default)]
struct Tour {
    stage: Stage,
    t: f32,
    beat: usize,
    placed: bool,
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
enum Stage {
    #[default]
    Land,
    RunRound,
    Moves,
    Heap,
    Stack,
    Climb,
    Out,
}

/// The tour (see the module docs): a timeline of stages, each driving the player through `Actions` as a player would.
#[allow(clippy::too_many_arguments)]
fn tour(
    mut commands: Commands,
    time: Res<Time>,
    demo: Res<Demo>,
    arena: Res<Arena>,
    mut actions: ResMut<Actions>,
    player: Query<(&Transform, &LinearVelocity, Has<Carry>), With<Player>>,
    left_over: Query<Entity, With<Brain>>,
    pieces: Query<(&Piece, &Transform)>,
    mut ended: MessageReader<BoxEnded>,
    mut s: Local<Tour>,
) {
    *actions = Actions::default();
    let Ok((at, v, carrying)) = player.single() else { return };
    let me = at.translation;
    let dt = time.delta_secs();
    let crossed = |t: f32, mark: f32| t > mark && t - dt <= mark;
    let next = |s: &mut Tour, stage: Stage| {
        info!("demo: {}", format!("{stage:?}").to_lowercase());
        (s.stage, s.t, s.beat, s.placed) = (stage, 0.0, 0, false);
    };
    if ended.read().any(|e| matches!(e, BoxEnded::Escaped { .. })) && s.stage != Stage::Out {
        info!("demo: escaped");
        next(&mut s, Stage::Out);
    }
    s.t += dt;
    match s.stage {
        Stage::Land => {
            if v.y.abs() < 0.01 && s.t > 0.3 {
                next(&mut s, Stage::RunRound);
            }
        }
        Stage::RunRound => {
            // Right, away from the camera, left, back toward it: all eight-way running needs, in four legs.
            let leg = (s.t / LEG) as usize;
            actions.dir = match leg {
                0 => [1.0, 0.0],
                1 => [0.0, 1.0],
                2 => [-1.0, 0.0],
                3 => [0.0, -1.0],
                _ => [1.0, 0.0],
            };
            if s.t > RUN_ROUND {
                next(&mut s, Stage::Moves);
            }
        }
        Stage::Moves | Stage::Heap => {
            let beat = if s.stage == Stage::Moves { &BEATS[s.beat] } else { &BEATS[0] };
            if !s.placed && s.t > PLACE {
                for e in &left_over {
                    commands.entity(e).despawn();
                }
                place_beat(&mut commands, &demo, &arena, &pieces, beat, me, 1.0);
                s.placed = true;
            }
            if crossed(s.t, beat.strike) {
                info!("demo: {}", beat.name);
                beat.verbs[0].set(&mut actions.verbs);
            }
            if let Some(&then) = beat.verbs.get(1)
                && crossed(s.t, beat.strike + beat.follow)
            {
                then.set(&mut actions.verbs);
            }
            if s.t > BEAT {
                let (count, after) =
                    if s.stage == Stage::Moves { (BEATS.len(), Stage::Heap) } else { (HEAP_KILLS, Stage::Stack) };
                (s.t, s.placed) = (0.0, false);
                s.beat += 1;
                if s.beat >= count {
                    next(&mut s, after);
                }
            }
        }
        Stage::Stack => {
            // Pick up a block off the front of the heap, hold it up, and set it down on top.
            if crossed(s.t, 0.15) {
                info!("demo: grab a block");
                actions.verbs.grab = true;
            }
            if s.t > 0.25 && s.t < 0.75 && carrying {
                actions.dir = [1.0, 0.0];
            }
            if crossed(s.t, 0.9) && carrying {
                info!("demo: set it on the heap");
                actions.verbs.grab = true;
            }
            if s.t > 1.3 {
                next(&mut s, Stage::Climb);
            }
        }
        Stage::Climb => {
            // Up the heap: walk at the highest place nearby a player could stand (the pile under a footprint, on a
            // grid round the player), not just the tallest piece, which may be a lone head with no room on top.
            let around = me.xz();
            let best = (-10..=10)
                .flat_map(|i| (-10..=10).map(move |j| around + Vec2::new(i as f32, j as f32) * CLIMB_GRID))
                .map(|spot| (pile_under(pieces.iter(), spot, FOOTPRINT, ANY_HEIGHT), spot))
                .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.distance(around).total_cmp(&a.1.distance(around))));
            if let Some((_, spot)) = best {
                let to = spot - around;
                if to.length() > 0.08 {
                    let d = to.normalize();
                    actions.dir = [d.x, -d.y];
                }
            }
            // Not out yet: the heap is short. Heap more kills, stack, and try again.
            if s.t > CLIMB_FOR {
                next(&mut s, Stage::Heap);
            }
        }
        Stage::Out => {}
    }
}

/// The stairs run: no enemies, a staircase in each box, and the player walking right.
pub fn stairs(app: &mut App) {
    app.insert_resource(Waves { enabled: false, ..default() })
        .add_systems(Update, build_stairs.in_set(crate::Frame::Show))
        .add_systems(PreUpdate, walk_right.after(ReadActions));
}

/// Each step rises this much (under `MAX_STEP`), metres.
const RISE: f32 = 0.75 * MAX_STEP;
/// Each step's depth along x and width along z, metres.
const TREAD: f32 = 0.55;
const WIDTH: f32 = 1.2;
/// Where the first step starts, right of the spawn, metres.
const FIRST: f32 = 0.8;

/// A staircase of frozen debris to the right of the spawn, up to the rim: blocks from the floor, each a step taller
/// than the last, the top one level with or above the rim.
fn build_stairs(mut commands: Commands, time: Res<Time>, mut started: MessageReader<BoxStarted>) {
    let Some(BoxStarted(spec)) = started.read().last() else { return };
    let steps = (spec.rim / RISE).ceil() as usize;
    for i in 0..steps {
        let h = RISE * (i + 1) as f32;
        let at = Vec3::new(FIRST + TREAD * (i as f32 + 0.5), h / 2.0, SPAWN.z);
        let block = piece(Solid::Cube([TREAD, h, WIDTH]), 0, at, Quat::IDENTITY, time.elapsed_secs());
        commands.spawn(block).insert(avian3d::prelude::RigidBody::Static);
    }
}

fn walk_right(mut actions: ResMut<Actions>) {
    *actions = Actions { dir: [1.0, 0.0], ..default() };
}
