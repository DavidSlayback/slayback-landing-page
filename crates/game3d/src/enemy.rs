//! Enemies enter through doors in the back and side walls (none in the front, Q58), which warn first; then they steer
//! toward the player while keeping apart and off the walls (Q63), telegraph and strike. The box keeps its crowd topped
//! up, calling at several doors at once (`game_core::run::BoxSpec`: crowd ramp, interval, burst), at Rumble Box's pace.
//!
//! Decisions come from `game_core::enemy::Mind` (the same brains and timings as the 2.5D game), steering from
//! `game_core::steer`, door timing from `game_core::doors` and the mix of kinds from the box
//! (`game_core::run::pick_kind`); this module turns them into entities, velocity, facing and swings. Fodder seek
//! and keep apart; thugs plough straight in, ignoring the crowd; a ninja the player faces circles round to its back.

use avian3d::prelude::*;
use bevy::prelude::*;
use game_core::doors::Door;
use game_core::enemy::{FODDER, Intent, Kind, Mind, NinjaPhase};
use game_core::grab::Held;
use game_core::plane::{RUN_ACCEL, RUN_SPEED, approach, heading};
use game_core::run::{box_spec, pick_kind};
use game_core::steer::{circle, steer};

use crate::Frame;
use crate::arena::Arena;
use crate::bomb::BombHead;
use crate::carry::{Carried, Grabbable};
use crate::fighter::{Attack, Hp, Team, body};
use crate::player::{Facing, Player};
use crate::run::{InBox, RunState};
use crate::{on_floor, plane};

#[derive(Component)]
pub struct Brain(pub Mind);

/// The fodder's capsule radius, metres: what the player's aim assist and the demo's piles take an enemy's to be.
/// Each kind's own is its `game_core::enemy::Style`'s.
pub const RADIUS: f32 = FODDER.radius;
/// The player counts as in front of an enemy when its facing is within this angle of the enemy (cos 60°).
const IN_FRONT: f32 = 0.5;

/// A door in a wall: where its enemy appears and which way it faces on entering. `next` is who it's warning about.
#[derive(Component)]
pub struct WallDoor {
    pub door: Door,
    /// The door's foot on the wall's inner face, and the floor direction into the box.
    pub face: Vec3,
    pub inward: Vec3,
    pub next: Kind,
    /// Whether that enemy carries a bomb for a head.
    pub bomb: bool,
}

impl WallDoor {
    /// Where an enemy of `kind` stands as it steps in: just inside the wall, on the floor.
    pub fn entry(&self, kind: Kind) -> Vec3 {
        let s = kind.style();
        self.face + self.inward * (s.radius + 0.2) + Vec3::Y * (s.height / 2.0 + 0.05)
    }
}

/// Enemy entries. Tests turn them off to place enemies by hand.
#[derive(Resource)]
pub struct Waves {
    pub enabled: bool,
    /// Each time it finishes, while the box holds fewer than its crowd (`Run::crowd`, counting enemies on their way in
    /// through a door), call at up to the box's `burst` free doors. Each box sets it to its spec's `interval`.
    pub timer: Timer,
    /// The share of entries that are bomb-heads, overriding the box spec's (the demo's `bombs=1` makes it 1).
    pub bombs: Option<f32>,
    /// The state of the dice that pick each entry's kind and bomb: a run is the same for the same seed and the same
    /// play (the RL environment varies it per episode).
    pub seed: u32,
}

impl Default for Waves {
    fn default() -> Self {
        let every = box_spec(1).interval;
        Self { enabled: true, timer: Timer::from_seconds(every, TimerMode::Repeating), bombs: None, seed: 0 }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Waves>()
        .add_systems(Startup, spawn_doors)
        .add_systems(Update, (request_entries, open_doors, think).in_set(Frame::Decide));
    #[cfg(feature = "render")]
    app.add_plugins(view::plugin);
}

/// The doors' feet on the walls' inner faces and their inward directions: two in the back wall, one in each side.
pub fn door_spots(arena: &Arena) -> [(Vec3, Vec3); 4] {
    let half = arena.floor / 2.0;
    let back = -half.y;
    [
        (Vec3::new(-half.x / 2.0, 0.0, back), Vec3::Z),
        (Vec3::new(half.x / 2.0, 0.0, back), Vec3::Z),
        (Vec3::new(-half.x, 0.0, -half.y / 4.0), Vec3::X),
        (Vec3::new(half.x, 0.0, -half.y / 4.0), Vec3::NEG_X),
    ]
}

fn spawn_doors(mut commands: Commands, arena: Res<Arena>) {
    for (face, inward) in door_spots(&arena) {
        commands.spawn((
            Name::new("door"),
            WallDoor { door: Door::Closed, face, inward, next: Kind::Fodder, bomb: false },
            Transform::from_translation(face),
        ));
    }
}

/// An enemy of `kind` standing at `at`, facing `yaw`.
pub fn enemy(kind: Kind, at: Vec3, yaw: f32) -> impl Bundle {
    let s = kind.style();
    let mut facing = game_core::plane::Facing::default();
    facing.snap(yaw);
    (
        Brain(Mind::new(kind)),
        InBox,
        Grabbable(Held::Enemy),
        Name::new(s.name),
        body(Team::Enemy, s.radius, s.height, s.max_hp),
        Transform::from_translation(at),
        Facing(facing),
    )
}

/// A fodder standing at `at`, facing `yaw`.
pub fn fodder(at: Vec3, yaw: f32) -> impl Bundle {
    enemy(Kind::Fodder, at, yaw)
}

fn request_entries(
    time: Res<Time>,
    run: Res<RunState>,
    mut waves: ResMut<Waves>,
    enemies: Query<(), With<Brain>>,
    mut doors: Query<&mut WallDoor>,
    mut turn: Local<usize>,
) {
    if !waves.enabled || !waves.timer.tick(time.delta()).just_finished() {
        return;
    }
    let pending = doors.iter().filter(|d| d.door != Door::Closed).count();
    let short = run.0.crowd().saturating_sub(enemies.iter().count() + pending);
    // Round the doors in turn, skipping busy ones.
    let mut free: Vec<_> = doors.iter_mut().filter(|d| d.door == Door::Closed).collect();
    let calls = short.min(run.0.spec.burst).min(free.len());
    if calls == 0 {
        return;
    }
    *turn += 1;
    let first = *turn % free.len();
    for k in 0..calls {
        let len = free.len();
        let door = &mut free[(first + k) % len];
        // xorshift (as in the 2.5D game): the box's mix of kinds and share of bomb-heads without a rand dependency.
        waves.seed = waves.seed.wrapping_add(0x9E37_79B9);
        let mut s = waves.seed;
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        door.next = pick_kind(&run.0.spec, run.0.elapsed, (s & 0xFFFF) as f32 / 65536.0);
        door.bomb = ((s >> 16) as f32 / 65536.0) < waves.bombs.unwrap_or(run.0.spec.bombs);
        door.door.request();
    }
}

fn open_doors(time: Res<Time>, mut commands: Commands, mut doors: Query<&mut WallDoor>) {
    for mut d in &mut doors {
        if d.door.tick(time.delta_secs()) {
            let yaw = heading(plane(d.inward)).unwrap_or(0.0);
            let mut e = commands.spawn(enemy(d.next, d.entry(d.next), yaw));
            if d.bomb {
                e.insert(BombHead);
            }
        }
    }
}

#[allow(clippy::type_complexity)]
fn think(
    time: Res<Time>,
    arena: Res<Arena>,
    player: Query<(&Transform, &Facing), (With<Player>, Without<Brain>)>,
    mut enemies: Query<
        (Entity, &Transform, &mut Brain, &mut Facing, &mut Attack, &Hp, &mut LinearVelocity),
        Without<Carried>,
    >,
) {
    let Ok((target, them)) = player.single() else { return };
    let dt = time.delta_secs();
    let goal = plane(target.translation);
    let crowd: Vec<(Entity, [f32; 2])> = enemies.iter().map(|(e, t, ..)| (e, plane(t.translation))).collect();
    let half = arena.floor / 2.0;
    let forward = them.0.forward();
    for (e, transform, mut brain, mut facing, mut attack, hp, mut velocity) in &mut enemies {
        let me = plane(transform.translation);
        let to_player = [goal[0] - me[0], goal[1] - me[1]];
        let dist = to_player[0].hypot(to_player[1]);
        // The player is facing this enemy (ninjas won't stand there).
        let in_front = dist > 1e-4 && -(forward[0] * to_player[0] + forward[1] * to_player[1]) / dist > IN_FRONT;
        let stunned = hp.0.stunned();
        let intent = brain.0.decide(dist, in_front, dt, stunned, attack.busy());
        if stunned {
            continue; // knockback plays out
        }
        let mut want = [0.0, 0.0];
        match intent {
            Intent::Walk(pace) => {
                let others: Vec<[f32; 2]> = crowd.iter().filter(|(o, _)| *o != e).map(|(_, p)| *p).collect();
                let half = [half.x, half.y];
                let dir = match brain.0 {
                    // Faced at close range: round to the player's back, not through it (the 2.5D ninja ran through).
                    Mind::Ninja(b) if b.phase == NinjaPhase::Flank => circle(me, goal, forward, &others, half),
                    // A thug ploughs a straight line; the others give way to it.
                    Mind::Melee(b) if b.style.kind == Kind::Thug => steer(me, goal, pace, &[], half),
                    _ => steer(me, goal, pace, &others, half),
                };
                want = [dir[0] * pace.abs() * RUN_SPEED, dir[1] * pace.abs() * RUN_SPEED];
                facing.0.step(dir, dt);
            }
            Intent::Hold => facing.0.step(to_player, dt),
            Intent::Attack(mv) => {
                if let Some(yaw) = heading(to_player) {
                    facing.0.snap(yaw);
                }
                attack.start(mv);
            }
        }
        let v = on_floor(approach(plane(velocity.0), want, RUN_ACCEL * dt));
        velocity.x = v.x;
        velocity.z = v.z;
    }
}

/// The doors as panels on the walls' inner faces, in the arena's darker door tone while closed (`game_core::palette`),
/// pulsing in the signal accent while they warn, and black while open.
#[cfg(feature = "render")]
mod view {
    use bevy::prelude::*;
    use game_core::doors::{Door, WARNING};

    use super::WallDoor;
    use crate::flat::Flat;
    use crate::look::Palette;
    use crate::paint::{Role, colour};

    /// Door panel size, metres: about a fighter and a half tall (taller than box 1's walls, as in Rumble Box).
    const PANEL: Vec3 = Vec3::new(1.2, 2.1, 0.08);

    /// Each door's own material, recoloured as it warns.
    #[derive(Component)]
    struct Panel(Handle<Flat>);

    pub fn plugin(app: &mut App) {
        app.add_systems(Update, (dress, pulse, fade).chain().in_set(crate::Frame::Show));
    }

    fn closed(palette: &Palette) -> Color {
        colour(Role::Door, palette)
    }

    fn dress(
        mut commands: Commands,
        palette: Res<Palette>,
        doors: Query<(Entity, &WallDoor), Added<WallDoor>>,
        mut meshes: ResMut<Assets<Mesh>>,
        mut mats: ResMut<Assets<Flat>>,
    ) {
        for (e, door) in &doors {
            let mat = mats.add(Flat::unlit(closed(&palette)));
            commands.entity(e).insert((Panel(mat.clone()), Visibility::default()));
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::from_size(PANEL))),
                MeshMaterial3d(mat),
                Transform::from_translation(Vec3::Y * PANEL.y / 2.0 + door.inward * PANEL.z / 2.0)
                    .looking_to(-door.inward, Vec3::Y),
                ChildOf(e),
            ));
        }
    }

    fn pulse(palette: Res<Palette>, doors: Query<(&WallDoor, &Panel)>, mut mats: ResMut<Assets<Flat>>) {
        for (door, panel) in &doors {
            let colour = match door.door {
                Door::Closed => closed(&palette),
                // Three flashes over the warning, brightening toward the opening.
                Door::Warning(left) => {
                    let t = 1.0 - left / WARNING;
                    let wave = 0.5 + 0.5 * (t * 3.0 * std::f32::consts::TAU).cos();
                    closed(&palette).mix(&colour(Role::Signal, &palette), (0.35 + 0.65 * t) * wave)
                }
                Door::Open(_) => Color::BLACK,
            };
            let colour = LinearRgba::from(colour);
            if let Some(mut m) = mats.get_mut(&panel.0)
                && m.color != colour
            {
                m.color = colour;
            }
        }
    }

    /// A door in a wall the camera is behind goes with the wall (`look`), so it never hides the fight.
    fn fade(
        cameras: Query<&GlobalTransform, With<Camera3d>>,
        mut doors: Query<(&WallDoor, &GlobalTransform, &mut Visibility)>,
    ) {
        let Some(eye) = cameras.iter().next().map(|c| c.translation()) else { return };
        for (door, at, mut shown) in &mut doors {
            let behind = (eye - at.translation()).with_y(0.0).dot(-door.inward) > 0.0;
            let want = if behind { Visibility::Hidden } else { Visibility::Inherited };
            if *shown != want {
                *shown = want;
            }
        }
    }
}
