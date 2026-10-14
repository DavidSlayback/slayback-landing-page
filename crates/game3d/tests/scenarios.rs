//! Headless 3D game scenarios: the real game systems in an app with no window or renderer, stepped frame by frame on
//! a fixed clock with scripted input. They assert what a player would see happen.

use std::time::Duration;

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use game_core::climb::{Placed, pile_height};
use game_core::combat::Move;
use game_core::debris::solid::{FODDER, FODDER_HEIGHT, MAX_FROZEN, MAX_MOVING, SETTLE, Solid};
use game_core::grab::{Held, impact};
use game_core::plane::RUN_SPEED;
use game3d::arena::{Arena, Slab};
use game3d::debris::{Piece, piece};
use game3d::enemy::{Brain, Waves, fodder};
use game3d::fighter::{Defeated, Hp, Team};
use game3d::grab::{Carried, Thrown};
use game3d::player::{Facing, HEIGHT, Holding, Player, RADIUS, SPAWN};
use game3d::run::RunState;
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

// Climbing and the run (G3).

/// Puts the player at `at` (its capsule's centre), at rest.
fn teleport(app: &mut App, at: Vec3) {
    let mut q = app.world_mut().query_filtered::<(&mut Transform, &mut Position, &mut LinearVelocity), With<Player>>();
    let (mut t, mut p, mut v) = q.single_mut(app.world_mut()).unwrap();
    t.translation = at;
    p.0 = at;
    v.0 = Vec3::ZERO;
}

/// A frozen block of debris of `size`, centred at `at` and turned `yaw` radians about the vertical.
fn block(app: &mut App, size: [f32; 3], at: Vec3, yaw: f32) -> Entity {
    let turn = Quat::from_rotation_y(yaw);
    app.world_mut().spawn(piece(Solid::Cube(size), 0, at, turn, 0.0)).insert(RigidBody::Static).id()
}

fn feet(app: &mut App) -> f32 {
    player(app).0.y - HEIGHT / 2.0
}

fn box_number(app: &App) -> usize {
    app.world().resource::<RunState>().0.spec.number
}

/// Holds `held` for `seconds`, returning the highest the feet got.
fn hold_highest(app: &mut App, held: &[KeyCode], seconds: f32) -> f32 {
    press(app, held);
    let mut top = f32::MIN;
    for _ in 0..(seconds * FPS).round() as usize {
        step(app, 1.0 / FPS);
        top = top.max(feet(app));
    }
    keys(app).release_all();
    top
}

/// The fodder's torso block, the biggest debris piece.
const TORSO: [f32; 3] = [0.5, 0.32, 0.4];

#[test]
fn the_player_steps_up_onto_a_piece_lying_on_the_floor_but_not_onto_a_waist_high_block() {
    let mut app = standing();
    let at = player(&mut app).0;
    block(&mut app, TORSO, Vec3::new(at.x + 1.5, TORSO[1] / 2.0, at.z), 0.0);
    step(&mut app, 0.1);
    let top = hold_highest(&mut app, &[KeyCode::KeyD], 0.7);
    assert!((top - TORSO[1]).abs() < 0.03, "stepped onto the torso block: feet reached {top}");
    assert!(player(&mut app).0.x > at.x + 2.5, "and walked on over it");
    assert!(feet(&mut app).abs() < 0.02, "back down on the floor: {}", feet(&mut app));

    let mut app = standing();
    let at = player(&mut app).0;
    block(&mut app, [0.6, 0.8, 1.6], Vec3::new(at.x + 1.5, 0.4, at.z), 0.0);
    step(&mut app, 0.1);
    let top = hold_highest(&mut app, &[KeyCode::KeyD], 1.5);
    assert!(top < 0.02, "a 0.8 m block is a wall, not a step: feet reached {top}");
    assert!(player(&mut app).0.x < at.x + 1.5 - 0.3 - RADIUS + 0.02, "stopped against it");
}

#[test]
fn the_player_cannot_climb_a_wall_or_the_glass() {
    for held in [KeyCode::KeyW, KeyCode::KeyA, KeyCode::KeyD, KeyCode::KeyS] {
        let mut app = standing();
        // Walk into it, then keep pushing and dashing against it.
        let mut top = hold_highest(&mut app, &[held], 2.0);
        for _ in 0..4 {
            press(&mut app, &[held]);
            tap(&mut app, KeyCode::Space);
            top = top.max(hold_highest(&mut app, &[held], 0.5));
        }
        assert!(top < 0.02, "{held:?}: the feet stayed on the floor, reaching {top}");
    }
}

#[test]
fn from_a_pile_just_under_the_rim_the_player_can_neither_step_onto_the_glass_nor_over_it() {
    let mut app = standing();
    let rim = app.world().resource::<Arena>().rim;
    let half = Arena::default().floor / 2.0;
    // A platform against the glass, a little lower than a step under the rim, and the player on it.
    let top = rim - 0.3;
    block(&mut app, [3.0, top, 1.5], Vec3::new(0.0, top / 2.0, half.y - 0.75), 0.0);
    teleport(&mut app, Vec3::new(0.0, top + HEIGHT / 2.0 + 0.05, half.y - 1.0));
    step(&mut app, 0.3);
    assert!((feet(&mut app) - top).abs() < 0.03, "stands on the platform: {}", feet(&mut app));
    let highest = hold_highest(&mut app, &[KeyCode::KeyS], 2.0);
    let (pos, _, _) = player(&mut app);
    assert!(pos.z < half.y - RADIUS + 0.02, "still inside the glass: {pos}");
    assert!(highest < top + 0.02, "never rose onto the glass's top edge: {highest}");
    assert_eq!(box_number(&app), 1, "and that is no escape");
}

#[test]
fn the_player_climbs_a_pile_of_debris_and_escapes_to_box_2() {
    let mut app = standing();
    let rim = app.world().resource::<Arena>().rim;
    // A rough ramp: columns of torso-sized blocks, each a block taller than the last and slightly askew, dropped loose
    // so they settle and freeze like a real pile.
    let columns = (rim / TORSO[1]).ceil() as usize + 1;
    for c in 0..columns {
        for layer in 0..=c {
            for side in [-1.0, 1.0] {
                let wobble = ((c * 7 + layer * 3) % 5) as f32 * 0.04 - 0.08;
                let at = Vec3::new(1.0 + 0.48 * c as f32, TORSO[1] * (layer as f32 + 0.5) + 0.01, -2.0 + side * 0.26);
                let turn = Quat::from_rotation_y(wobble);
                app.world_mut().spawn(piece(Solid::Cube(TORSO), 1, at, turn, 0.0));
            }
        }
    }
    step(&mut app, SETTLE + 1.0);
    let settled = pieces(&mut app);
    assert!(settled.iter().all(|(_, b)| *b == RigidBody::Static), "the pile settled and froze");
    let peak = settled.iter().fold(0.0f32, |m, (at, _)| m.max(at.y + TORSO[1] / 2.0));
    assert!(peak >= rim, "and stands up to the rim ({rim} m): {peak}");
    let score = app.world().resource::<RunScore>().0.points;
    // From the foot of the ramp, up it.
    teleport(&mut app, Vec3::new(-0.5, HEIGHT / 2.0 + 0.02, -2.0));
    step(&mut app, 0.2);
    press(&mut app, &[KeyCode::KeyD]);
    let mut top: f32 = 0.0;
    for _ in 0..(4.0 * FPS) as usize {
        step(&mut app, 1.0 / FPS);
        if box_number(&app) == 2 {
            break;
        }
        top = top.max(feet(&mut app));
    }
    keys(&mut app).release_all();
    assert_eq!(box_number(&app), 2, "escaped to box 2 (feet reached {top} of the rim's {rim})");
    step(&mut app, 0.1);
    assert!(pieces(&mut app).is_empty(), "the next box starts empty");
    assert!((player(&mut app).0.xz() - SPAWN.xz()).length() < 0.05, "with the player back at the start");
    assert!(app.world().resource::<RunScore>().0.points > score, "the time left scored a bonus");
    let next = app.world().resource::<Arena>().rim;
    assert!((next - game_core::run::rim_metres(&game_core::run::box_spec(2))).abs() < 1e-5, "box 2's rim: {next}");
    let mut walls = app.world_mut().query::<&Slab>();
    let tallest = walls.iter(app.world()).fold(0.0f32, |m, s| m.max(s.size.y));
    assert!((tallest - next).abs() < 1e-5, "the walls were rebuilt at the new rim: {tallest}");
}

/// `n` fodder burst one after another around `centre`, each standing on the heap so far, as kills in one spot would
/// leave them.
fn heap(app: &mut App, n: usize, centre: Vec2) {
    for i in 0..n {
        let a = i as f32 * 2.4;
        let r = 0.8 * ((i % 7) as f32 / 7.0).sqrt();
        let (x, z) = (centre.x + r * a.cos(), centre.y + r * a.sin());
        let mut q = app.world_mut().query::<(&Piece, &Transform)>();
        let lying: Vec<Placed> = q
            .iter(app.world())
            .map(|(p, t)| Placed { solid: p.solid, centre: t.translation.to_array(), rotation: t.rotation.to_array() })
            .collect();
        let ground = pile_height(&lying, [x, z], 0.35, 100.0);
        let at = Vec3::new(x, ground + FODDER_HEIGHT / 2.0 + 0.05, z);
        let e = app.world_mut().spawn(fodder(at, a)).id();
        app.update();
        app.world_mut().write_message(Defeated { entity: e, team: Team::Enemy, at });
        for _ in 0..15 {
            app.update();
        }
    }
}

#[test]
fn a_heap_of_bursts_built_and_climbed_at_30_fps_stays_put_under_the_player() {
    let mut app = standing();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 30.0)));
    let centre = Vec2::new(-6.6, -4.1);
    heap(&mut app, 25, centre);
    for _ in 0..((SETTLE + 6.0) * 30.0) as usize {
        app.update();
    }
    let before = pieces(&mut app);
    assert!(before.iter().all(|(_, b)| *b == RigidBody::Static), "the heap froze");
    // Walk at the heap from the open floor, up and across it, at 30 fps.
    teleport(&mut app, Vec3::new(centre.x + 4.0, HEIGHT / 2.0 + 0.02, centre.y + 3.0));
    for _ in 0..6 {
        app.update();
    }
    let mut top: f32 = 0.0;
    let mut lowest = f32::MAX;
    press(&mut app, &[KeyCode::KeyA, KeyCode::KeyW]);
    for _ in 0..120 {
        app.update();
        top = top.max(feet(&mut app));
        lowest = lowest.min(feet(&mut app));
    }
    keys(&mut app).release_all();
    assert!(top > 1.2, "climbed well up the heap: feet reached {top}");
    assert!(lowest > -0.02, "never sank into the floor: {lowest}");
    let after = pieces(&mut app);
    assert_eq!(after.len(), before.len(), "no piece was lost");
    let moved = before.iter().zip(&after).map(|(a, b)| a.0.distance(b.0)).fold(0.0f32, f32::max);
    assert!(moved < 1e-4, "frozen pieces don't budge under the player: {moved}");
}

#[test]
fn running_out_of_time_costs_health_and_on_the_last_of_it_ends_the_run() {
    let mut app = standing();
    app.world_mut().resource_mut::<RunState>().0.escape(100.0, 100.0);
    app.world_mut().resource_mut::<RunScore>().0.points = 900;
    app.world_mut().resource_mut::<RunState>().0.time_left = 0.05;
    step(&mut app, 0.1);
    let (hp, full) = player_hp(&mut app);
    assert_eq!(hp, full - game_core::run::TIMEOUT_PENALTY, "the timeout cost health");
    assert_eq!(box_number(&app), 2, "but the run goes on");
    assert!(app.world().resource::<RunState>().0.time_left > 60.0, "on a fresh clock");
    // Out of time again, on the last of the health.
    let mut q = app.world_mut().query_filtered::<&mut Hp, With<Player>>();
    q.single_mut(app.world_mut()).unwrap().0.hp = game_core::run::TIMEOUT_PENALTY;
    app.world_mut().resource_mut::<RunState>().0.time_left = 0.05;
    step(&mut app, 0.1);
    assert_eq!(box_number(&app), 1, "the run ended and a new one starts at box 1");
    let (hp, full) = player_hp(&mut app);
    assert_eq!(hp, full, "at full health");
    assert_eq!(app.world().resource::<RunScore>().0.points, 0, "with a fresh score");
}

#[test]
fn a_defeat_ends_the_run_and_the_next_starts_at_box_1_in_an_empty_box() {
    let mut app = standing();
    app.world_mut().resource_mut::<RunState>().0.escape(100.0, 100.0);
    app.world_mut().resource_mut::<RunScore>().0.points = 1_200;
    block(&mut app, TORSO, Vec3::new(3.0, 0.16, 0.0), 0.0);
    let mut q = app.world_mut().query_filtered::<&mut Hp, With<Player>>();
    q.single_mut(app.world_mut()).unwrap().0.hp = 1.0;
    fodder_at(&mut app, [1.0, 0.0]);
    let mut ended = false;
    for _ in 0..(4.0 * FPS) as usize {
        step(&mut app, 1.0 / FPS);
        if app.world().resource::<RunScore>().0.points == 0 {
            ended = true;
            break;
        }
    }
    assert!(ended, "the fodder's punch ended the run");
    step(&mut app, 0.1);
    assert_eq!(box_number(&app), 1);
    let (hp, full) = player_hp(&mut app);
    assert_eq!(hp, full, "at full health");
    assert!(enemies(&mut app).is_empty() && pieces(&mut app).is_empty(), "in an empty box");
    assert!((player(&mut app).0.xz() - SPAWN.xz()).length() < 0.05, "back at the start");
    let rim = app.world().resource::<Arena>().rim;
    assert_eq!(rim, game_core::run::rim_metres(&game_core::run::box_spec(1)), "box 1's rim again");
}

#[test]
fn the_demo_stairs_run_climbs_out_of_box_after_box() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, game3d::plugin, game3d::demo::stairs))
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / FPS)));
    app.finish();
    app.cleanup();
    let mut reached = vec![1];
    for _ in 0..(20.0 * FPS) as usize {
        step(&mut app, 1.0 / FPS);
        if reached.last() != Some(&box_number(&app)) {
            reached.push(box_number(&app));
        }
    }
    assert!(reached.starts_with(&[1, 2, 3]), "box after box: {reached:?}");
}

// Grab and spin-throw (G6).

fn carried(app: &App, e: Entity) -> bool {
    app.world().get::<Carried>(e).is_some()
}

fn holding(app: &mut App) -> Option<Holding> {
    app.world_mut().query_filtered::<&Holding, With<Player>>().single(app.world()).ok().copied()
}

fn at_of(app: &App, e: Entity) -> Vec3 {
    app.world().get::<Transform>(e).expect("still in the box").translation
}

fn body_of(app: &App, e: Entity) -> RigidBody {
    *app.world().get::<RigidBody>(e).expect("still in the box")
}

/// Stuns `e` for a few seconds, so it stands still as a target.
fn stun(app: &mut App, e: Entity) {
    app.world_mut().get_mut::<Hp>(e).unwrap().0.stun = 3.0;
}

/// A plane offset `dist` metres from the player at `deg` degrees off its facing (+x).
fn bearing(deg: f32, dist: f32) -> [f32; 2] {
    let a = deg.to_radians();
    [dist * a.cos(), dist * a.sin()]
}

#[test]
fn the_player_grabs_a_fodder_in_front_carries_it_overhead_and_sets_it_down() {
    let mut app = standing();
    let behind = fodder_at(&mut app, [-0.9, 0.0]);
    step(&mut app, 0.05);
    tap(&mut app, KeyCode::KeyL);
    assert!(!carried(&app, behind) && holding(&mut app).is_none(), "nothing in front: nothing grabbed");
    app.world_mut().despawn(behind);

    let held = fodder_at(&mut app, [0.9, 0.0]);
    step(&mut app, 0.05);
    tap(&mut app, KeyCode::KeyL);
    assert!(carried(&app, held), "grabbed the fodder in front");
    assert_eq!(holding(&mut app).map(|h| h.target), Some(held));
    // Carried about for a while, into the back-left corner: it rides overhead and can't fight.
    hold(&mut app, &[KeyCode::KeyW, KeyCode::KeyA], 1.5);
    step(&mut app, 0.5);
    let (me, _, _) = player(&mut app);
    let at = at_of(&app, held);
    assert!(at.xz().distance(me.xz()) < 0.05 && at.y > me.y + HEIGHT / 2.0, "overhead: {at}, player {me}");
    assert!(me.distance(SPAWN) > 2.0, "carried along: {me}");
    let (hp, full) = player_hp(&mut app);
    assert_eq!(hp, full, "a held fodder doesn't punch");
    // Set down in front (the corner's walls keep it in the box): it stands on the floor again, a fighter once more.
    tap(&mut app, KeyCode::KeyL);
    step(&mut app, 0.2);
    assert!(!carried(&app, held) && holding(&mut app).is_none(), "let go");
    assert_eq!(body_of(&app, held), RigidBody::Dynamic);
    let at = at_of(&app, held);
    let me = player(&mut app).0;
    assert!((at.y - FODDER_HEIGHT / 2.0).abs() < 0.1, "back on the floor: {at}");
    assert!(at.xz().distance(me.xz()) < 1.0, "set down by the player: {at}, player {me}");
    let half = Arena::default().floor / 2.0;
    assert!(at.x.abs() <= half.x && at.z.abs() <= half.y, "inside the box: {at}");
}

#[test]
fn a_spin_throw_through_a_line_of_fodder_hurts_each_once_and_the_thrown_one_once() {
    let mut app = standing();
    let held = fodder_at(&mut app, [0.9, 0.0]);
    step(&mut app, 0.05);
    tap(&mut app, KeyCode::KeyL);
    assert!(carried(&app, held));
    let line: Vec<Entity> = [3.0, 4.5, 6.0].into_iter().map(|x| fodder_at(&mut app, [x, 0.0])).collect();
    for &e in &line {
        stun(&mut app, e);
    }
    app.world_mut().get_mut::<Hp>(line[2]).unwrap().0.hp = 5.0;
    let start = player(&mut app).0;
    tap(&mut app, KeyCode::KeyJ);
    // The spin, the flight, and the hit-stops (real time) of three hits, one a finisher.
    step(&mut app, 1.6);
    let hit = impact(Held::Enemy);
    for (i, &e) in line[..2].iter().enumerate() {
        assert_eq!(hp(&app, e), FULL - hit.damage, "fodder {i} in the line was hurt exactly once");
    }
    assert!(app.world().get_entity(line[2]).is_err(), "the weakened one was defeated and burst");
    assert_eq!(pieces(&mut app).len(), FODDER.len(), "into its pieces");
    assert_eq!(app.world().resource::<RunScore>().0.points, game_core::score::KILL, "and the kill scored");
    assert_eq!(hp(&app, held), FULL - hit.self_damage, "the thrown fodder was hurt by its first impact only");
    let landed = at_of(&app, held);
    // It flew on through the line (and the burst's pieces, which it may bump off).
    assert!(landed.x > start.x + 4.0, "it flew along the facing: {landed}");
    assert!(!carried(&app, held) && app.world().get::<Thrown>(held).is_none(), "and landed");
}

#[test]
fn frozen_debris_thaws_when_grabbed_and_once_thrown_freezes_again() {
    let mut app = standing();
    let at = player(&mut app).0;
    let part = block(&mut app, TORSO, Vec3::new(at.x + 0.9, TORSO[1] / 2.0, at.z), 0.0);
    step(&mut app, 0.05);
    assert_eq!(body_of(&app, part), RigidBody::Static, "frozen on the floor");
    tap(&mut app, KeyCode::KeyL);
    assert!(carried(&app, part), "grabbed");
    assert_ne!(body_of(&app, part), RigidBody::Static, "and thawed");
    tap(&mut app, KeyCode::KeyK);
    step(&mut app, game_core::grab::SPIN + 0.05);
    assert!(app.world().get::<Thrown>(part).is_some(), "an attack spun it round and threw it");
    assert_eq!(body_of(&app, part), RigidBody::Dynamic);
    step(&mut app, game_core::grab::FLIGHT);
    assert!(app.world().get::<Thrown>(part).is_none(), "its flight ended");
    let flew = at_of(&app, part);
    assert!(flew.x > at.x + 4.0, "it flew: {flew} from {at}");
    // Back under the debris rules: it falls, rests and freezes.
    step(&mut app, SETTLE + 3.0);
    assert_eq!(body_of(&app, part), RigidBody::Static, "frozen again");
    let rest = at_of(&app, part);
    assert!(rest.y > 0.0 && rest.y < 0.5, "on the floor: {rest}");
    assert_eq!(pieces(&mut app).len(), 1);
}

#[test]
fn aim_assist_bends_a_throw_toward_the_biggest_group_within_30_degrees() {
    let mut app = standing();
    let at = player(&mut app).0;
    let part = block(&mut app, TORSO, Vec3::new(at.x + 0.9, TORSO[1] / 2.0, at.z), 0.0);
    step(&mut app, 0.05);
    tap(&mut app, KeyCode::KeyL);
    assert!(carried(&app, part));
    // A lone fodder straight ahead, a group of three 20 degrees to the left, and a bigger group 45 degrees off.
    let lone = fodder_at(&mut app, [3.0, 0.0]);
    let group: Vec<Entity> =
        [(18.0, 5.0), (22.0, 5.4), (20.0, 4.6)].into_iter().map(|(d, r)| fodder_at(&mut app, bearing(d, r))).collect();
    let wide: Vec<Entity> = [(45.0, 3.0), (47.0, 3.6), (43.0, 3.3), (46.0, 4.2)]
        .into_iter()
        .map(|(d, r)| fodder_at(&mut app, bearing(d, r)))
        .collect();
    for &e in group.iter().chain(&wide).chain([&lone]) {
        stun(&mut app, e);
    }
    tap(&mut app, KeyCode::KeyJ);
    let yaw = player(&mut app).2;
    assert!((yaw.to_degrees() - 20.0).abs() < 2.0, "turned toward the group: {}", yaw.to_degrees());
    step(&mut app, 1.0);
    let damage = impact(Held::Debris).damage;
    for &e in &group {
        assert_eq!(hp(&app, e), FULL - damage, "the throw went through the group");
    }
    assert_eq!(hp(&app, lone), FULL, "and past the lone one straight ahead");
    for &e in &wide {
        assert_eq!(hp(&app, e), FULL, "the group outside 30 degrees wasn't aimed at");
    }
}
