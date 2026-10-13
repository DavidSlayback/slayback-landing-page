//! Headless game scenarios (test seam 2): the real game systems in an app with no window or renderer, stepped
//! frame by frame on a fixed clock with scripted keyboard input. They assert what a player would see happen.

use std::time::Duration;

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use game::ARENA;
use game::arena::{WALL, Wall};
use game::bomb::{BombHead, LitBomb, lit_bomb};
use game::debris::{DebrisPart, part};
use game::enemy::{Brain, MAX_HP, Waves, enemy, fodder};
use game::fighter::Hp;
use game::flow::Phase;
use game::grab::{Carried, Thrown};
use game::player::{MAX_HP as PLAYER_MAX_HP, Player, SIZE, SPAWN};
use game::run::RunState;
use game::score::{LastRun, RunScore};
use game_core::debris::{FREEZE_AFTER, MAX_PARTS_PER_ENEMY, Shape};
use game_core::enemy::{Kind, THUG};
use game_core::movement::{DASH_SPEED, DASH_TIME};
use game_core::run::{Run, TIMEOUT_PENALTY, box_spec};

const FPS: f32 = 60.0;

/// The y the player's centre settles at when standing on the floor.
const STANDING_Y: f32 = -ARENA.y / 2.0 + WALL + SIZE.y / 2.0;

/// A game app without window or renderer. Keyboard state is driven by the test (no input plugin clears it), and
/// enemies only appear where a test puts them.
fn headless() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, game::plugin))
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
fn a_dash_covers_its_distance_along_the_floor_and_the_player_stays_grounded() {
    let mut app = standing();
    let (start, _, _) = player(&mut app);
    keys(&mut app).press(KeyCode::Space);
    for _ in 0..30 {
        step(&mut app, 1.0 / FPS);
        keys(&mut app).release(KeyCode::Space);
        let (pos, _, grounded) = player(&mut app);
        assert!(grounded && (pos.y - STANDING_Y).abs() < 2.0, "stays on the floor while dashing: y {}", pos.y);
    }
    let (end, vel, _) = player(&mut app);
    let expected = DASH_SPEED * DASH_TIME;
    assert!((end.x - start.x - expected).abs() < 25.0, "dashed {} to the right, expected ~{expected}", end.x - start.x);
    assert_eq!(vel.x, 0.0, "stops after the dash when no direction is held");
}

#[test]
fn there_is_no_jump() {
    let mut app = standing();
    for key in [KeyCode::KeyW, KeyCode::ArrowUp, KeyCode::Space] {
        keys(&mut app).press(key);
        for _ in 0..20 {
            step(&mut app, 1.0 / FPS);
            keys(&mut app).release(key);
            let y = player(&mut app).0.y;
            assert!(y < STANDING_Y + 2.0, "{key:?} lifted the player to {y}");
        }
        step(&mut app, 0.6); // past the dash cooldown
    }
}

/// Puts a fodder on the floor `dx` from the player (the player faces right after `standing()`).
fn fodder_at(app: &mut App, dx: f32) -> Entity {
    let x = player(app).0.x + dx;
    app.world_mut().spawn(fodder(Vec2::new(x, STANDING_Y - 2.0))).id()
}

fn fodder_state(app: &App, e: Entity) -> (f32, Vec2) {
    let hp = app.world().get::<Hp>(e).expect("fodder still in the box").0.hp;
    (hp, app.world().get::<LinearVelocity>(e).unwrap().0)
}

#[test]
fn a_punch_lands_on_fodder_in_range_and_not_out_of_range() {
    let mut app = standing();
    let near = fodder_at(&mut app, 50.0);
    let far = fodder_at(&mut app, 220.0);
    step(&mut app, 0.05);
    keys(&mut app).press(KeyCode::KeyJ);
    step(&mut app, 0.2);
    let (near_hp, near_v) = fodder_state(&app, near);
    assert!(near_hp < MAX_HP, "the punch hurt the fodder in range: {near_hp}");
    assert!(near_v.x > 200.0, "and knocked it away: {near_v}");
    assert_eq!(fodder_state(&app, far).0, MAX_HP, "the fodder out of range is untouched");
}

#[test]
fn attacking_during_a_dash_is_a_drop_kick_that_launches_harder_than_a_punch() {
    let mut app = standing();
    let target = fodder_at(&mut app, 140.0);
    keys(&mut app).press(KeyCode::Space);
    step(&mut app, 1.0 / 60.0);
    keys(&mut app).release(KeyCode::Space);
    keys(&mut app).press(KeyCode::KeyJ);
    let mut fastest: f32 = 0.0;
    for _ in 0..20 {
        step(&mut app, 1.0 / 60.0);
        keys(&mut app).release(KeyCode::KeyJ);
        fastest = fastest.max(fodder_state(&app, target).1.x);
    }
    let punch = game_core::combat::Move::Punch.spec().knockback.0;
    assert!(fodder_state(&app, target).0 < MAX_HP, "the drop-kick landed");
    assert!(fastest > punch, "launched at {fastest}, harder than a punch ({punch})");
}

#[test]
fn fodder_enter_through_a_door_only_after_its_warning() {
    let mut app = standing();
    app.world_mut().resource_mut::<Waves>().enabled = true;
    let count = |app: &mut App| app.world_mut().query_filtered::<(), With<Brain>>().iter(app.world()).count();
    let mut doors = app.world_mut().query::<&game::enemy::WallDoor>();
    let mut warned_for = 0.0;
    let mut first_entry = None;
    for frame in 0..(4.0 * FPS) as usize {
        step(&mut app, 1.0 / FPS);
        if doors.iter(app.world()).any(|d| d.door.warning()) {
            warned_for += 1.0 / FPS;
        }
        if first_entry.is_none() && count(&mut app) > 0 {
            first_entry = Some(frame);
        }
    }
    assert!(first_entry.is_some(), "fodder entered within 4 s");
    assert!(warned_for >= game_core::doors::WARNING - 0.05, "a door warned for {warned_for} s before it opened");
}

fn debris_parts(app: &mut App) -> Vec<(Vec2, RigidBody)> {
    let mut q = app.world_mut().query::<(&DebrisPart, &Transform, &RigidBody)>();
    q.iter(app.world()).map(|(_, t, b)| (t.translation.truncate(), *b)).collect()
}

/// Punches a nearly beaten fodder until it breaks apart.
fn defeat_fodder(app: &mut App) {
    let target = fodder_at(app, 50.0);
    app.world_mut().get_mut::<Hp>(target).unwrap().0.hp = 1.0;
    step(app, 0.05);
    keys(app).press(KeyCode::KeyJ);
    step(app, 0.3);
    assert!(app.world().get_entity(target).is_err(), "the defeated fodder is gone");
}

#[test]
fn a_defeated_enemy_leaves_its_capped_number_of_solid_parts() {
    let mut app = standing();
    defeat_fodder(&mut app);
    assert_eq!(debris_parts(&mut app).len(), MAX_PARTS_PER_ENEMY);
    step(&mut app, 1.5);
    let floor_top = -ARENA.y / 2.0 + WALL;
    for (pos, _) in debris_parts(&mut app) {
        assert!(pos.y > floor_top && pos.y < floor_top + 40.0, "parts rest on the floor, not through it: {pos}");
    }
}

#[test]
fn debris_freezes_once_settled_per_the_spike_rule() {
    let mut app = standing();
    defeat_fodder(&mut app);
    step(&mut app, FREEZE_AFTER + 0.2);
    let parts = debris_parts(&mut app);
    assert!(parts.iter().all(|(_, b)| *b == RigidBody::Static), "all parts frozen: {parts:?}");
}

#[test]
fn the_player_walks_up_a_ramp_of_debris() {
    let mut app = standing();
    // A staircase of frozen parts in front of the player: 12-unit steps, 36 wide.
    let floor_top = -ARENA.y / 2.0 + WALL;
    for i in 0..6 {
        let h = 12.0 * (i + 1) as f32;
        let at = Vec2::new(60.0 + 36.0 * i as f32, floor_top + h / 2.0);
        app.world_mut().spawn(part(Shape::Box { w: 36.0, h }, at, 0.0)).insert(RigidBody::Static);
    }
    keys(&mut app).press(KeyCode::KeyD);
    let mut highest = f32::MIN;
    for _ in 0..(2.0 * FPS) as usize {
        step(&mut app, 1.0 / FPS);
        highest = highest.max(player(&mut app).0.y);
    }
    assert!(highest > STANDING_Y + 50.0, "climbed to {highest} from {STANDING_Y}");
}

fn run_state(app: &App) -> Run {
    app.world().resource::<RunState>().0
}

fn player_hp(app: &mut App) -> f32 {
    let mut q = app.world_mut().query_filtered::<&Hp, With<Player>>();
    q.single(app.world()).unwrap().0.hp
}

fn right_wall_height(app: &mut App) -> f32 {
    let mut q = app.world_mut().query::<(&Wall, &Name)>();
    q.iter(app.world()).find(|(_, n)| n.as_str() == "right wall").expect("a right wall").0.0.y
}

#[test]
fn building_the_pile_under_the_player_to_the_rim_moves_on_to_a_taller_box() {
    let mut app = standing();
    let rim = run_state(&app).spec.rim;
    assert_eq!(right_wall_height(&mut app), rim);
    // A staircase of frozen debris, 12-unit steps, up past the rim.
    let floor_top = -ARENA.y / 2.0 + WALL;
    let steps = (rim / 12.0).ceil() as usize + 1;
    for i in 0..steps {
        let h = 12.0 * (i + 1) as f32;
        let at = Vec2::new(50.0 + 36.0 * i as f32, floor_top + h / 2.0);
        app.world_mut().spawn(part(Shape::Box { w: 36.0, h }, at, 0.0)).insert(RigidBody::Static);
    }
    keys(&mut app).press(KeyCode::KeyD);
    for _ in 0..(4.0 * FPS) as usize {
        step(&mut app, 1.0 / FPS);
        if run_state(&app).spec.number > 1 {
            break;
        }
    }
    keys(&mut app).release(KeyCode::KeyD);
    assert_eq!(run_state(&app).spec.number, 2, "escaped to box 2");
    step(&mut app, 0.1);
    assert!(debris_parts(&mut app).is_empty(), "the new box starts empty");
    assert_eq!(right_wall_height(&mut app), box_spec(2).rim, "and its walls are taller");
    assert!((player(&mut app).0.x - SPAWN.x).abs() < 5.0, "the player starts over at the spawn point");
}

#[test]
fn running_out_of_time_costs_health_but_a_healthy_player_carries_on() {
    let mut app = standing();
    app.world_mut().resource_mut::<RunState>().0.time_left = 0.01;
    step(&mut app, 0.05);
    assert_eq!(player_hp(&mut app), PLAYER_MAX_HP - TIMEOUT_PENALTY);
    assert_eq!(run_state(&app).spec.number, 1);
    assert!(run_state(&app).time_left > box_spec(1).timer - 1.0, "the clock restarted");
}

#[test]
fn death_shows_the_game_over_screen_and_enter_starts_a_fresh_run() {
    let mut app = standing();
    app.world_mut().resource_mut::<RunState>().0.escape(PLAYER_MAX_HP, PLAYER_MAX_HP);
    let mut q = app.world_mut().query_filtered::<&mut Hp, With<Player>>();
    q.single_mut(app.world_mut()).unwrap().0.hp = 5.0;
    app.world_mut().resource_mut::<RunState>().0.time_left = 0.01;
    step(&mut app, 0.1);
    assert_eq!(*app.world().resource::<State<Phase>>().get(), Phase::Over, "game over");
    let last = *app.world().resource::<LastRun>();
    assert_eq!(last.reached, 2, "the screen knows how far the run got");

    keys(&mut app).press(KeyCode::Enter);
    step(&mut app, 0.1);
    assert_eq!(*app.world().resource::<State<Phase>>().get(), Phase::Playing);
    assert_eq!(run_state(&app).spec.number, 1, "back to box 1");
    assert_eq!(player_hp(&mut app), PLAYER_MAX_HP);
}

fn body_of(app: &App, e: Entity) -> RigidBody {
    *app.world().get::<RigidBody>(e).expect("still exists")
}

#[test]
fn a_grabbed_frozen_part_thaws_and_once_thrown_lands_as_debris() {
    let mut app = standing();
    let floor_top = -ARENA.y / 2.0 + WALL;
    let start = player(&mut app).0.x + 32.0;
    let piece = app
        .world_mut()
        .spawn(part(Shape::Box { w: 20.0, h: 10.0 }, Vec2::new(start, floor_top + 5.0), 0.0))
        .insert(RigidBody::Static)
        .id();
    step(&mut app, 0.05);
    keys(&mut app).press(KeyCode::KeyL);
    step(&mut app, 1.0 / FPS);
    keys(&mut app).release(KeyCode::KeyL);
    assert_ne!(body_of(&app, piece), RigidBody::Static, "grabbing thaws it");
    assert!(app.world().get::<Carried>(piece).is_some(), "and the player holds it");

    keys(&mut app).press(KeyCode::KeyJ);
    step(&mut app, 0.3);
    keys(&mut app).release(KeyCode::KeyJ);
    assert!(app.world().get::<Thrown>(piece).is_some(), "an attack throws it");
    assert_eq!(body_of(&app, piece), RigidBody::Dynamic);

    step(&mut app, 1.2);
    let x = app.world().get::<Transform>(piece).unwrap().translation.x;
    assert!(x > start + 120.0, "it flew: {x} from {start}");
    assert!(app.world().get::<Thrown>(piece).is_none(), "and landed");
    step(&mut app, FREEZE_AFTER);
    assert_eq!(body_of(&app, piece), RigidBody::Static, "it freezes again as debris");
}

#[test]
fn a_thrown_enemy_damages_the_enemies_it_hits() {
    let mut app = standing();
    let held = fodder_at(&mut app, 45.0);
    let target = fodder_at(&mut app, 260.0);
    step(&mut app, 0.05);
    keys(&mut app).press(KeyCode::KeyL);
    step(&mut app, 1.0 / FPS);
    keys(&mut app).release(KeyCode::KeyL);
    assert!(app.world().get::<Carried>(held).is_some(), "grabbed the near fodder");
    let start = player(&mut app).0.x;
    keys(&mut app).press(KeyCode::KeyJ);
    let mut farthest = f32::MIN;
    for _ in 0..(1.0 * FPS) as usize {
        step(&mut app, 1.0 / FPS);
        keys(&mut app).release(KeyCode::KeyJ);
        farthest = farthest.max(app.world().get::<Transform>(held).unwrap().translation.x);
    }
    assert!(farthest > start + 200.0, "the throw carried the fodder well away: {farthest} from {start}");
    let (hp, _) = fodder_state(&app, target);
    assert!(hp < MAX_HP, "the thrown fodder hurt the other one: {hp}");
    assert!(fodder_state(&app, held).0 < MAX_HP, "and was hurt by the impact itself");
}

#[test]
fn escape_pauses_the_clock_and_resumes_it() {
    let mut app = standing();
    let before = run_state(&app).time_left;
    keys(&mut app).press(KeyCode::Escape);
    step(&mut app, 1.0);
    assert_eq!(*app.world().resource::<State<Phase>>().get(), Phase::Paused);
    assert!((run_state(&app).time_left - before).abs() < 0.05, "the box timer stood still while paused");
    keys(&mut app).release(KeyCode::Escape);
    step(&mut app, 1.0 / FPS);
    keys(&mut app).press(KeyCode::Escape);
    step(&mut app, 1.0);
    assert!(run_state(&app).time_left < before - 0.5, "and runs again after Esc");
}

#[test]
fn kills_score_points_and_a_new_run_starts_from_zero() {
    let mut app = standing();
    defeat_fodder(&mut app);
    assert_eq!(app.world().resource::<RunScore>().0.points, game_core::score::KILL);
    app.world_mut().resource_mut::<NextState<Phase>>().set(Phase::Over);
    step(&mut app, 0.05);
    keys(&mut app).press(KeyCode::Enter);
    step(&mut app, 0.05);
    assert_eq!(app.world().resource::<RunScore>().0.points, 0);
}

#[test]
fn a_punch_lands_even_when_a_slow_frame_skips_its_active_window() {
    let mut app = standing();
    let target = fodder_at(&mut app, 50.0);
    step(&mut app, 0.05);
    keys(&mut app).press(KeyCode::KeyJ);
    step(&mut app, 1.0 / FPS); // the punch starts
    // One frame of 250 ms (Bevy's longest step): past the whole active window.
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(0.25)));
    app.update();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / FPS)));
    step(&mut app, 0.1);
    assert!(fodder_state(&app, target).0 < MAX_HP, "the punch still landed");
}

/// A thug standing just in front of the player, already winding up.
fn winding_thug(app: &mut App) -> Entity {
    let x = player(app).0.x + 60.0;
    let thug = app.world_mut().spawn(enemy(Kind::Thug, Vec2::new(x, -ARENA.y / 2.0 + WALL + 31.0))).id();
    step(app, 0.2);
    assert!(thug_telegraph(app, thug) > 0.0, "the thug is winding up");
    thug
}

fn thug_telegraph(app: &App, thug: Entity) -> f32 {
    app.world().get::<Brain>(thug).unwrap().0.telegraph()
}

#[test]
fn punches_hurt_a_thug_but_dont_interrupt_its_wind_up() {
    let mut app = standing();
    let thug = winding_thug(&mut app);
    keys(&mut app).press(KeyCode::KeyJ);
    step(&mut app, 0.15);
    let hp = app.world().get::<Hp>(thug).unwrap().0;
    assert!(hp.hp < THUG.max_hp, "the punch hurt");
    assert!(!hp.stunned() && thug_telegraph(&app, thug) > 0.4, "but the wind-up carried on");
}

#[test]
fn a_kick_interrupts_a_thugs_wind_up() {
    let mut app = standing();
    let thug = winding_thug(&mut app);
    keys(&mut app).press(KeyCode::KeyK);
    step(&mut app, 0.25);
    assert!(app.world().get::<Hp>(thug).unwrap().0.stunned(), "a kick staggers it");
    assert_eq!(thug_telegraph(&app, thug), 0.0, "and resets its wind-up");
}

#[test]
fn doors_deliver_thugs_when_the_box_calls_for_them() {
    let mut app = standing();
    app.world_mut().resource_mut::<RunState>().0.spec.thugs = 1.0;
    app.world_mut().resource_mut::<Waves>().enabled = true;
    step(&mut app, 3.0);
    let mut q = app.world_mut().query::<&Brain>();
    let kinds: Vec<Kind> = q.iter(app.world()).map(|b| b.0.style().kind).collect();
    assert!(!kinds.is_empty() && kinds.iter().all(|k| *k == Kind::Thug), "entered: {kinds:?}");
}

fn x_of(app: &App, e: Entity) -> f32 {
    app.world().get::<Transform>(e).unwrap().translation.x
}

#[test]
fn a_ninja_strikes_from_behind_and_backs_off() {
    let mut app = standing();
    let floor = -ARENA.y / 2.0 + WALL;
    let start = player(&mut app).0.x;
    // Behind the player (who faces right).
    let ninja = app.world_mut().spawn(enemy(Kind::Ninja, Vec2::new(start - 150.0, floor + 25.0))).id();
    let mut closest = f32::MAX;
    let mut hurt_at = None;
    for frame in 0..(2.0 * FPS) as usize {
        step(&mut app, 1.0 / FPS);
        closest = closest.min((x_of(&app, ninja) - player(&mut app).0.x).abs());
        if hurt_at.is_none() && player_hp(&mut app) < PLAYER_MAX_HP {
            hurt_at = Some(frame);
        }
    }
    assert!(hurt_at.is_some_and(|f| f < 60), "it closed in and struck within a second: {hurt_at:?}");
    assert!(closest < 60.0);
    let gap = (x_of(&app, ninja) - player(&mut app).0.x).abs();
    assert!(gap > closest + 30.0, "then backed off: {gap} vs closest {closest}");
}

#[test]
fn a_ninja_runs_through_rather_than_standing_in_front() {
    let mut app = standing();
    let floor = -ARENA.y / 2.0 + WALL;
    let start = player(&mut app).0.x;
    // In front of the player (who faces right), inside its avoid range.
    let ninja = app.world_mut().spawn(enemy(Kind::Ninja, Vec2::new(start + 120.0, floor + 25.0))).id();
    let mut got_behind = false;
    for _ in 0..FPS as usize {
        step(&mut app, 1.0 / FPS);
        got_behind |= x_of(&app, ninja) < player(&mut app).0.x;
    }
    assert!(got_behind, "it darted through to the player's back");
}

fn lit_bombs(app: &mut App) -> Vec<Entity> {
    let mut q = app.world_mut().query_filtered::<Entity, With<LitBomb>>();
    q.iter(app.world()).collect()
}

#[test]
fn a_blast_knocks_debris_loose_without_deleting_any() {
    let mut app = standing();
    let floor = -ARENA.y / 2.0 + WALL;
    // A frozen pile around x = 300, plus one part far away.
    let near: Vec<Entity> = (0..8)
        .map(|i| {
            app.world_mut()
                .spawn(part(Shape::Box { w: 18.0, h: 10.0 }, Vec2::new(260.0 + 12.0 * i as f32, floor + 5.0), 0.0))
                .insert(RigidBody::Static)
                .id()
        })
        .collect();
    let far = app
        .world_mut()
        .spawn(part(Shape::Box { w: 18.0, h: 10.0 }, Vec2::new(-500.0, floor + 5.0), 0.0))
        .insert(RigidBody::Static)
        .id();
    let before = debris_parts(&mut app).len();
    let bomb = app.world_mut().spawn(lit_bomb(Vec2::new(300.0, floor + 30.0), Vec2::ZERO)).id();
    app.world_mut().get_mut::<LitBomb>(bomb).unwrap().fuse.left = 0.05;
    step(&mut app, 0.1);
    assert!(app.world().get_entity(bomb).is_err(), "the bomb went off");
    assert_eq!(debris_parts(&mut app).len(), before, "no debris was deleted");
    assert!(near.iter().all(|e| body_of(&app, *e) == RigidBody::Dynamic), "the pile in range was knocked loose");
    assert_eq!(body_of(&app, far), RigidBody::Static, "debris out of range stayed put");
}

#[test]
fn a_defeated_bomb_head_drops_a_lit_bomb_instead_of_its_head() {
    let mut app = standing();
    let target = fodder_at(&mut app, 50.0);
    app.world_mut().entity_mut(target).insert(BombHead);
    app.world_mut().get_mut::<Hp>(target).unwrap().0.hp = 1.0;
    step(&mut app, 0.05);
    keys(&mut app).press(KeyCode::KeyJ);
    step(&mut app, 0.3);
    assert_eq!(debris_parts(&mut app).len(), MAX_PARTS_PER_ENEMY - 1, "five parts join the pile");
    assert_eq!(lit_bombs(&mut app).len(), 1, "and the head is a lit bomb");
}

#[test]
fn a_lit_bomb_can_be_grabbed_and_thrown() {
    let mut app = standing();
    let floor = -ARENA.y / 2.0 + WALL;
    let start = player(&mut app).0.x;
    let bomb = app.world_mut().spawn(lit_bomb(Vec2::new(start + 28.0, floor + 10.0), Vec2::ZERO)).id();
    step(&mut app, 0.05);
    keys(&mut app).press(KeyCode::KeyL);
    step(&mut app, 1.0 / FPS);
    keys(&mut app).release(KeyCode::KeyL);
    assert!(app.world().get::<Carried>(bomb).is_some(), "grabbed the lit bomb");
    keys(&mut app).press(KeyCode::KeyJ);
    let mut farthest = f32::MIN;
    for _ in 0..(0.8 * FPS) as usize {
        step(&mut app, 1.0 / FPS);
        keys(&mut app).release(KeyCode::KeyJ);
        if let Some(t) = app.world().get::<Transform>(bomb) {
            farthest = farthest.max(t.translation.x);
        }
    }
    assert!(farthest > start + 150.0, "thrown: {farthest} from {start}");
    step(&mut app, 1.0);
    assert!(lit_bombs(&mut app).is_empty(), "and it went off");
}
