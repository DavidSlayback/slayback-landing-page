//! Enemies (fodder, thugs, ninjas) enter through doors in the side walls, which warn first; then they walk at the player,
//! telegraph and strike. Decisions come from `game_core::enemy::Mind`, door timing from `game_core::doors`, and
//! the mix from the box (`game_core::run`); this module turns them into entities, velocity, facing and swings.

use avian2d::prelude::*;
use bevy::prelude::*;
use game_core::doors::Door;
use game_core::enemy::{Armour, FODDER, Intent, Kind, Mind};
use game_core::run::pick_kind;
use game_core::movement::run_velocity;

use crate::ARENA;
use crate::arena::WALL;
use crate::bomb::BombHead;
use crate::fighter::{Attack, Fighter, Hp, Team, body};
use crate::grab::Carried;
use crate::player::Player;
use crate::run::RunState;

#[derive(Component)]
pub struct Brain(pub Mind);

/// Blows this enemy shrugs off (see `game_core::enemy::staggers`).
#[derive(Component, Clone, Copy)]
pub struct Armoured(pub Armour);

/// A door in a side wall: `side` is -1 (left) or +1 (right). `next` is who it's warning about.
#[derive(Component)]
pub struct WallDoor {
    pub side: f32,
    pub door: Door,
    pub next: Kind,
    /// Whether that enemy carries a bomb for a head.
    pub bomb: bool,
}

/// Fodder collider half-size and health (the common enemy; tests use these).
pub const HALF: Vec2 = Vec2::new(FODDER.half.0, FODDER.half.1);
pub const MAX_HP: f32 = FODDER.max_hp;

/// Enemy entries. Tests turn them off to place fodder by hand.
#[derive(Resource)]
pub struct Waves {
    pub enabled: bool,
    /// Keep this many fodder in the box; ask a door for one more every `timer` while below it.
    pub crowd: usize,
    pub timer: Timer,
}

impl Default for Waves {
    fn default() -> Self {
        Self { enabled: true, crowd: 3, timer: Timer::from_seconds(2.0, TimerMode::Repeating) }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Waves>()
        .add_systems(Startup, spawn_doors)
        .add_systems(Update, (request_entries, open_doors, think));
}

/// Where a door's enemy (of half-size `half`) appears: just inside the wall, standing on the floor.
pub fn door_spot(side: f32, half: Vec2) -> Vec2 {
    Vec2::new(side * (ARENA.x / 2.0 - WALL - half.x - 2.0), -ARENA.y / 2.0 + WALL + half.y + 1.0)
}

fn spawn_doors(mut commands: Commands) {
    for side in [-1.0, 1.0] {
        commands.spawn((Name::new("door"), WallDoor { side, door: Door::Closed, next: Kind::Fodder, bomb: false }, Transform::from_translation(door_spot(side, HALF).extend(0.0))));
    }
}

/// An enemy of `kind` standing at `at`.
pub fn enemy(kind: Kind, at: Vec2) -> impl Bundle {
    let s = kind.style();
    let half = Vec2::new(s.half.0, s.half.1);
    let name = match kind {
        Kind::Fodder => "fodder",
        Kind::Thug => "thug",
        Kind::Ninja => "ninja",
    };
    (Brain(Mind::new(kind)), Armoured(s.armour), Name::new(name), body(Team::Enemy, half, s.max_hp), Transform::from_translation(at.extend(0.0)))
}

/// A fodder standing at `at`.
pub fn fodder(at: Vec2) -> impl Bundle {
    enemy(Kind::Fodder, at)
}

fn request_entries(
    time: Res<Time>,
    mut waves: ResMut<Waves>,
    run: Res<RunState>,
    enemies: Query<(), With<Brain>>,
    mut doors: Query<&mut WallDoor>,
    mut seed: Local<u32>,
) {
    if !waves.enabled || !waves.timer.tick(time.delta()).just_finished() {
        return;
    }
    let pending = doors.iter().filter(|d| d.door != Door::Closed).count();
    if enemies.iter().count() + pending >= waves.crowd {
        return;
    }
    // Alternate sides: the door with the most recent entry is skipped while the other is free.
    let mut free: Vec<_> = doors.iter_mut().filter(|d| d.door == Door::Closed).collect();
    let pick = (time.elapsed_secs() / 2.0) as usize % free.len().max(1);
    if let Some(d) = free.get_mut(pick) {
        // xorshift: the box's enemy mix without a rand dependency.
        *seed = seed.wrapping_add(0x9E37_79B9);
        let mut s = *seed;
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        d.next = pick_kind(&run.0.spec, (s & 0xFFFF) as f32 / 65536.0);
        d.bomb = ((s >> 16) as f32 / 65536.0) < run.0.spec.bombs;
        d.door.request();
    }
}

fn open_doors(time: Res<Time>, mut commands: Commands, mut doors: Query<&mut WallDoor>) {
    for mut d in &mut doors {
        if d.door.tick(time.delta_secs()) {
            let s = d.next.style();
            let mut e = commands.spawn(enemy(d.next, door_spot(d.side, Vec2::new(s.half.0, s.half.1))));
            if d.bomb {
                e.insert(BombHead);
            }
        }
    }
}

#[allow(clippy::type_complexity)]
fn think(
    time: Res<Time>,
    player: Query<(&Transform, &Fighter), (With<Player>, Without<Brain>)>,
    mut enemies: Query<(&Transform, &mut Brain, &mut Fighter, &mut Attack, &Hp, &mut LinearVelocity), Without<Carried>>,
) {
    let Ok((target, them)) = player.single() else { return };
    let dt = time.delta_secs();
    for (transform, mut brain, mut fighter, mut attack, hp, mut velocity) in &mut enemies {
        let dx = target.translation.x - transform.translation.x;
        let stunned = hp.0.stunned();
        // The player is facing this enemy (ninjas won't stand there).
        let in_front = (-dx).signum() == them.facing;
        match brain.0.decide(dx, in_front, dt, stunned, attack.busy()) {
            _ if stunned => {} // knockback plays out
            Intent::Walk(axis) => {
                fighter.facing = axis.signum();
                velocity.x = run_velocity(velocity.x, axis, dt);
            }
            Intent::Hold => {
                fighter.facing = if dx != 0.0 { dx.signum() } else { fighter.facing };
                velocity.x = run_velocity(velocity.x, 0.0, dt);
            }
            Intent::Attack(mv) => {
                fighter.facing = dx.signum();
                attack.start(mv);
            }
        }
    }
}
