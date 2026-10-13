//! 3D spike (branch spike/3d, throwaway): can the game go full 3D like Rumble Box in the browser?
//!
//! An open box seen from above at ~45 degrees, cube-built figures, punches that burst enemies into their cubes,
//! and piles of resting cubes on avian3d. Query parameters (web) or the `BOX3D` env var (native, same syntax):
//!
//! - `cubes=N`    pre-fill the box with N resting cubes (edge 0.3-0.6) dropped from just above the floor
//! - `enemies=N`  at most N enemies at once (default 8; 0 with `bench=1`)
//! - `demo=1`     the player walks to the nearest enemy and punches by itself
//! - `burst=1`    knock a region of the pile loose every 2.5 s (with `bench=1`: only in the burst window)
//! - `bench=1`    settle, measure 10 s settled, then (with `burst=1`) 10 s of bursts, then re-settle; logs `BENCH {json}`
//! - `outline=W`  inverted-hull outline thickness per side (default 0.05, 0 = off); `bloom=1` HDR + bloom like the 2.5D game; `msaa=1|4`; `substeps=N`
//!
//! Timing (as in the B2 spike): `interval` is First-to-First wall time, `main` is First to Last of the main
//! schedule, `cpu` is First to the end of the render world, `phys` is the physics schedule's time in that frame.

use std::sync::{Arc, Mutex};

use avian3d::prelude::*;
use bevy::camera::Hdr;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::platform::time::Instant;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::render_resource::Face;
use bevy::render::view::Msaa;
use bevy::render::{Render, RenderApp, RenderSystems};
use bevy::window::WindowResolution;

/// Half the box's inner size in X and Z; the walls' height and thickness.
const HALF: f32 = 10.0;
const WALL_H: f32 = 3.0;
const WALL_T: f32 = 0.5;
/// The pile counts as settled when at most this fraction of debris is awake.
const SETTLED_AWAKE: f32 = 0.02;
const WINDOW_S: f32 = 10.0;
const WARMUP_S: f32 = 1.0;
const BURST_EVERY: f32 = 2.5;
const PLAYER_SPEED: f32 = 5.0;
const ENEMY_SPEED: f32 = 2.2;
const REACH: f32 = 1.1;

#[derive(Resource, Clone, Debug)]
struct Params {
    cubes: usize,
    enemies: usize,
    demo: bool,
    burst: bool,
    bench: bool,
    outline: f32,
    bloom: bool,
    msaa: u32,
    substeps: Option<u32>,
    raw: String,
}

impl Params {
    fn parse(q: &str) -> Self {
        let q = q.trim_start_matches('?');
        let get = |k: &str| {
            q.split('&').find_map(|kv| {
                let (a, b) = kv.split_once('=')?;
                (a == k).then(|| b.to_string())
            })
        };
        let num = |k: &str| get(k).and_then(|v| v.parse::<usize>().ok());
        let flag = |k: &str, d: bool| get(k).map(|v| v == "1" || v == "true").unwrap_or(d);
        let bench = flag("bench", false);
        Self {
            cubes: num("cubes").unwrap_or(0),
            enemies: num("enemies").unwrap_or(if bench { 0 } else { 8 }),
            demo: flag("demo", false),
            burst: flag("burst", false),
            bench,
            outline: get("outline").and_then(|v| v.parse().ok()).unwrap_or(0.05),
            bloom: flag("bloom", false),
            msaa: num("msaa").unwrap_or(4) as u32,
            substeps: num("substeps").map(|n| n as u32),
            raw: q.to_string(),
        }
    }
}

fn query_string() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window().and_then(|w| w.location().search().ok()).unwrap_or_default()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var("BOX3D").unwrap_or_default()
    }
}

fn emit(line: &str) {
    #[cfg(target_arch = "wasm32")]
    web_sys::console::log_1(&line.into());
    #[cfg(not(target_arch = "wasm32"))]
    println!("{line}");
}

/// Deterministic xorshift, so every run drops the same pile and bursts the same spots.
#[derive(Resource)]
struct Rng(u32);

impl Rng {
    fn unit(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.unit()
    }
    fn spin(&mut self, k: f32) -> Vec3 {
        Vec3::new(self.range(-k, k), self.range(-k, k), self.range(-k, k))
    }
}

/// Shared meshes and materials, so cubes batch.
#[derive(Resource)]
struct Kit {
    cube: Handle<Mesh>,
    ball: Handle<Mesh>,
    outline: Option<Handle<StandardMaterial>>,
    outline_w: f32,
    colours: Vec<Handle<StandardMaterial>>,
    white: Handle<StandardMaterial>,
    player: Handle<StandardMaterial>,
}

impl Kit {
    fn mesh(&self, ball: bool) -> Handle<Mesh> {
        if ball { self.ball.clone() } else { self.cube.clone() }
    }
}

#[derive(Component)]
struct Fighter {
    rig: Entity,
    facing: Vec3,
    walk: f32,
    punch: f32,
}

#[derive(Component)]
struct Player;

#[derive(Component)]
struct Enemy;

/// One primitive of a figure: a cube (edge `size`) or a ball (diameter `size`). `limb`: 1/-1 legs, 2/-2 arms
/// (sign = swing phase), 3 the punching fist, 0 rigid.
#[derive(Component, Clone, Copy)]
struct Part {
    size: f32,
    ball: bool,
    base: Vec3,
    limb: i8,
}

#[derive(Component)]
struct Debris;

#[derive(Component)]
struct Hud;

/// Figure layout relative to the capsule's centre (0.85 above the feet); `role` 0 body, 1 limb, 2 head.
const PARTS: [(f32, bool, [f32; 3], i8, u8); 11] = [
    (0.28, false, [-0.16, -0.71, 0.0], 1, 1),
    (0.30, false, [-0.16, -0.42, 0.0], 1, 1),
    (0.28, false, [0.16, -0.71, 0.0], -1, 1),
    (0.30, false, [0.16, -0.42, 0.0], -1, 1),
    (0.44, false, [0.0, -0.05, 0.0], 0, 0),
    (0.60, false, [0.0, 0.47, 0.0], 0, 0),
    (0.50, true, [0.0, 1.03, 0.0], 0, 2),
    (0.26, false, [-0.45, 0.62, 0.0], -2, 1),
    (0.24, false, [-0.45, 0.37, 0.0], -2, 1),
    (0.26, false, [0.45, 0.62, 0.0], 2, 1),
    (0.24, false, [0.45, 0.37, 0.0], 3, 1),
];

/// Timing shared between the main and render worlds (both single-threaded on wasm).
#[derive(Default)]
struct Clock {
    frame_start: Option<Instant>,
    main_end: Option<Instant>,
    render_end: Option<Instant>,
    phys_t0: Option<Instant>,
    phys_frame: f32,
    steps_frame: u32,
    recording: bool,
    interval: Vec<f32>,
    main: Vec<f32>,
    cpu: Vec<f32>,
    phys: Vec<f32>,
    steps: u32,
}

#[derive(Resource, Clone, Default)]
struct BenchClock(Arc<Mutex<Clock>>);

#[derive(Clone, Copy, PartialEq, Debug)]
enum Phase {
    Settling,
    Warmup,
    Settled,
    Burst,
    Resettling,
    Watching,
    Done,
}

#[derive(Resource)]
struct Bench {
    phase: Phase,
    since: Instant,
    next_burst: f32,
    bursts: u32,
    awake_sum: u64,
    awake_frames: u64,
    awake_max: usize,
}

fn main() {
    let params = Params::parse(&query_string());
    let clock = BenchClock::default();
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "box3d spike".into(),
            canvas: Some("#game".into()),
            fit_canvas_to_parent: true,
            prevent_default_event_handling: true,
            resolution: WindowResolution::new(1280, 720).with_scale_factor_override(1.0),
            present_mode: if cfg!(target_arch = "wasm32") {
                bevy::window::PresentMode::AutoVsync
            } else {
                bevy::window::PresentMode::AutoNoVsync
            },
            ..default()
        }),
        ..default()
    }))
    .add_plugins(PhysicsPlugins::default())
    .insert_resource(Gravity(Vec3::NEG_Y * 15.0))
    .insert_resource(ClearColor(Color::srgb(0.06, 0.07, 0.1)))
    .insert_resource(GlobalAmbientLight { color: Color::srgb(0.75, 0.82, 1.0), brightness: 350.0, ..default() })
    .insert_resource(Rng(0x9E37_79B9))
    .insert_resource(Bench {
        phase: Phase::Settling,
        since: Instant::now(),
        next_burst: 0.0,
        bursts: 0,
        awake_sum: 0,
        awake_frames: 0,
        awake_max: 0,
    })
    .insert_resource(clock.clone())
    .add_systems(Startup, (setup, prefill).chain())
    .add_systems(First, frame_start)
    .add_systems(FixedPostUpdate, phys_begin.in_set(PhysicsSystems::First))
    .add_systems(FixedPostUpdate, phys_end.in_set(PhysicsSystems::Last))
    .add_systems(Update, (spawn_enemies, steer, punch, animate, cull, bench_control, hud).chain())
    .add_systems(Last, main_end);
    if let Some(n) = params.substeps {
        app.insert_resource(SubstepCount(n));
    }
    app.insert_resource(params);
    app.sub_app_mut(RenderApp)
        .insert_resource(clock)
        .add_systems(Render, render_end.in_set(RenderSystems::PostCleanup));
    app.run();
}

fn flat(mesh: impl Into<Mesh>) -> Mesh {
    mesh.into().with_duplicated_vertices().with_computed_flat_normals()
}

fn setup(
    mut commands: Commands,
    params: Res<Params>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let mut mat = |c: Color| mats.add(StandardMaterial { base_color: c, perceptual_roughness: 0.85, ..default() });
    // Flat, bold colours.
    let colours = [
        Color::srgb(0.93, 0.22, 0.2),
        Color::srgb(1.0, 0.62, 0.1),
        Color::srgb(0.98, 0.85, 0.15),
        Color::srgb(0.25, 0.8, 0.3),
        Color::srgb(0.65, 0.35, 0.9),
        Color::srgb(0.95, 0.4, 0.7),
    ]
    .map(&mut mat)
    .to_vec();
    let white = mat(Color::srgb(0.95, 0.95, 0.92));
    let player = mat(Color::srgb(0.15, 0.45, 1.0));
    let floor = mat(Color::srgb(0.78, 0.74, 0.66));
    let wall = mat(Color::srgb(0.62, 0.68, 0.8));
    let rim = mat(Color::srgb(0.12, 0.13, 0.18));
    let outline = (params.outline > 0.0).then(|| {
        mats.add(StandardMaterial {
            base_color: Color::srgb(0.03, 0.03, 0.05),
            unlit: true,
            cull_mode: Some(Face::Front),
            ..default()
        })
    });
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let ball = meshes.add(flat(Sphere::new(0.5).mesh().ico(1).unwrap()));

    // The open box: a thick floor (top at y = 0) and four walls.
    let outer = 2.0 * (HALF + WALL_T);
    for (size, pos, m) in [
        (Vec3::new(outer, 1.0, outer), Vec3::new(0.0, -0.5, 0.0), &floor),
        (Vec3::new(outer, WALL_H, WALL_T), Vec3::new(0.0, WALL_H / 2.0, -HALF - WALL_T / 2.0), &wall),
        (Vec3::new(outer, WALL_H, WALL_T), Vec3::new(0.0, WALL_H / 2.0, HALF + WALL_T / 2.0), &wall),
        (Vec3::new(WALL_T, WALL_H, outer), Vec3::new(-HALF - WALL_T / 2.0, WALL_H / 2.0, 0.0), &wall),
        (Vec3::new(WALL_T, WALL_H, outer), Vec3::new(HALF + WALL_T / 2.0, WALL_H / 2.0, 0.0), &wall),
    ] {
        commands.spawn((
            RigidBody::Static,
            Collider::cuboid(size.x, size.y, size.z),
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(m.clone()),
            Transform::from_translation(pos),
        ));
    }
    // Dark rims on the wall tops, standing in for outlines on the box.
    for (size, pos) in [
        (Vec3::new(outer + 0.04, 0.08, WALL_T + 0.04), Vec3::new(0.0, WALL_H, -HALF - WALL_T / 2.0)),
        (Vec3::new(outer + 0.04, 0.08, WALL_T + 0.04), Vec3::new(0.0, WALL_H, HALF + WALL_T / 2.0)),
        (Vec3::new(WALL_T + 0.04, 0.08, outer + 0.04), Vec3::new(-HALF - WALL_T / 2.0, WALL_H, 0.0)),
        (Vec3::new(WALL_T + 0.04, 0.08, outer + 0.04), Vec3::new(HALF + WALL_T / 2.0, WALL_H, 0.0)),
    ] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(rim.clone()),
            Transform::from_translation(pos),
        ));
    }

    // One directional light (WebGL2 keeps only one), no shadow map.
    commands.spawn((
        DirectionalLight { illuminance: 9_000.0, shadow_maps_enabled: false, ..default() },
        Transform::from_xyz(0.0, 14.0, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    // 3/4 view: ~45 degrees down at the whole box.
    let mut cam = commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection { fov: 42f32.to_radians(), ..default() }),
        Transform::from_xyz(0.0, 22.0, 17.0).looking_at(Vec3::new(0.0, 0.0, 0.8), Vec3::Y),
        if params.msaa == 1 { Msaa::Off } else { Msaa::Sample4 },
        Tonemapping::AcesFitted,
    ));
    if params.bloom {
        cam.insert((Hdr, Bloom::NATURAL));
    }

    commands.spawn((
        Hud,
        Text::new(""),
        TextFont::from_font_size(16.0),
        TextColor(Color::WHITE),
        Node { position_type: PositionType::Absolute, top: Val::Px(8.0), left: Val::Px(10.0), ..default() },
    ));

    let kit = Kit { cube, ball, outline, outline_w: params.outline, colours, white, player };
    let p = kit.player.clone();
    let w = kit.white.clone();
    spawn_fighter(&mut commands, &kit, Vec3::new(0.0, 0.9, 3.0), p, w, true);
    commands.insert_resource(kit);
}

/// Mesh + material, plus an inverted-hull outline child when outlines are on.
/// The outline is a fixed world thickness (outline_w per side), so the hull scale depends on the part size.
fn spawn_visual(e: &mut EntityCommands, kit: &Kit, size: f32, ball: bool, mat: Handle<StandardMaterial>) {
    e.insert((Mesh3d(kit.mesh(ball)), MeshMaterial3d(mat)));
    if let Some(outline) = &kit.outline {
        e.with_child((
            Mesh3d(kit.mesh(ball)),
            MeshMaterial3d(outline.clone()),
            Transform::from_scale(Vec3::splat(1.0 + 2.0 * kit.outline_w / size)),
        ));
    }
}

fn spawn_fighter(
    commands: &mut Commands,
    kit: &Kit,
    at: Vec3,
    body: Handle<StandardMaterial>,
    limb: Handle<StandardMaterial>,
    player: bool,
) -> Entity {
    let rig = commands.spawn((Transform::default(), Visibility::default())).id();
    for (size, ball, base, l, role) in PARTS {
        let base = Vec3::from(base);
        let mat = if role == 1 { limb.clone() } else { body.clone() };
        let mut e = commands.spawn((
            Part { size, ball, base, limb: l },
            Transform::from_translation(base).with_scale(Vec3::splat(size)),
            ChildOf(rig),
        ));
        spawn_visual(&mut e, kit, size, ball, mat);
    }
    let mut f = commands.spawn((
        Fighter { rig, facing: Vec3::Z, walk: 0.0, punch: 0.0 },
        RigidBody::Dynamic,
        Collider::capsule(0.35, 1.0),
        LockedAxes::ROTATION_LOCKED,
        // Frictionless, so walls and piles don't hold fighters; horizontal control is set on velocity.
        Friction::ZERO.with_combine_rule(CoefficientCombine::Min),
        SleepingDisabled,
        TransformInterpolation,
        Transform::from_translation(at),
        Visibility::default(),
    ));
    f.add_child(rig);
    if player {
        f.insert(Player);
    } else {
        f.insert(Enemy);
    }
    f.id()
}

#[allow(clippy::too_many_arguments)]
fn spawn_debris(
    commands: &mut Commands,
    kit: &Kit,
    mat: Handle<StandardMaterial>,
    size: f32,
    ball: bool,
    at: Vec3,
    rot: Quat,
    lin: Vec3,
    ang: Vec3,
) {
    let collider = if ball { Collider::sphere(0.5) } else { Collider::cuboid(1.0, 1.0, 1.0) };
    // The body's scale scales the unit collider and the shared unit mesh together.
    let mut e = commands.spawn((
        Debris,
        RigidBody::Dynamic,
        collider,
        Friction::new(0.7),
        Restitution::new(0.05),
        Transform { translation: at, rotation: rot, scale: Vec3::splat(size) },
        LinearVelocity(lin),
        AngularVelocity(ang),
    ));
    spawn_visual(&mut e, kit, size, ball, mat);
}

/// `?cubes=N`: a grid of cubes dropped from just above the floor, layer on layer, so they settle into a pile.
fn prefill(mut commands: Commands, kit: Res<Kit>, params: Res<Params>, mut rng: ResMut<Rng>) {
    let step = 0.75;
    let per_row = ((2.0 * HALF - 1.0) / step) as usize;
    for i in 0..params.cubes {
        let layer = i / (per_row * per_row);
        // Cells in a scattered order (7919 is coprime to the cell count), so any N covers the whole floor.
        let cell = (i * 7919) % (per_row * per_row);
        let x = -HALF + 0.5 + step * (cell % per_row) as f32 + step / 2.0 + rng.range(-0.08, 0.08);
        let z = -HALF + 0.5 + step * (cell / per_row) as f32 + step / 2.0 + rng.range(-0.08, 0.08);
        let y = 0.6 + layer as f32 * 0.8;
        let size = rng.range(0.3, 0.6);
        let rot = Quat::from_euler(EulerRot::XYZ, rng.range(0.0, 3.0), rng.range(0.0, 3.0), rng.range(0.0, 3.0));
        let mat = kit.colours[(rng.unit() * kit.colours.len() as f32) as usize % kit.colours.len()].clone();
        spawn_debris(&mut commands, &kit, mat, size, false, Vec3::new(x, y, z), rot, Vec3::ZERO, Vec3::ZERO);
    }
}

fn spawn_enemies(
    mut commands: Commands,
    kit: Res<Kit>,
    params: Res<Params>,
    time: Res<Time>,
    mut rng: ResMut<Rng>,
    mut next: Local<f32>,
    enemies: Query<(), With<Enemy>>,
) {
    if time.elapsed_secs() < *next || enemies.iter().len() >= params.enemies {
        return;
    }
    *next = time.elapsed_secs() + 1.2;
    // Drop in over a wall, just inside it.
    let t = rng.range(-HALF + 1.5, HALF - 1.5);
    let edge = HALF - 0.8;
    let at = match (rng.unit() * 4.0) as u32 {
        0 => Vec3::new(t, 3.8, -edge),
        1 => Vec3::new(t, 3.8, edge),
        2 => Vec3::new(-edge, 3.8, t),
        _ => Vec3::new(edge, 3.8, t),
    };
    let c = (rng.unit() * kit.colours.len() as f32) as usize % kit.colours.len();
    let body = kit.colours[c].clone();
    let limb = kit.colours[(c + 3) % kit.colours.len()].clone();
    spawn_fighter(&mut commands, &kit, at, body, limb, false);
}

fn flat_dir(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z).normalize_or_zero()
}

fn steer(
    keys: Res<ButtonInput<KeyCode>>,
    params: Res<Params>,
    time: Res<Time>,
    mut player: Query<(&Transform, &mut LinearVelocity, &mut Fighter), With<Player>>,
    mut enemies: Query<(&Transform, &mut LinearVelocity, &mut Fighter), (With<Enemy>, Without<Player>)>,
) {
    let dt = time.delta_secs();
    let foes: Vec<Vec3> = enemies.iter().map(|(t, ..)| t.translation).collect();
    let Ok((pt, mut pv, mut pf)) = player.single_mut() else { return };
    let me = pt.translation;
    let mut dir = Vec3::ZERO;
    for (k, d) in [
        (KeyCode::KeyW, Vec3::NEG_Z),
        (KeyCode::ArrowUp, Vec3::NEG_Z),
        (KeyCode::KeyS, Vec3::Z),
        (KeyCode::ArrowDown, Vec3::Z),
        (KeyCode::KeyA, Vec3::NEG_X),
        (KeyCode::ArrowLeft, Vec3::NEG_X),
        (KeyCode::KeyD, Vec3::X),
        (KeyCode::ArrowRight, Vec3::X),
    ] {
        if keys.pressed(k) {
            dir += d;
        }
    }
    if params.demo {
        if let Some(target) = foes.iter().min_by(|a, b| a.distance_squared(me).total_cmp(&b.distance_squared(me))) {
            let to = flat_dir(*target - me);
            pf.facing = if to == Vec3::ZERO { pf.facing } else { to };
            if (*target - me).with_y(0.0).length() > REACH * 0.8 {
                dir = to;
            }
        }
    }
    let dir = dir.normalize_or_zero();
    move_fighter(&mut pv, &mut pf, dir, PLAYER_SPEED, dt);
    for (t, mut v, mut f) in &mut enemies {
        let to = (me - t.translation).with_y(0.0);
        let dir = if to.length() > 0.9 { to.normalize() } else { Vec3::ZERO };
        if to.length() > 0.01 {
            f.facing = to.normalize();
        }
        move_fighter(&mut v, &mut f, dir, ENEMY_SPEED, dt);
    }
}

fn move_fighter(v: &mut LinearVelocity, f: &mut Fighter, dir: Vec3, speed: f32, dt: f32) {
    v.x = dir.x * speed;
    v.z = dir.z * speed;
    if dir != Vec3::ZERO {
        f.facing = dir;
        f.walk += dt * speed * 3.0;
    }
    f.punch = (f.punch - dt).max(0.0);
}

#[allow(clippy::too_many_arguments)]
fn punch(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    params: Res<Params>,
    kit: Res<Kit>,
    mut rng: ResMut<Rng>,
    mut player: Query<(&Transform, &mut Fighter), With<Player>>,
    enemies: Query<(Entity, &Transform, &Fighter, &LinearVelocity), (With<Enemy>, Without<Player>)>,
    kids: Query<&Children>,
    parts: Query<(&Part, &GlobalTransform, &MeshMaterial3d<StandardMaterial>)>,
    mut debris: Query<(&Transform, &mut LinearVelocity, &mut AngularVelocity), (With<Debris>, Without<Fighter>)>,
) {
    let Ok((pt, mut pf)) = player.single_mut() else { return };
    let fist = pt.translation + pf.facing * 0.7;
    let in_reach = |t: &Transform| (t.translation - fist).with_y(0.0).length() < REACH;
    let wants = keys.just_pressed(KeyCode::Space) || (params.demo && enemies.iter().any(|(_, t, ..)| in_reach(t)));
    if !wants || pf.punch > 0.0 {
        return;
    }
    pf.punch = 0.3;
    let facing = pf.facing;
    // Loose cubes in front of the fist get knocked too.
    for (t, mut v, mut w) in &mut debris {
        if (t.translation - fist).length() < 1.0 {
            v.0 += facing * 4.0 + Vec3::Y * 2.0;
            w.0 += rng.spin(6.0);
        }
    }
    for (e, t, f, v) in &enemies {
        if !in_reach(t) {
            continue;
        }
        // Defeated: burst into its parts, which keep their place and fly outward with spin.
        let centre = t.translation;
        if let Ok(children) = kids.get(f.rig) {
            for &c in children {
                let Ok((part, gt, mat)) = parts.get(c) else { continue };
                let (_, rot, at) = gt.to_scale_rotation_translation();
                let out = flat_dir(at - centre) * 2.5 + facing * 4.5 + Vec3::Y * 3.5 + rng.spin(1.0) + v.0 * 0.3;
                spawn_debris(&mut commands, &kit, mat.0.clone(), part.size, part.ball, at, rot, out, rng.spin(12.0));
            }
        }
        commands.entity(e).despawn();
    }
}

fn animate(
    fighters: Query<(&Fighter, &LinearVelocity)>,
    kids: Query<&Children>,
    mut parts: Query<(&Part, &mut Transform)>,
    mut rigs: Query<&mut Transform, Without<Part>>,
) {
    for (f, v) in &fighters {
        if let Ok(mut rt) = rigs.get_mut(f.rig) {
            rt.rotation = Quat::from_rotation_y(f.facing.x.atan2(f.facing.z));
        }
        let moving = v.0.with_y(0.0).length() > 0.1;
        let swing = if moving { f.walk.sin() * 0.18 } else { 0.0 };
        let jab = if f.punch > 0.0 { (f.punch / 0.3 * std::f32::consts::PI).sin() * 0.5 } else { 0.0 };
        let Ok(children) = kids.get(f.rig) else { continue };
        for &c in children {
            let Ok((p, mut t)) = parts.get_mut(c) else { continue };
            let dz = match p.limb {
                0 => 0.0,
                3 => jab - swing,
                l => swing * l.signum() as f32 * if l.abs() == 2 { -1.0 } else { 1.0 },
            };
            t.translation = p.base + Vec3::Z * dz;
        }
    }
}

/// Anything that left the box downward is gone; a fighter that did is put back.
fn cull(
    mut commands: Commands,
    mut bodies: Query<(Entity, &mut Transform, Has<Debris>), Or<(With<Debris>, With<Fighter>)>>,
) {
    for (e, mut t, debris) in &mut bodies {
        if t.translation.y < -5.0 {
            if debris {
                commands.entity(e).despawn();
            } else {
                t.translation = Vec3::new(0.0, 3.0, 0.0);
            }
        }
    }
}

fn burst(
    rng: &mut Rng,
    debris: &mut Query<(&Transform, &mut LinearVelocity, &mut AngularVelocity), With<Debris>>,
) -> usize {
    let at = Vec3::new(rng.range(-6.0, 6.0), 0.0, rng.range(-6.0, 6.0));
    let mut n = 0;
    for (t, mut v, mut w) in debris.iter_mut() {
        let d = (t.translation - at).with_y(0.0);
        if d.length() < 4.0 {
            v.0 = d.normalize_or_zero() * 3.0 + Vec3::Y * rng.range(4.0, 7.0);
            w.0 = rng.spin(10.0);
            n += 1;
        }
    }
    n
}

#[allow(clippy::too_many_arguments)]
fn bench_control(
    params: Res<Params>,
    clock: Res<BenchClock>,
    time: Res<Time>,
    substeps: Res<SubstepCount>,
    mut bench: ResMut<Bench>,
    mut rng: ResMut<Rng>,
    mut debris: Query<(&Transform, &mut LinearVelocity, &mut AngularVelocity), With<Debris>>,
    sleepers: Query<Has<Sleeping>, With<Debris>>,
    mut next_burst: Local<f32>,
) {
    if !params.bench {
        if params.burst && time.elapsed_secs() > 3.0 && time.elapsed_secs() >= *next_burst {
            *next_burst = time.elapsed_secs() + BURST_EVERY;
            burst(&mut rng, &mut debris);
        }
        return;
    }
    let total = sleepers.iter().len();
    let awake = sleepers.iter().filter(|s| !s).count();
    let frac = if total == 0 { 0.0 } else { awake as f32 / total as f32 };
    let now = Instant::now();
    let t = (now - bench.since).as_secs_f32();
    let head = format!("\"q\":\"{}\",\"cubes\":{},\"substeps\":{}", params.raw, params.cubes, substeps.0);
    if matches!(bench.phase, Phase::Settled | Phase::Burst) {
        bench.awake_sum += awake as u64;
        bench.awake_frames += 1;
    }
    let go = |b: &mut Bench, p: Phase| {
        b.phase = p;
        b.since = now;
    };
    match bench.phase {
        Phase::Settling | Phase::Resettling if t > 0.5 && (frac <= SETTLED_AWAKE || t > 30.0) => {
            let event = if bench.phase == Phase::Settling { "settled" } else { "resettled" };
            emit(&format!(
                "BENCH {{{head},\"event\":\"{event}\",\"after_s\":{t:.2},\"awake\":{awake},\"total\":{total}}}"
            ));
            let next = if bench.phase == Phase::Settling { Phase::Warmup } else { Phase::Watching };
            bench.awake_max = 0;
            go(&mut bench, next);
        }
        Phase::Warmup if t >= WARMUP_S => {
            clock.0.lock().unwrap().start();
            go(&mut bench, Phase::Settled);
        }
        Phase::Settled if t >= WINDOW_S => {
            report(&clock, &mut bench, &head, "settled");
            if params.burst {
                bench.next_burst = 0.0;
                clock.0.lock().unwrap().start();
                go(&mut bench, Phase::Burst);
            } else {
                go(&mut bench, Phase::Done);
                emit("BENCH_DONE");
            }
        }
        Phase::Burst => {
            if t >= bench.next_burst {
                bench.next_burst += BURST_EVERY;
                bench.bursts += burst(&mut rng, &mut debris) as u32;
            }
            if t >= WINDOW_S {
                report(&clock, &mut bench, &head, "burst");
                go(&mut bench, Phase::Resettling);
            }
        }
        // Do resting piles stay asleep with nothing touching them?
        Phase::Watching => {
            bench.awake_max = bench.awake_max.max(awake);
            if t >= 5.0 {
                emit(&format!(
                    "BENCH {{{head},\"event\":\"watch\",\"secs\":5,\"awake_max\":{},\"awake_end\":{awake},\"total\":{total}}}",
                    bench.awake_max
                ));
                go(&mut bench, Phase::Done);
                emit("BENCH_DONE");
                #[cfg(not(target_arch = "wasm32"))]
                std::process::exit(0);
            }
        }
        _ => {}
    }
}

fn report(clock: &BenchClock, bench: &mut Bench, head: &str, phase: &str) {
    let mut guard = clock.0.lock().unwrap();
    let c = &mut *guard;
    c.recording = false;
    let stat = |v: &mut Vec<f32>| {
        v.sort_by(|a, b| a.total_cmp(b));
        let avg = v.iter().sum::<f32>() / v.len().max(1) as f32;
        let p95 = v.get(v.len() * 95 / 100).copied().unwrap_or(0.0);
        let max = v.last().copied().unwrap_or(0.0);
        let over = v.iter().filter(|&&x| x > 1000.0 / 60.0).count();
        format!("{{\"avg\":{avg:.3},\"p95\":{p95:.3},\"max\":{max:.3},\"over_16_7\":{over}}}")
    };
    let frames = c.interval.len();
    let awake_avg = bench.awake_sum as f32 / bench.awake_frames.max(1) as f32;
    let line = format!(
        "BENCH {{{head},\"phase\":\"{phase}\",\"frames\":{frames},\"steps\":{},\"awake_avg\":{awake_avg:.1},\"burst_hits\":{},\"interval_ms\":{},\"main_ms\":{},\"cpu_ms\":{},\"phys_ms_per_frame\":{}}}",
        c.steps,
        bench.bursts,
        stat(&mut c.interval),
        stat(&mut c.main),
        stat(&mut c.cpu),
        stat(&mut c.phys),
    );
    emit(&line);
    bench.awake_sum = 0;
    bench.awake_frames = 0;
    bench.bursts = 0;
}

impl Clock {
    fn start(&mut self) {
        self.interval.clear();
        self.main.clear();
        self.cpu.clear();
        self.phys.clear();
        self.steps = 0;
        self.recording = true;
    }
}

fn hud(
    time: Res<Time>,
    mut fps: Local<f32>,
    sleepers: Query<Has<Sleeping>, With<Debris>>,
    enemies: Query<(), With<Enemy>>,
    mut text: Query<&mut Text, With<Hud>>,
) {
    let dt = time.delta_secs().max(1e-4);
    *fps = if *fps == 0.0 { 1.0 / dt } else { *fps * 0.95 + 0.05 / dt };
    let total = sleepers.iter().len();
    let awake = sleepers.iter().filter(|s| !s).count();
    if let Ok(mut t) = text.single_mut() {
        t.0 = format!(
            "{:.0} fps   cubes {total} ({awake} awake)   enemies {}   WASD/arrows move, Space punch",
            *fps,
            enemies.iter().len()
        );
    }
}

fn frame_start(clock: Res<BenchClock>) {
    let now = Instant::now();
    let mut guard = clock.0.lock().unwrap();
    let c = &mut *guard;
    if let (Some(start), Some(me), true) = (c.frame_start, c.main_end, c.recording) {
        c.interval.push((now - start).as_secs_f32() * 1e3);
        c.main.push((me - start).as_secs_f32() * 1e3);
        if let Some(re) = c.render_end.filter(|re| *re > start) {
            c.cpu.push((re - start).as_secs_f32() * 1e3);
        }
        let (pf, sf) = (c.phys_frame, c.steps_frame);
        c.phys.push(pf);
        c.steps += sf;
    }
    c.phys_frame = 0.0;
    c.steps_frame = 0;
    c.frame_start = Some(now);
}

fn phys_begin(clock: Res<BenchClock>) {
    clock.0.lock().unwrap().phys_t0 = Some(Instant::now());
}

fn phys_end(clock: Res<BenchClock>) {
    let mut c = clock.0.lock().unwrap();
    if let Some(t0) = c.phys_t0.take() {
        c.phys_frame += t0.elapsed().as_secs_f32() * 1e3;
        c.steps_frame += 1;
    }
}

fn main_end(clock: Res<BenchClock>) {
    clock.0.lock().unwrap().main_end = Some(Instant::now());
}

fn render_end(clock: Res<BenchClock>) {
    clock.0.lock().unwrap().render_end = Some(Instant::now());
}
