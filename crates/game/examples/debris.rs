//! SPIKE (ticket B1, not for main): how many debris parts the browser carries at 60 fps, and which freeze policy.
//!
//! Web: `debris.html?n=600&sub=4&policy=asleep` logs `BENCH {json}` lines to the console (update CPU time while
//! spawning / settling / settled). Native: `cargo run -p game --example debris -- soak` runs the whole matrix
//! headless for 60 simulated seconds per config, with a periodic kicker that wakes the pile, and reports panics.

use std::time::Duration;

use avian2d::prelude::*;
use bevy::platform::time::Instant;
use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Policy {
    /// Bodies stay dynamic; Avian's island sleeping is the only saving.
    None,
    /// A body becomes static as soon as Avian puts it to sleep.
    Asleep,
    /// A body becomes static N seconds after it spawned, asleep or not.
    After(f32),
}

#[derive(Resource, Clone, Copy, Debug)]
struct Cfg {
    n: usize,
    substeps: u32,
    policy: Policy,
    seed: u64,
    /// Fire a heavy ball into the pile every 1.5 s (wakes islands, like fights happening on the pile).
    kick: bool,
}

#[derive(Component)]
struct Debris {
    born: f32,
}

#[derive(Component)]
struct Kicker;

#[derive(Resource)]
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        // xorshift64*: deterministic per seed, no dependency.
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next()
    }
}

/// Per-frame CPU time of the whole update (all schedules), split by phase.
#[derive(Resource, Default)]
struct Stats {
    frame_start: Option<Instant>,
    spawned_at: Option<f32>,
    phases: [Vec<f32>; 3],
    intervals: [Vec<f32>; 3],
    reported: bool,
}

const PHASES: [&str; 3] = ["spawning", "settling", "settled"];
/// Parts per spawn batch (one defeated enemy) and batches per second.
const BATCH: usize = 10;
const BATCHES_PER_S: f32 = 10.0;

fn setup(app: &mut App, cfg: Cfg) {
    app.add_plugins(PhysicsPlugins::default().with_length_unit(32.0))
        .insert_resource(Gravity(Vec2::NEG_Y * game::GRAVITY))
        .insert_resource(SubstepCount(cfg.substeps))
        .insert_resource(cfg)
        .insert_resource(Rng(cfg.seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1))
        .init_resource::<Stats>()
        .add_plugins(game::arena::plugin)
        .add_systems(First, frame_start)
        .add_systems(Update, (spawn_debris, freeze, kick))
        .add_systems(Last, frame_end);
}

fn frame_start(mut stats: ResMut<Stats>) {
    stats.frame_start = Some(Instant::now());
}

fn frame_end(mut stats: ResMut<Stats>, time: Res<Time>, real: Res<Time<Real>>, cfg: Res<Cfg>, bodies: Query<(&RigidBody, Has<Sleeping>), With<Debris>>) {
    let Some(start) = stats.frame_start else { return };
    let ms = start.elapsed().as_secs_f32() * 1000.0;
    let t = time.elapsed_secs();
    let phase = match stats.spawned_at {
        None => 0,
        Some(s) if t < s + 3.0 => 1,
        Some(s) if t < s + 8.0 => 2,
        Some(_) => 3,
    };
    if phase < 3 {
        stats.phases[phase].push(ms);
        stats.intervals[phase].push(real.delta_secs() * 1000.0);
        return;
    }
    if stats.reported {
        return;
    }
    stats.reported = true;
    let (mut dynamic, mut sleeping, mut fixed) = (0, 0, 0);
    for (body, asleep) in &bodies {
        match body {
            RigidBody::Static => fixed += 1,
            _ if asleep => sleeping += 1,
            _ => dynamic += 1,
        }
    }
    let mut parts = Vec::new();
    for (i, name) in PHASES.iter().enumerate() {
        let (avg, p95) = summary(&mut stats.phases[i]);
        let (iv, _) = summary(&mut stats.intervals[i]);
        parts.push(format!("\"{name}\":{{\"avg_ms\":{avg:.2},\"p95_ms\":{p95:.2},\"interval_ms\":{iv:.2},\"frames\":{}}}", stats.phases[i].len()));
    }
    let line = format!(
        "BENCH {{\"n\":{},\"substeps\":{},\"policy\":\"{:?}\",\"awake\":{dynamic},\"asleep\":{sleeping},\"static\":{fixed},{}}}",
        cfg.n, cfg.substeps, cfg.policy, parts.join(",")
    );
    info!("{line}");
    println!("{line}");
}

fn summary(v: &mut [f32]) -> (f32, f32) {
    if v.is_empty() {
        return (0.0, 0.0);
    }
    let avg = v.iter().sum::<f32>() / v.len() as f32;
    let mut s = v.to_vec();
    s.sort_by(f32::total_cmp);
    (avg, s[((s.len() as f32 * 0.95) as usize).min(s.len() - 1)])
}

fn spawn_debris(mut commands: Commands, time: Res<Time>, cfg: Res<Cfg>, mut rng: ResMut<Rng>, mut stats: ResMut<Stats>, existing: Query<(), With<Debris>>) {
    let have = existing.iter().count();
    let due = ((time.elapsed_secs() * BATCHES_PER_S) as usize + 1) * BATCH;
    if have >= cfg.n {
        if stats.spawned_at.is_none() {
            stats.spawned_at = Some(time.elapsed_secs());
        }
        return;
    }
    let t = time.elapsed_secs();
    let half = game::ARENA / 2.0;
    for _ in have..due.min(cfg.n) {
        let size = Vec2::new(rng.range(10.0, 28.0), rng.range(10.0, 28.0));
        let pos = Vec2::new(rng.range(-half.x + 60.0, half.x - 60.0), half.y - 80.0 + rng.range(-20.0, 20.0));
        let hue = rng.range(0.0, 360.0);
        commands.spawn((
            Debris { born: t },
            Sprite::from_color(Color::hsl(hue, 0.9, 0.6), size),
            Transform::from_translation(pos.extend(0.0)).with_rotation(Quat::from_rotation_z(rng.range(-3.1, 3.1))),
            RigidBody::Dynamic,
            Collider::rectangle(size.x, size.y),
            LinearVelocity(Vec2::new(rng.range(-200.0, 200.0), rng.range(-100.0, 100.0))),
            AngularVelocity(rng.range(-6.0, 6.0)),
        ));
    }
}

fn freeze(mut commands: Commands, time: Res<Time>, cfg: Res<Cfg>, asleep: Query<Entity, (With<Debris>, Added<Sleeping>)>, debris: Query<(Entity, &Debris, &RigidBody)>) {
    match cfg.policy {
        Policy::None => {}
        Policy::Asleep => {
            for e in &asleep {
                commands.entity(e).insert(RigidBody::Static);
            }
        }
        Policy::After(s) => {
            for (e, d, body) in &debris {
                if *body == RigidBody::Dynamic && time.elapsed_secs() - d.born >= s {
                    commands.entity(e).insert(RigidBody::Static);
                }
            }
        }
    }
}

fn kick(mut commands: Commands, time: Res<Time>, cfg: Res<Cfg>, mut rng: ResMut<Rng>, kickers: Query<(Entity, &Transform), With<Kicker>>, mut last: Local<f32>) {
    if !cfg.kick {
        return;
    }
    for (e, t) in &kickers {
        if t.translation.y < -game::ARENA.y {
            commands.entity(e).despawn();
        }
    }
    if time.elapsed_secs() - *last < 1.5 {
        return;
    }
    *last = time.elapsed_secs();
    let side = if rng.next() < 0.5 { -1.0 } else { 1.0 };
    commands.spawn((
        Kicker,
        Sprite::from_color(Color::WHITE, Vec2::splat(40.0)),
        Transform::from_xyz(side * (game::ARENA.x / 2.0 - 80.0), rng.range(-200.0, 100.0), 1.0),
        RigidBody::Dynamic,
        Collider::circle(20.0),
        Mass(50.0),
        LinearVelocity(Vec2::new(-side * 900.0, rng.range(-200.0, 300.0))),
    ));
    // Kickers live 4 s, so a few are always crossing the pile.
    let old: Vec<Entity> = kickers.iter().map(|(e, _)| e).collect();
    if old.len() > 3 {
        commands.entity(old[0]).despawn();
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {
    let search = web_sys::window().and_then(|w| w.location().search().ok()).unwrap_or_default();
    let get = |k: &str| search.trim_start_matches('?').split('&').find_map(|kv| kv.strip_prefix(&format!("{k}="))).map(str::to_owned);
    let policy = match get("policy").as_deref() {
        Some("asleep") => Policy::Asleep,
        Some(p) if p.starts_with("after") => Policy::After(p[5..].parse().unwrap_or(3.0)),
        _ => Policy::None,
    };
    let cfg = Cfg {
        n: get("n").and_then(|v| v.parse().ok()).unwrap_or(300),
        substeps: get("sub").and_then(|v| v.parse().ok()).unwrap_or(6),
        policy,
        seed: 1,
        kick: get("kick").as_deref() != Some("0"),
    };
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window { canvas: Some("#game".into()), fit_canvas_to_parent: true, ..default() }),
        ..default()
    }))
    .insert_resource(ClearColor(Color::BLACK));
    setup(&mut app, cfg);
    app.run();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let mut failures = 0;
    for n in [100, 300, 600, 1000] {
        for substeps in [4, 6] {
            for policy in [Policy::None, Policy::Asleep, Policy::After(3.0)] {
                for seed in 1..=3 {
                    let cfg = Cfg { n, substeps, policy, seed, kick: true };
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        let mut app = App::new();
                        app.add_plugins((MinimalPlugins, TransformPlugin))
                            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 60.0)));
                        setup(&mut app, cfg);
                        app.finish();
                        app.cleanup();
                        for _ in 0..60 * 60 {
                            app.update();
                        }
                    }));
                    if result.is_err() {
                        failures += 1;
                        println!("PANIC {cfg:?}");
                    }
                }
            }
        }
    }
    println!("soak done: {failures} panicking configs");
}
