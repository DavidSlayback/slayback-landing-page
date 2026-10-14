//! Demo mode (`/play/?demo=1`, or `DEMO=1` natively), driving the real game with synthetic key presses, so what it
//! shows is what playing looks like. It goes straight into play (no start overlay), doesn't pause when the page loses
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

use avian3d::prelude::LinearVelocity;
use bevy::input::InputSystems;
use bevy::prelude::*;
use game_core::climb::{MAX_STEP, Placed, pile_height};
use game_core::combat::Health;
use game_core::debris::solid::{FODDER, FODDER_HEIGHT, Solid};
use game_core::enemy::Kind;

use crate::arena::Arena;
use crate::bomb::BombHead;
use crate::debris::{Piece, piece};
use crate::enemy::{Brain, RADIUS, Waves, enemy};
use crate::fighter::Hp;
use crate::flow::{PauseOnBlur, Phase};
use crate::input::{GRAB_KEY, KICK_KEY, PUNCH_KEY, ReadActions, WorldControls};
use crate::player::{Facing, Player, SPAWN};
use crate::run::{BoxStarted, RunState};

/// A demo setting from the page query (`demo=1&stairs=1`) or, natively, the environment (`DEMO=1 STAIRS=1`).
fn setting(key: &str) -> Option<String> {
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
    // Its key presses mean world directions (D is +x), not the camera's.
    app.insert_resource(PauseOnBlur(false))
        .init_resource::<WorldControls>()
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
}

impl Default for Demo {
    fn default() -> Self {
        Self { stairs: false, slow: 1.0, debris: 0, foe: Kind::Fodder, bombs: false, timer: None, hp: None }
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
        if demo.stairs {
            app.add_plugins(stairs);
        } else {
            // Only while playing: after a death the script must not press "again" (Space is also the dash).
            app.add_systems(PreUpdate, fight.after(InputSystems).before(ReadActions).run_if(in_state(Phase::Playing)));
        }
    }
}

fn slow_motion(demo: Res<Demo>, mut time: ResMut<Time<Virtual>>) {
    time.set_relative_speed(demo.slow);
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

/// One beat of the fight: what it is, what to press (a dash or a grab first, then the attack: a drop-kick or a
/// spin-throw) and when, seconds into the beat (before the fodder's own punch comes out), how far in front of the
/// player its fodder stands, metres, and for the throw a second fodder further on for it to hit.
struct Beat {
    name: &'static str,
    keys: &'static [KeyCode],
    strike: f32,
    gap: f32,
    beyond: Option<f32>,
}

const BEATS: [Beat; 4] = [
    Beat { name: "punch", keys: &[PUNCH_KEY], strike: 0.5, gap: 1.1, beyond: None },
    Beat { name: "kick", keys: &[KICK_KEY], strike: 0.35, gap: 1.3, beyond: None },
    Beat { name: "drop-kick", keys: &[KeyCode::Space, PUNCH_KEY], strike: 0.5, gap: 2.2, beyond: None },
    Beat { name: "spin-throw", keys: &[GRAB_KEY, PUNCH_KEY], strike: 0.35, gap: 0.95, beyond: Some(2.2) },
];
/// Seconds per beat; the fodder is placed this far into it.
const BEAT: f32 = 1.4;
const PLACE: f32 = 0.2;
/// The second key of a beat follows the first after this, seconds.
const FOLLOW: f32 = 0.05;
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
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut player: Query<(&Transform, &Facing, &LinearVelocity, &mut Hp), With<Player>>,
    left_over: Query<Entity, With<Brain>>,
    pieces: Query<(&Piece, &Transform)>,
    mut script: Local<Script>,
) {
    keys.release_all();
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
            keys.press(if s.side > 0.0 { KeyCode::KeyD } else { KeyCode::KeyA });
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
        let inside = half - Vec2::splat(0.5);
        let yaw = if s.side > 0.0 { std::f32::consts::PI } else { 0.0 };
        for (i, gap) in std::iter::once(beat.gap).chain(beat.beyond).enumerate() {
            let spot = Vec2::new((me.x + s.side * gap).clamp(-inside.x, inside.x), me.z.clamp(-inside.y, inside.y));
            // On top of whatever debris lies there, not inside it (which would fling it out).
            let under: Vec<Placed> = pieces
                .iter()
                .filter(|(_, t)| t.translation.xz().distance(spot) < 1.5)
                .map(|(p, t)| Placed {
                    solid: p.solid,
                    centre: t.translation.to_array(),
                    rotation: t.rotation.to_array(),
                })
                .collect();
            let ground = pile_height(&under, spot.to_array(), RADIUS + 0.15, f32::MAX);
            let at = Vec3::new(spot.x, ground + FODDER_HEIGHT / 2.0 + 0.05, spot.y);
            let mut health = Health::new(DEMO_FODDER_HP);
            // The throw's target stands dazed until the throw comes, rather than walking in to punch first.
            if i > 0 {
                health.stun = 1.0;
            }
            let mut foe = commands.spawn(enemy(demo.foe, at, yaw));
            foe.insert(Hp(health));
            if demo.bombs {
                foe.insert(BombHead);
            }
        }
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
        keys.press(beat.keys[0]);
    }
    if let Some(&then) = beat.keys.get(1)
        && crossed(beat.strike + FOLLOW)
    {
        keys.press(then);
    }
    if s.t > BEAT {
        (s.t, s.placed) = (0.0, false);
        s.beat += 1;
    }
}

/// The stairs run: no enemies, a staircase in each box, and the player walking right.
pub fn stairs(app: &mut App) {
    app.insert_resource(Waves { enabled: false, ..default() })
        .add_systems(Update, build_stairs)
        .add_systems(PreUpdate, walk_right.after(InputSystems).before(ReadActions));
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

fn walk_right(mut keys: ResMut<ButtonInput<KeyCode>>) {
    keys.release_all();
    keys.press(KeyCode::KeyD);
}
