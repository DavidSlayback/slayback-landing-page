//! B2 spike (not merged): what the 2.5D look costs. A 1280x720-style box as 3D walls and floor, a tilted
//! perspective camera, N primitive humanoids, a directional light plus point lights, HDR + bloom, emissive accents.
//!
//! Variants come from the URL query on the web (`?n=60&shadows=0&bloom=1&msaa=4&lit=1&points=2&tm=aces`) or the
//! `LOOK` env var natively (same syntax). After a warmup it logs `BENCH {json}` lines: CPU time per frame for the
//! main schedule and for main + render (First of the main world to the end of the render world's schedule), and
//! the frame interval (time between successive First runs).
//!
//! Build: `cargo build -p game --profile game --target wasm32-unknown-unknown --example look`, then
//! `bash crates/game/examples/look/ship.sh target/wasm32-unknown-unknown/game/examples/look.wasm <dir> look`, copy
//! `look/index.html` into `<dir>`, serve it, and run `look/bench.mjs` (see its header). Results: `look/results/`.

use std::sync::{Arc, Mutex};

use bevy::camera::Hdr;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::light::CascadeShadowConfigBuilder;
use bevy::platform::time::Instant;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::Msaa;
use bevy::render::{Render, RenderApp, RenderSystems};

/// Box in world units: 1 unit = 32 px of the 2D arena (1280x720 -> 40 x 22.5), depth 6.
const W: f32 = 40.0;
const H: f32 = 22.5;
const D: f32 = 6.0;
const WARMUP: usize = 180;
const WINDOW: usize = 600;

#[derive(Resource, Clone, Debug)]
struct Params {
    n: usize,
    shadows: bool,
    bloom: bool,
    msaa: u32,
    lit: bool,
    points: usize,
    tm: String,
    raw: String,
}

impl Params {
    fn parse(q: &str) -> Self {
        let get = |k: &str| {
            q.trim_start_matches('?').split('&').find_map(|kv| {
                let (a, b) = kv.split_once('=')?;
                (a == k).then(|| b.to_string())
            })
        };
        let num = |k: &str, d: usize| get(k).and_then(|v| v.parse().ok()).unwrap_or(d);
        let flag = |k: &str, d: bool| get(k).map(|v| v == "1" || v == "true").unwrap_or(d);
        Self {
            n: num("n", 30),
            shadows: flag("shadows", true),
            bloom: flag("bloom", true),
            msaa: num("msaa", 4) as u32,
            lit: flag("lit", true),
            points: num("points", 2),
            tm: get("tm").unwrap_or_else(|| "aces".into()),
            raw: q.trim_start_matches('?').to_string(),
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
        std::env::var("LOOK").unwrap_or_default()
    }
}

fn emit(line: &str) {
    #[cfg(target_arch = "wasm32")]
    web_sys::console::log_1(&line.into());
    #[cfg(not(target_arch = "wasm32"))]
    println!("{line}");
}

/// Shared between the main and render worlds (both single-threaded on wasm).
#[derive(Default)]
struct Clock {
    frame_start: Option<Instant>,
    main_end: Option<Instant>,
    render_end: Option<Instant>,
    frames: usize,
    main: Vec<f32>,
    total: Vec<f32>,
    interval: Vec<f32>,
    reports: usize,
}

#[derive(Resource, Clone, Default)]
struct BenchClock(Arc<Mutex<Clock>>);

#[derive(Component)]
struct Figure {
    home: Vec3,
    phase: f32,
    speed: f32,
}

#[derive(Component)]
struct Limb(f32);

fn main() {
    let params = Params::parse(&query_string());
    let clock = BenchClock::default();
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "look spike".into(),
            canvas: Some("#game".into()),
            fit_canvas_to_parent: true,
            resolution: (1280, 720).into(),
            present_mode: if cfg!(target_arch = "wasm32") {
                bevy::window::PresentMode::AutoVsync
            } else {
                bevy::window::PresentMode::AutoNoVsync
            },
            ..default()
        }),
        ..default()
    }))
    .insert_resource(ClearColor(Color::BLACK))
    .insert_resource(GlobalAmbientLight { color: Color::WHITE, brightness: 120.0, ..default() })
    .insert_resource(params)
    .insert_resource(clock.clone())
    .add_systems(Startup, setup)
    .add_systems(First, frame_start)
    .add_systems(Update, animate)
    .add_systems(Last, main_end);
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
    let p = &*params;
    let mat = |mats: &mut Assets<StandardMaterial>, c: Color, emissive: LinearRgba| {
        mats.add(StandardMaterial {
            base_color: c,
            emissive,
            perceptual_roughness: 0.85,
            metallic: 0.0,
            unlit: !p.lit,
            ..default()
        })
    };

    // Netrunner palette (site/hero.js): lime, green, teal, yellow-lime.
    let lime = Color::srgb(0.65, 1.0, 0.0);
    let palette = [lime, Color::srgb(0.15, 1.0, 0.2), Color::srgb(0.0, 1.0, 0.72), Color::srgb(0.88, 1.0, 0.12)];

    // The box: floor, back wall, side walls, and glowing rim strips.
    let wall = mat(&mut mats, Color::srgb(0.05, 0.07, 0.05), LinearRgba::BLACK);
    let floor = mat(&mut mats, Color::srgb(0.08, 0.1, 0.07), LinearRgba::BLACK);
    let rim = mat(&mut mats, lime, LinearRgba::rgb(2.6, 4.0, 0.0));
    let t = 0.5;
    for (size, pos, m) in [
        (Vec3::new(W + 2.0 * t, t, D), Vec3::new(0.0, -t / 2.0, 0.0), &floor),
        (Vec3::new(W + 2.0 * t, H, t), Vec3::new(0.0, H / 2.0, -D / 2.0 - t / 2.0), &wall),
        (Vec3::new(t, H, D), Vec3::new(-W / 2.0 - t / 2.0, H / 2.0, 0.0), &wall),
        (Vec3::new(t, H, D), Vec3::new(W / 2.0 + t / 2.0, H / 2.0, 0.0), &wall),
        (Vec3::new(W + 2.0 * t, 0.15, 0.15), Vec3::new(0.0, H, -D / 2.0), &rim),
        (Vec3::new(0.15, 0.15, D), Vec3::new(-W / 2.0 - t / 2.0, H, 0.0), &rim),
        (Vec3::new(0.15, 0.15, D), Vec3::new(W / 2.0 + t / 2.0, H, 0.0), &rim),
        (Vec3::new(W, 0.08, 0.08), Vec3::new(0.0, 0.0, D / 2.0), &rim),
    ] {
        commands.spawn((Mesh3d(meshes.add(flat(Cuboid::from_size(size)))), MeshMaterial3d(m.clone()), Transform::from_translation(pos)));
    }

    // Shared meshes and a few shared materials, so figures batch.
    let torso = meshes.add(flat(Cuboid::new(0.7, 1.0, 0.4)));
    let head = meshes.add(flat(Sphere::new(0.32).mesh().ico(1).unwrap()));
    let limb = meshes.add(flat(Cuboid::new(0.22, 0.75, 0.22)));
    let visor = meshes.add(flat(Cuboid::new(0.4, 0.08, 0.1)));
    let bodies: Vec<_> = palette.iter().map(|&c| mat(&mut mats, c.darker(0.55), LinearRgba::BLACK)).collect();
    let glows: Vec<_> = palette.iter().map(|&c| mat(&mut mats, c, c.to_linear() * 6.0)).collect();
    let skin = mat(&mut mats, Color::srgb(0.85, 0.9, 0.8), LinearRgba::BLACK);

    for i in 0..p.n {
        // Deterministic scatter across the floor and a few stacked rows, like a busy box.
        let h = (i as f32 * 0.618_034).fract();
        let x = -W / 2.0 + 1.5 + h * (W - 3.0);
        let z = ((i * 7) % 5) as f32 * 0.9 - 1.8;
        let y = if i % 4 == 3 { 3.0 + (i % 3) as f32 * 2.5 } else { 0.0 };
        let home = Vec3::new(x, y + 1.2, z);
        let c = i % palette.len();
        commands
            .spawn((
                Transform::from_translation(home),
                Visibility::default(),
                Figure { home, phase: i as f32 * 0.37, speed: 0.6 + (i % 5) as f32 * 0.15 },
            ))
            .with_children(|f| {
                f.spawn((Mesh3d(torso.clone()), MeshMaterial3d(bodies[c].clone()), Transform::from_xyz(0.0, 0.5, 0.0)));
                f.spawn((Mesh3d(head.clone()), MeshMaterial3d(skin.clone()), Transform::from_xyz(0.0, 1.35, 0.0)));
                f.spawn((Mesh3d(visor.clone()), MeshMaterial3d(glows[c].clone()), Transform::from_xyz(0.0, 1.4, 0.3)));
                for (dx, dy, swing, m) in [
                    (-0.48, 0.45, 1.0, &bodies[c]),
                    (0.48, 0.45, -1.0, &bodies[c]),
                    (-0.18, -0.4, -1.0, &glows[c]),
                    (0.18, -0.4, 1.0, &bodies[c]),
                ] {
                    f.spawn((Mesh3d(limb.clone()), MeshMaterial3d(m.clone()), Transform::from_xyz(dx, dy, 0.0), Limb(swing)));
                }
            });
    }

    // One directional key light, optional shadows (one cascade: WebGL2 supports only one).
    commands.spawn((
        DirectionalLight { illuminance: 6_000.0, shadow_maps_enabled: p.shadows, ..default() },
        Transform::from_xyz(-10.0, 30.0, 25.0).looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y),
        CascadeShadowConfigBuilder { num_cascades: 1, maximum_distance: 90.0, ..default() }.build(),
    ));
    for k in 0..p.points {
        let x = if p.points == 1 { 0.0 } else { -W / 3.0 + k as f32 * (2.0 * W / 3.0) / (p.points - 1) as f32 };
        commands.spawn((
            PointLight {
                color: palette[k % palette.len()],
                intensity: 2.0e6,
                range: 30.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_xyz(x, H * 0.7, 1.5),
        ));
    }

    // Slightly tilted side view showing the whole box.
    let mut cam = commands.spawn((
        Camera3d::default(),
        Hdr,
        Projection::Perspective(PerspectiveProjection { fov: 30f32.to_radians(), ..default() }),
        Transform::from_xyz(0.0, 16.0, 50.0).looking_at(Vec3::new(0.0, 10.0, 0.0), Vec3::Y),
        match p.msaa {
            1 => Msaa::Off,
            2 => Msaa::Sample2,
            8 => Msaa::Sample8,
            _ => Msaa::Sample4,
        },
        match p.tm.as_str() {
            "tony" => Tonemapping::TonyMcMapface,
            "reinhard" => Tonemapping::ReinhardLuminance,
            "none" => Tonemapping::None,
            _ => Tonemapping::AcesFitted,
        },
    ));
    if p.bloom {
        cam.insert(Bloom::NATURAL);
    }
}

fn animate(time: Res<Time>, mut figs: Query<(&Figure, &mut Transform, &Children)>, mut limbs: Query<(&Limb, &mut Transform), Without<Figure>>) {
    let t = time.elapsed_secs();
    for (f, mut tf, children) in &mut figs {
        let s = t * f.speed + f.phase;
        tf.translation = f.home + Vec3::new((s * 0.5).sin() * 1.5, (s * 4.0).sin().abs() * 0.15, 0.0);
        tf.rotation = Quat::from_rotation_y(if (s * 0.5).cos() > 0.0 { 0.5 } else { -0.5 });
        for &c in children {
            if let Ok((l, mut lt)) = limbs.get_mut(c) {
                lt.rotation = Quat::from_rotation_x((s * 4.0).sin() * 0.7 * l.0);
            }
        }
    }
}

fn frame_start(clock: Res<BenchClock>, params: Res<Params>) {
    let now = Instant::now();
    let mut guard = clock.0.lock().unwrap();
    let c = &mut *guard;
    if let (Some(start), Some(me)) = (c.frame_start, c.main_end) {
        c.frames += 1;
        if c.frames > WARMUP {
            c.main.push((me - start).as_secs_f32() * 1e3);
            if let Some(re) = c.render_end.filter(|re| *re > start) {
                c.total.push((re - start).as_secs_f32() * 1e3);
            }
            c.interval.push((now - start).as_secs_f32() * 1e3);
        }
        if c.interval.len() >= WINDOW {
            c.reports += 1;
            let stat = |v: &mut Vec<f32>| {
                v.sort_by(|a, b| a.total_cmp(b));
                let avg = v.iter().sum::<f32>() / v.len().max(1) as f32;
                let p95 = v.get(v.len() * 95 / 100).copied().unwrap_or(0.0);
                format!("{{\"avg\":{avg:.3},\"p95\":{p95:.3}}}")
            };
            let line = format!(
                "BENCH {{\"q\":\"{}\",\"n\":{},\"shadows\":{},\"bloom\":{},\"msaa\":{},\"lit\":{},\"points\":{},\"tm\":\"{}\",\"report\":{},\"frames\":{},\"main_ms\":{},\"cpu_ms\":{},\"interval_ms\":{}}}",
                params.raw, params.n, params.shadows, params.bloom, params.msaa, params.lit, params.points, params.tm,
                c.reports, WINDOW, stat(&mut c.main), stat(&mut c.total), stat(&mut c.interval)
            );
            emit(&line);
            #[cfg(not(target_arch = "wasm32"))]
            if c.reports >= 3 {
                std::process::exit(0);
            }
            c.main.clear();
            c.total.clear();
            c.interval.clear();
        }
    }
    c.frame_start = Some(now);
}

fn main_end(clock: Res<BenchClock>) {
    clock.0.lock().unwrap().main_end = Some(Instant::now());
}

fn render_end(clock: Res<BenchClock>) {
    clock.0.lock().unwrap().render_end = Some(Instant::now());
}
