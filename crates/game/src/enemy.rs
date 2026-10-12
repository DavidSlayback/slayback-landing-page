//! Fodder: drops into the box, walks at the player, telegraphs, punches. Decisions come from
//! `game_core::enemy::FodderBrain`; this module turns them into velocity, facing and swings.

use avian2d::prelude::*;
use bevy::prelude::*;
use game_core::enemy::{FodderBrain, Intent};
use game_core::movement::run_velocity;

use crate::ARENA;
use crate::fighter::{Attack, Defeated, Fighter, Hp, Team, fighter_bundle, strike_fx};
use crate::player::{Player, visor};

#[derive(Component)]
pub struct Brain(pub FodderBrain);

const HALF: Vec2 = Vec2::new(15.0, 26.0);
const COLOR: Color = Color::srgb(1.0, 0.18, 0.62);
const TELEGRAPH: Color = Color::srgb(1.0, 0.75, 0.9);
const MAX_HP: f32 = 30.0;
/// Keep this many fodder in the box; one more drops in every `SPAWN_EVERY` seconds while below it.
const CROWD: usize = 3;
const SPAWN_EVERY: f32 = 2.0;

#[derive(Resource)]
struct Spawner {
    timer: Timer,
    seed: u32,
}

pub fn plugin(app: &mut App) {
    app.insert_resource(Spawner { timer: Timer::from_seconds(SPAWN_EVERY, TimerMode::Repeating), seed: 0x9e37_79b9 })
        .add_systems(Update, (spawn_waves, think, telegraph, remove_defeated));
}

fn spawn_waves(time: Res<Time>, mut spawner: ResMut<Spawner>, enemies: Query<(), With<Brain>>, mut commands: Commands) {
    if !spawner.timer.tick(time.delta()).just_finished() || enemies.iter().count() >= CROWD {
        return;
    }
    // xorshift: a deterministic spread of drop points without pulling in a rand crate
    spawner.seed ^= spawner.seed << 13;
    spawner.seed ^= spawner.seed >> 17;
    spawner.seed ^= spawner.seed << 5;
    let x = (spawner.seed % 1000) as f32 / 1000.0 * (ARENA.x - 200.0) - (ARENA.x - 200.0) / 2.0;
    commands
        .spawn((
            Brain(FodderBrain::default()),
            Name::new("fodder"),
            fighter_bundle(Team::Enemy, HALF, COLOR, MAX_HP),
            Transform::from_xyz(x, ARENA.y / 2.0 - 80.0, 1.0),
        ))
        .with_children(|c| {
            c.spawn(strike_fx(Color::srgb(1.0, 0.4, 0.75)));
            c.spawn(visor(COLOR));
        });
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

/// Brightens fodder as its windup builds, so the punch is readable before it lands.
fn telegraph(mut enemies: Query<(&Brain, &mut Fighter)>) {
    for (brain, mut fighter) in &mut enemies {
        let t = (brain.0.windup / FodderBrain::WINDUP).clamp(0.0, 1.0);
        fighter.color = COLOR.mix(&TELEGRAPH, t);
    }
}

/// Slice 2: defeated fodder simply leave (slice 3 turns them into climbable debris).
fn remove_defeated(mut defeated: MessageReader<Defeated>, mut commands: Commands) {
    for d in defeated.read().filter(|d| d.team == Team::Enemy) {
        commands.entity(d.entity).despawn();
    }
}
