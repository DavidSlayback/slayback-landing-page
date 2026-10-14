//! Headless 3D game scenarios: the real game systems in an app with no window or renderer, stepped frame by frame on
//! a fixed clock with scripted input. They assert what a player would see happen.

use std::time::Duration;

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use game_core::combat::Move;
use game_core::debris::solid::{FODDER, FODDER_HEIGHT, MAX_FROZEN, MAX_MOVING, SETTLE, Solid};
use game_core::plane::RUN_SPEED;
use game3d::arena::Arena;
use game3d::debris::{Piece, piece};
use game3d::enemy::{Brain, Waves, fodder};
use game3d::fighter::{Defeated, Hp, Team};
use game3d::player::{Facing, HEIGHT, Player, RADIUS, SPAWN};
use game3d::score::RunScore;

const FPS: f32 = 60.0;

/// A game app without window or renderer. Keyboard state is driven by the test (no input plugin clears it), and
/// enemies only appear where a test puts them.
fn headless() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, game3d::plugin))
        .insert_resource(Waves { enabled: false, ..default() })
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

// Fighting (G2).

/// Puts a fodder on the floor at `offset` (a plane vector, metres) from the player, facing it.
fn fodder_at(app: &mut App, offset: [f32; 2]) -> Entity {
    let at = player(app).0.with_y(FODDER_HEIGHT / 2.0 + 0.02) + game3d::on_floor(offset);
    let yaw = (-offset[1]).atan2(-offset[0]);
    app.world_mut().spawn(fodder(at, yaw)).id()
}

fn hp(app: &App, e: Entity) -> f32 {
    app.world().get::<Hp>(e).expect("still in the box").0.hp
}

fn player_hp(app: &mut App) -> (f32, f32) {
    let h = app.world_mut().query_filtered::<&Hp, With<Player>>().single(app.world()).unwrap().0;
    (h.hp, h.max)
}

const FULL: f32 = game_core::enemy::FODDER.max_hp;

/// Presses `key` for one frame, then lets it go.
fn tap(app: &mut App, key: KeyCode) {
    keys(app).press(key);
    step(app, 1.0 / FPS);
    keys(app).release(key);
}

#[test]
fn a_punch_lands_on_fodder_in_its_cone_and_not_beside_behind_or_beyond() {
    let mut app = standing();
    // The player faces +x (screen right).
    let ahead = fodder_at(&mut app, [1.1, 0.0]);
    let beside = fodder_at(&mut app, [0.0, 1.2]);
    let behind = fodder_at(&mut app, [-1.1, 0.0]);
    let beyond = fodder_at(&mut app, [3.2, 0.3]);
    step(&mut app, 0.05);
    tap(&mut app, KeyCode::KeyJ);
    step(&mut app, 0.2);
    assert!(hp(&app, ahead) < FULL, "the punch hurt the fodder in front");
    assert!(app.world().get::<LinearVelocity>(ahead).unwrap().x > 1.0, "and knocked it away from the player");
    for (who, e) in [("beside", beside), ("behind", behind), ("beyond reach", beyond)] {
        assert_eq!(hp(&app, e), FULL, "the fodder {who} is untouched");
    }
}

#[test]
fn aim_assist_turns_a_swing_toward_the_nearest_enemy_in_the_cone() {
    let mut app = standing();
    let off = 40f32.to_radians();
    let target = fodder_at(&mut app, [1.2 * off.cos(), 1.2 * off.sin()]);
    fodder_at(&mut app, [1.6, -0.3]);
    step(&mut app, 0.05);
    tap(&mut app, KeyCode::KeyK);
    step(&mut app, 0.3);
    let yaw = player(&mut app).2;
    assert!((yaw - off).abs() < 0.05, "turned 40 degrees toward the nearest one: yaw {yaw}");
    assert!(hp(&app, target) < FULL, "and the kick landed on it");
}

#[test]
fn a_dash_kick_launches_harder_than_a_punch() {
    let mut app = standing();
    let target = fodder_at(&mut app, [2.4, 0.0]);
    tap(&mut app, KeyCode::Space);
    tap(&mut app, KeyCode::KeyJ);
    let mut fastest: f32 = 0.0;
    for _ in 0..20 {
        step(&mut app, 1.0 / FPS);
        fastest = fastest.max(app.world().get::<LinearVelocity>(target).unwrap().x);
    }
    let punch = game_core::aim::launch(Move::Punch, 0.0).0[0];
    assert!(hp(&app, target) < FULL, "the drop-kick landed");
    assert!(fastest > punch * 1.5, "launched at {fastest} m/s, far harder than a punch ({punch})");
}

#[test]
fn a_blow_lands_even_on_slow_frames() {
    let mut app = standing();
    // A punch thrown at 20 fps...
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 20.0)));
    let first = fodder_at(&mut app, [1.1, 0.0]);
    app.update();
    keys(&mut app).press(KeyCode::KeyJ);
    app.update();
    keys(&mut app).release(KeyCode::KeyJ);
    for _ in 0..6 {
        app.update();
        keys(&mut app).clear();
    }
    assert!(hp(&app, first) < FULL, "a punch lands at 20 fps");
    // ...and one thrown just before a 250 ms frame (Bevy's longest step) that jumps its whole active window.
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / FPS)));
    step(&mut app, 0.5);
    let second = fodder_at(&mut app, [1.1, 0.0]);
    step(&mut app, 0.05);
    tap(&mut app, KeyCode::KeyJ);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(0.25)));
    app.update();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / FPS)));
    step(&mut app, 0.1);
    assert!(hp(&app, second) < FULL, "the punch still landed");
}

fn enemies(app: &mut App) -> Vec<Vec3> {
    let mut q = app.world_mut().query_filtered::<&Transform, With<Brain>>();
    q.iter(app.world()).map(|t| t.translation).collect()
}

#[test]
fn fodder_close_in_keep_apart_and_punch() {
    let mut app = standing();
    for (i, x) in [-5.0, -4.6, -4.2, -3.8].into_iter().enumerate() {
        fodder_at(&mut app, [x, 0.15 * i as f32 - 0.2]);
    }
    step(&mut app, 4.0);
    let me = player(&mut app).0;
    let crowd = enemies(&mut app);
    for (i, a) in crowd.iter().enumerate() {
        // Their punches shove the player about, so they may be a step behind.
        assert!(a.xz().distance(me.xz()) < 3.0, "closed in on the player at {me}: {a}");
        for b in &crowd[i + 1..] {
            assert!(a.xz().distance(b.xz()) > 0.6, "kept apart: {a} and {b}");
        }
    }
    let (now, full) = player_hp(&mut app);
    assert!(now < full, "and punched the player: {now}");
}

#[test]
fn fodder_dont_pin_the_player_against_a_wall() {
    let mut app = standing();
    hold(&mut app, &[KeyCode::KeyA], 2.0);
    let wall = player(&mut app).0;
    let half = Arena::default().floor / 2.0;
    assert!(wall.x < -half.x + RADIUS + 0.05, "against the left wall: {wall}");
    // A crowd closes the open side, arriving at different times so their punches don't fall in step.
    for offset in [[1.2, 0.0], [0.9, 0.9], [0.9, -0.9], [1.6, 0.4]] {
        fodder_at(&mut app, offset);
        step(&mut app, 0.27);
    }
    step(&mut app, 1.0);
    // Walking out through them: fighters pass through each other, so only blows can hold the player back.
    hold(&mut app, &[KeyCode::KeyD], 2.0);
    let out = player(&mut app).0;
    assert!(out.x > wall.x + 4.0, "walked out through the crowd: from {wall} to {out}");
}

fn pieces(app: &mut App) -> Vec<(Vec3, RigidBody)> {
    let mut q = app.world_mut().query_filtered::<(&Transform, &RigidBody), With<Piece>>();
    q.iter(app.world()).map(|(t, b)| (t.translation, *b)).collect()
}

#[test]
fn a_defeated_fodder_bursts_into_its_pieces_which_tumble_then_freeze_on_the_floor() {
    let mut app = standing();
    let target = fodder_at(&mut app, [1.1, 0.0]);
    app.world_mut().get_mut::<Hp>(target).unwrap().0.hp = 1.0;
    step(&mut app, 0.05);
    tap(&mut app, KeyCode::KeyJ);
    step(&mut app, 0.4);
    assert!(app.world().get_entity(target).is_err(), "the defeated fodder is gone");
    let burst = pieces(&mut app);
    assert_eq!(burst.len(), FODDER.len(), "it burst into all its pieces");
    assert!(burst.iter().all(|(_, b)| *b == RigidBody::Dynamic), "tumbling at first");
    assert_eq!(app.world().resource::<RunScore>().0.points, game_core::score::KILL, "and the kill scored");
    // Coming to rest takes a few seconds (the ball head rolls a while), then the settle time; all within the loose
    // limit, so this is the rest rule freezing them, not the age one.
    step(&mut app, SETTLE + 3.5);
    let settled = pieces(&mut app);
    assert!(settled.iter().all(|(_, b)| *b == RigidBody::Static), "all frozen: {settled:?}");
    for (at, _) in settled {
        assert!(at.y > 0.0 && at.y < 1.0, "resting on the floor, not through it or floating: {at}");
    }
}

#[test]
fn a_mass_burst_keeps_at_most_the_moving_cap_in_motion_and_all_of_it_freezes() {
    let mut app = standing();
    // Thirty enemies defeated at once, in rows across the back of the box: 330 pieces.
    let mut defeated = Vec::new();
    for i in 0..30 {
        let at = Vec3::new(-6.0 + (i % 10) as f32 * 1.3, FODDER_HEIGHT / 2.0, -3.5 + (i / 10) as f32 * 1.4);
        let e = app.world_mut().spawn(fodder(at, 0.0)).id();
        defeated.push(Defeated { entity: e, team: Team::Enemy, at });
    }
    step(&mut app, 0.1);
    for d in defeated {
        app.world_mut().write_message(d);
    }
    let mut most_moving = 0;
    for _ in 0..((SETTLE + 6.0) * FPS) as usize {
        step(&mut app, 1.0 / FPS);
        let moving = pieces(&mut app).iter().filter(|(_, b)| *b == RigidBody::Dynamic).count();
        most_moving = most_moving.max(moving);
    }
    assert_eq!(most_moving, MAX_MOVING, "the cap held, and was reached");
    let all = pieces(&mut app);
    assert_eq!(all.len(), 30 * FODDER.len(), "nothing was lost");
    assert!(all.iter().all(|(_, b)| *b == RigidBody::Static), "and everything froze");
}

#[test]
fn past_the_frozen_cap_the_oldest_frozen_pieces_go() {
    let mut app = standing();
    let cube = Solid::Cube([0.3, 0.3, 0.3]);
    // A floor-wide carpet of frozen cubes, three deep, oldest first: 40 more than the cap.
    for i in 0..MAX_FROZEN + 40 {
        let (layer, cell) = (i / 600, i % 600);
        let at =
            Vec3::new(-7.2 + (cell % 30) as f32 * 0.5, 0.16 + layer as f32 * 0.32, -4.8 + (cell / 30) as f32 * 0.5);
        app.world_mut().spawn(piece(cube, 0, at, Quat::IDENTITY, i as f32 * 1e-3)).insert(RigidBody::Static);
    }
    step(&mut app, 1.0 / FPS);
    let mut q = app.world_mut().query::<&Piece>();
    let born: Vec<f32> = q.iter(app.world()).map(|p| p.born).collect();
    assert_eq!(born.len(), MAX_FROZEN);
    let oldest = born.iter().fold(f32::MAX, |m, b| m.min(*b));
    assert!((oldest - 40.0 * 1e-3).abs() < 1e-6, "the 40 oldest went: the oldest left was born at {oldest}");
}
