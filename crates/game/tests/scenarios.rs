//! Headless game scenarios (test seam 2): the real game systems in an app with no window or renderer, stepped
//! frame by frame on a fixed clock with scripted keyboard input. They assert what a player would see happen.

use std::time::Duration;

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use game::arena::WALL;
use game::player::{Player, SIZE, SPAWN};
use game::ARENA;

const FPS: f32 = 60.0;

/// The y the player's centre settles at when standing on the floor.
const STANDING_Y: f32 = -ARENA.y / 2.0 + WALL + SIZE.y / 2.0;

/// A game app without window or renderer. Keyboard state is driven by the test (no input plugin clears it).
fn headless() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, game::plugin))
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / FPS)));
    app.finish();
    app.cleanup();
    app
}

fn step(app: &mut App, seconds: f32) {
    for _ in 0..(seconds * FPS).round() as usize {
        app.update();
        // Edge states last one frame, as with real input.
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear();
    }
}

fn keys(app: &mut App) -> Mut<'_, ButtonInput<KeyCode>> {
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>()
}

/// Player position, velocity and whether its ground probe touches something.
fn player(app: &mut App) -> (Vec2, Vec2, bool) {
    let mut q = app.world_mut().query_filtered::<(&Transform, &LinearVelocity, &ShapeHits), With<Player>>();
    let (t, v, hits) = q.single(app.world()).expect("exactly one player");
    (t.translation.truncate(), v.0, !hits.is_empty())
}

/// A headless app whose player has already landed.
fn standing() -> App {
    let mut app = headless();
    step(&mut app, 1.5);
    app
}

#[test]
fn player_spawns_falls_and_comes_to_rest_on_the_floor() {
    let mut app = headless();
    step(&mut app, 0.1);
    let (pos, vel, _) = player(&mut app);
    assert!(pos.y < SPAWN.y && vel.y < 0.0, "falling after 0.1 s: pos {pos}, vel {vel}");

    step(&mut app, 1.4);
    let (pos, vel, grounded) = player(&mut app);
    assert!((pos.y - STANDING_Y).abs() < 2.0, "rests on the floor: y {} vs {STANDING_Y}", pos.y);
    assert!(vel.length() < 5.0, "at rest: vel {vel}");
    assert!(grounded, "ground probe touches the floor");
    assert!((pos.x - SPAWN.x).abs() < 1.0, "lands where it spawned: x {}", pos.x);
}

#[test]
fn holding_run_moves_the_player_and_releasing_stops_it() {
    let mut app = standing();
    keys(&mut app).press(KeyCode::KeyD);
    step(&mut app, 1.0);
    let (pos, vel, _) = player(&mut app);
    assert!(pos.x > 300.0, "ran right about 400 units in 1 s: x {}", pos.x);
    assert!((vel.x - game_core::movement::RUN_SPEED).abs() < 1.0, "at full run speed: vx {}", vel.x);

    keys(&mut app).release(KeyCode::KeyD);
    step(&mut app, 0.25);
    let (stopped, vel, _) = player(&mut app);
    assert!(vel.x.abs() < 1.0 && stopped.x - pos.x < 60.0, "stops quickly: vx {}, slid {}", vel.x, stopped.x - pos.x);

    keys(&mut app).press(KeyCode::ArrowLeft);
    step(&mut app, 0.5);
    assert!(player(&mut app).0.x < stopped.x - 100.0, "arrow keys run left too");
}

#[test]
fn a_grounded_jump_rises_and_lands_again() {
    let mut app = standing();
    keys(&mut app).press(KeyCode::Space);
    let mut peak = f32::MIN;
    for _ in 0..30 {
        step(&mut app, 1.0 / FPS);
        keys(&mut app).release(KeyCode::Space);
        peak = peak.max(player(&mut app).0.y);
    }
    // Apex is JUMP_SPEED² / 2g ≈ 200 units above the floor.
    assert!(peak > STANDING_Y + 150.0, "jumped: peak {peak} vs standing {STANDING_Y}");

    step(&mut app, 1.0);
    let (pos, _, grounded) = player(&mut app);
    assert!((pos.y - STANDING_Y).abs() < 2.0 && grounded, "landed again: y {}", pos.y);
}

#[test]
fn jumping_in_mid_air_does_nothing() {
    let mut app = standing();
    keys(&mut app).press(KeyCode::Space);
    step(&mut app, 0.2);
    let (_, rising, grounded) = player(&mut app);
    assert!(!grounded, "airborne after the first jump");

    keys(&mut app).press(KeyCode::KeyW);
    step(&mut app, 1.0 / FPS);
    let (_, vel, _) = player(&mut app);
    assert!(vel.y < rising.y, "no second boost in the air: vy {} after {}", vel.y, rising.y);
}
