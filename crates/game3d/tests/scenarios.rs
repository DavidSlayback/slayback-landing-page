//! Headless 3D game scenarios: the real game systems in an app with no window or renderer, stepped frame by frame on
//! a fixed clock with scripted input. They assert what a player would see happen.

use std::time::Duration;

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use game_core::plane::RUN_SPEED;
use game3d::arena::Arena;
use game3d::player::{Facing, HEIGHT, Player, RADIUS, SPAWN};

const FPS: f32 = 60.0;

/// A game app without window or renderer. Keyboard state is driven by the test (no input plugin clears it).
fn headless() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, game3d::plugin))
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

/// Player position, velocity and facing yaw.
fn player(app: &mut App) -> (Vec3, Vec3, f32) {
    let mut q = app.world_mut().query_filtered::<(&Transform, &LinearVelocity, &Facing), With<Player>>();
    let (t, v, f) = q.single(app.world()).expect("exactly one player");
    (t.translation, v.0, f.0.yaw)
}

/// A headless app whose player has already landed.
fn standing() -> App {
    let mut app = headless();
    step(&mut app, 1.0);
    app
}

fn press(app: &mut App, held: &[KeyCode]) {
    for &k in held {
        keys(app).press(k);
    }
}

/// Holds `held` for `seconds`, then lets go of them.
fn hold(app: &mut App, held: &[KeyCode], seconds: f32) {
    press(app, held);
    step(app, seconds);
    keys(app).release_all();
}

#[test]
fn the_player_lands_on_the_floor_and_stands_still() {
    let mut app = headless();
    step(&mut app, 1.0);
    let (pos, vel, _) = player(&mut app);
    assert!((pos.y - HEIGHT / 2.0).abs() < 0.05, "stands on the floor: y {}", pos.y);
    assert!(vel.length() < 0.05, "at rest: {vel}");
    assert!(pos.xz().distance(SPAWN.xz()) < 0.01, "lands where it spawned: {pos}");
}

#[test]
fn the_player_walks_into_a_corner_and_the_walls_stop_it() {
    let mut app = standing();
    // Up and right: toward the back-right corner, across the whole floor.
    press(&mut app, &[KeyCode::KeyW, KeyCode::KeyD]);
    step(&mut app, 4.0);
    let (before, _, _) = player(&mut app);
    step(&mut app, 0.5);
    let (pos, _, _) = player(&mut app);
    let half = Arena::default().floor / 2.0;
    let corner = Vec2::new(half.x - RADIUS, -half.y + RADIUS);
    assert!(pos.xz().distance(corner) < 0.05, "pinned in the back-right corner: {pos}, corner {corner}");
    assert!(pos.distance(before) < 0.01, "and stays there while still pushing: moved {}", pos.distance(before));
    assert!((pos.y - HEIGHT / 2.0).abs() < 0.05, "still on the floor, not climbing the walls: y {}", pos.y);
}

#[test]
fn no_wall_lets_the_player_out_including_the_glass_front() {
    let half = Arena::default().floor / 2.0;
    let dirs: [&[KeyCode]; 8] = [
        &[KeyCode::KeyW],
        &[KeyCode::KeyW, KeyCode::KeyD],
        &[KeyCode::KeyD],
        &[KeyCode::KeyS, KeyCode::KeyD],
        &[KeyCode::KeyS],
        &[KeyCode::KeyS, KeyCode::KeyA],
        &[KeyCode::KeyA],
        &[KeyCode::KeyW, KeyCode::KeyA],
    ];
    for held in dirs {
        let mut app = standing();
        hold(&mut app, held, 4.0);
        let (pos, _, _) = player(&mut app);
        let inside = pos.x.abs() <= half.x - RADIUS + 0.02 && pos.z.abs() <= half.y - RADIUS + 0.02;
        assert!(inside, "{held:?} left the box: {pos}");
        assert!(pos.x.abs() > half.x - 0.5 || pos.z.abs() > half.y - 0.5, "{held:?} reached a wall: {pos}");
    }
    // Walking toward the camera ends against the front wall.
    let mut app = standing();
    hold(&mut app, &[KeyCode::ArrowDown], 3.0);
    assert!((player(&mut app).0.z - (half.y - RADIUS)).abs() < 0.05, "stopped by the glass");
}

#[test]
fn moves_run_at_full_speed_in_every_direction_and_stop_on_release() {
    let mut app = standing();
    let (start, _, _) = player(&mut app);
    press(&mut app, &[KeyCode::ArrowLeft, KeyCode::ArrowDown]);
    step(&mut app, 0.3);
    let (_, vel, _) = player(&mut app);
    assert!((vel.xz().length() - RUN_SPEED).abs() < 0.1, "a diagonal runs at full speed, not faster: {vel}");
    assert!(vel.x < 0.0 && vel.z > 0.0, "left and toward the camera: {vel}");
    keys(&mut app).release_all();
    step(&mut app, 0.2);
    let (pos, vel, _) = player(&mut app);
    assert!(vel.xz().length() < 0.05, "stops within a fifth of a second: {vel}");
    assert!(pos.xz().distance(start.xz()) > 1.0, "it moved: {pos}");
}

#[test]
fn the_player_turns_smoothly_to_face_its_last_move_and_keeps_facing_it() {
    use std::f32::consts::{FRAC_PI_2, PI};
    let mut app = standing();
    assert!(player(&mut app).2.abs() < 1e-6, "starts facing +x (screen right)");
    // Toward the camera: plane heading -90 degrees.
    keys(&mut app).press(KeyCode::KeyS);
    step(&mut app, 1.0 / FPS);
    let yaw = player(&mut app).2;
    assert!(yaw < 0.0 && yaw > -FRAC_PI_2 + 0.1, "turning, not snapped: {yaw}");
    step(&mut app, 0.25);
    assert!((player(&mut app).2 + FRAC_PI_2).abs() < 1e-4, "faces the move");
    keys(&mut app).release(KeyCode::KeyS);
    step(&mut app, 0.5);
    assert!((player(&mut app).2 + FRAC_PI_2).abs() < 1e-4, "keeps facing it after letting go");
    hold(&mut app, &[KeyCode::KeyA], 0.4);
    assert!((player(&mut app).2.abs() - PI).abs() < 1e-4, "about-face to the left");
}

#[test]
fn a_gamepad_stick_moves_the_player_in_eight_directions() {
    let mut app = standing();
    let (start, _, _) = player(&mut app);
    let mut pad = Gamepad::default();
    // Mostly right and a little up: snaps to straight right.
    pad.analog_mut().set(GamepadAxis::LeftStickX, 0.9);
    pad.analog_mut().set(GamepadAxis::LeftStickY, 0.3);
    let id = app.world_mut().spawn(pad).id();
    step(&mut app, 0.5);
    let (pos, vel, _) = player(&mut app);
    assert!(vel.x > RUN_SPEED - 0.1 && vel.z.abs() < 1e-3, "straight right at full speed: {vel}");
    assert!(pos.x > start.x + 2.0, "it moved: {pos}");
    // A drifting stick inside the deadzone does nothing.
    let mut pad = app.world_mut().get_mut::<Gamepad>(id).unwrap();
    pad.analog_mut().set(GamepadAxis::LeftStickX, 0.1);
    pad.analog_mut().set(GamepadAxis::LeftStickY, 0.1);
    step(&mut app, 0.3);
    assert!(player(&mut app).1.xz().length() < 0.05, "the deadzone holds it still");
}
