//! Fodder: enter through doors in the side walls (which warn first), walk at the player, telegraph, punch.
//! Decisions come from `game_core::enemy::FodderBrain` and door timing from `game_core::doors`; this module turns
//! them into entities, velocity, facing and swings.

use avian2d::prelude::*;
use bevy::prelude::*;
use game_core::doors::Door;
use game_core::enemy::{FodderBrain, Intent};
use game_core::movement::run_velocity;

use crate::ARENA;
use crate::arena::WALL;
use crate::fighter::{Attack, Fighter, Hp, Team, body};
use crate::player::Player;

#[derive(Component)]
pub struct Brain(pub FodderBrain);

/// A door in a side wall: `side` is -1 (left) or +1 (right).
#[derive(Component)]
pub struct WallDoor {
    pub side: f32,
    pub door: Door,
}

/// Fodder collider half-size.
pub const HALF: Vec2 = Vec2::new(15.0, 26.0);
pub const MAX_HP: f32 = 30.0;

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

/// Where a door's fodder appears: just inside the wall, standing on the floor.
pub fn door_spot(side: f32) -> Vec2 {
    Vec2::new(side * (ARENA.x / 2.0 - WALL - HALF.x - 2.0), -ARENA.y / 2.0 + WALL + HALF.y + 1.0)
}

fn spawn_doors(mut commands: Commands) {
    for side in [-1.0, 1.0] {
        commands.spawn((Name::new("door"), WallDoor { side, door: Door::Closed }, Transform::from_translation(door_spot(side).extend(0.0))));
    }
}

/// A fodder standing at `at`.
pub fn fodder(at: Vec2) -> impl Bundle {
    (Brain(FodderBrain::default()), Name::new("fodder"), body(Team::Enemy, HALF, MAX_HP), Transform::from_translation(at.extend(0.0)))
}

fn request_entries(time: Res<Time>, mut waves: ResMut<Waves>, enemies: Query<(), With<Brain>>, mut doors: Query<&mut WallDoor>) {
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
        d.door.request();
    }
}

fn open_doors(time: Res<Time>, mut commands: Commands, mut doors: Query<&mut WallDoor>) {
    for mut d in &mut doors {
        if d.door.tick(time.delta_secs()) {
            commands.spawn(fodder(door_spot(d.side)));
        }
    }
}

#[allow(clippy::type_complexity)]
fn think(
    time: Res<Time>,
    player: Query<&Transform, (With<Player>, Without<Brain>)>,
    mut enemies: Query<(&Transform, &mut Brain, &mut Fighter, &mut Attack, &Hp, &mut LinearVelocity)>,
) {
    let Ok(target) = player.single() else { return };
    let dt = time.delta_secs();
    for (transform, mut brain, mut fighter, mut attack, hp, mut velocity) in &mut enemies {
        let dx = target.translation.x - transform.translation.x;
        let stunned = hp.0.stunned();
        match brain.0.decide(dx, dt, stunned, attack.busy()) {
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
