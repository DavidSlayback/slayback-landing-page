//! The 2.5D look (ticket D1): the 2D game drawn as lit, flat-shaded 3D primitives in a slightly tilted perspective,
//! with emissive neon and bloom in the page's colour scheme.
//!
//! Gameplay entities carry no visuals; this plugin dresses them when they appear, so the headless scenarios run the
//! same game without assets or a renderer. Choices and budget come from the B2 spike (docs/research/game-research.md,
//! section 6): one directional light with a single shadow cascade, low ambient, no point lights, ACES tonemapping,
//! bloom and MSAA 4x, a backbuffer capped at 1280x720 by the host page.

use avian2d::prelude::LinearVelocity;
use bevy::camera::Hdr;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::light::CascadeShadowConfigBuilder;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::Msaa;
use game_core::movement::RUN_SPEED;

use crate::ARENA;
use crate::arena::{WALL, Wall};
use crate::player::{Player, SIZE};

/// Depth of the box, world units (the 2D game lives on the z = 0 plane).
pub const DEPTH: f32 = 192.0;

/// A colour scheme shared with the homepage hero (`site/hero.js`): four neon accents, the first one primary.
#[derive(Resource, Clone, Copy, Debug)]
pub struct Scheme {
    pub key: &'static str,
    pub accents: [Color; 4],
}

pub const SCHEMES: [Scheme; 3] = [
    Scheme {
        key: "netrunner",
        accents: [Color::srgb(0.62, 1.0, 0.0), Color::srgb(0.15, 1.0, 0.2), Color::srgb(0.0, 1.0, 0.72), Color::srgb(0.88, 1.0, 0.12)],
    },
    Scheme {
        key: "synthwave",
        accents: [Color::srgb(1.0, 0.08, 0.72), Color::srgb(0.0, 0.9, 1.0), Color::srgb(1.0, 0.92, 0.05), Color::srgb(0.5, 0.15, 1.0)],
    },
    Scheme {
        key: "gold",
        accents: [Color::srgb(1.0, 0.55, 0.04), Color::srgb(1.0, 0.82, 0.2), Color::srgb(1.0, 0.3, 0.0), Color::srgb(1.0, 0.95, 0.65)],
    },
];

impl Scheme {
    pub fn by_key(key: &str) -> Option<Scheme> {
        SCHEMES.iter().copied().find(|s| s.key == key)
    }

    /// The page's scheme: `?palette=` first, then the visitor's saved pick (the hero's `slayback-scheme`), else Netrunner.
    fn from_page() -> Scheme {
        #[cfg(target_arch = "wasm32")]
        {
            let window = web_sys::window();
            let query = window.as_ref().and_then(|w| w.location().search().ok()).unwrap_or_default();
            let from_query = query.trim_start_matches('?').split('&').find_map(|kv| kv.strip_prefix("palette=")).map(str::to_owned);
            let saved = || window.as_ref()?.local_storage().ok()??.get_item("slayback-scheme").ok()?;
            if let Some(s) = from_query.or_else(saved).as_deref().and_then(Scheme::by_key) {
                return s;
            }
        }
        SCHEMES[0]
    }
}

pub fn plugin(app: &mut App) {
    app.insert_resource(ClearColor(Color::BLACK))
        .insert_resource(GlobalAmbientLight { color: Color::WHITE, brightness: 120.0, ..default() })
        .insert_resource(Scheme::from_page())
        .add_systems(Startup, (load_kit, spawn_stage).chain())
        .add_systems(Update, (dress_walls, dress_player, animate_player));
}

/// Shared meshes and materials, so every figure and slab batches.
#[derive(Resource)]
struct Kit {
    wall: Handle<StandardMaterial>,
    rim: Handle<StandardMaterial>,
    body: Handle<StandardMaterial>,
    glow: Handle<StandardMaterial>,
    skin: Handle<StandardMaterial>,
    torso: Handle<Mesh>,
    head: Handle<Mesh>,
    limb: Handle<Mesh>,
    visor: Handle<Mesh>,
}

fn flat(mesh: impl Into<Mesh>) -> Mesh {
    mesh.into().with_duplicated_vertices().with_computed_flat_normals()
}

fn load_kit(mut commands: Commands, scheme: Res<Scheme>, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>) {
    let accent = scheme.accents[0];
    let mut mat = |base: Color, emissive: LinearRgba| {
        mats.add(StandardMaterial { base_color: base, emissive, perceptual_roughness: 0.85, metallic: 0.0, ..default() })
    };
    let kit = Kit {
        wall: mat(Color::srgb(0.05, 0.07, 0.05), LinearRgba::BLACK),
        rim: mat(accent, accent.to_linear() * 4.0),
        body: mat(accent.darker(0.55), LinearRgba::BLACK),
        glow: mat(accent, accent.to_linear() * 6.0),
        skin: mat(Color::srgb(0.85, 0.9, 0.8), LinearRgba::BLACK),
        // A figure in "rig units" (about 2.45 tall), scaled to the player's collider by `Rig`.
        torso: meshes.add(flat(Cuboid::new(0.7, 1.0, 0.4))),
        head: meshes.add(flat(Sphere::new(0.32).mesh().ico(1).expect("ico subdivision 1 is valid"))),
        limb: meshes.add(flat(Cuboid::new(0.22, 0.75, 0.22))),
        visor: meshes.add(flat(Cuboid::new(0.4, 0.08, 0.1))),
    };
    commands.insert_resource(kit);
}

/// Camera, light, the back wall and the glowing rim of the box.
fn spawn_stage(mut commands: Commands, kit: Res<Kit>, mut meshes: ResMut<Assets<Mesh>>) {
    let half = ARENA / 2.0;
    // A slightly tilted view of the whole box (from the B2 spike, scaled to world units).
    commands.spawn((
        Camera3d::default(),
        Hdr,
        Projection::Perspective(PerspectiveProjection { fov: 30f32.to_radians(), far: 10_000.0, ..default() }),
        Transform::from_xyz(0.0, half.y * 0.42, 1600.0).looking_at(Vec3::new(0.0, -40.0, 0.0), Vec3::Y),
        Msaa::Sample4,
        Tonemapping::AcesFitted,
        Bloom::NATURAL,
    ));
    // One key light; WebGL2 supports a single shadow cascade.
    commands.spawn((
        DirectionalLight { illuminance: 6_000.0, shadow_maps_enabled: true, ..default() },
        Transform::from_xyz(-320.0, 960.0, 800.0).looking_at(Vec3::ZERO, Vec3::Y),
        CascadeShadowConfigBuilder { num_cascades: 1, maximum_distance: 4_000.0, ..default() }.build(),
    ));
    let strip = 5.0;
    let slabs = [
        // The back wall, behind the play plane.
        (Vec3::new(ARENA.x, ARENA.y, WALL), Vec3::new(0.0, 0.0, -DEPTH / 2.0 - WALL / 2.0), &kit.wall),
        // Neon rims: a frame around the inside of the back wall, the tops of the side walls, the front edge of the floor.
        (Vec3::new(ARENA.x - 2.0 * WALL, strip, strip), Vec3::new(0.0, half.y - WALL, -DEPTH / 2.0), &kit.rim),
        (Vec3::new(ARENA.x - 2.0 * WALL, strip, strip), Vec3::new(0.0, -half.y + WALL, -DEPTH / 2.0), &kit.rim),
        (Vec3::new(strip, ARENA.y - 2.0 * WALL, strip), Vec3::new(-half.x + WALL, 0.0, -DEPTH / 2.0), &kit.rim),
        (Vec3::new(strip, ARENA.y - 2.0 * WALL, strip), Vec3::new(half.x - WALL, 0.0, -DEPTH / 2.0), &kit.rim),
        (Vec3::new(strip, strip, DEPTH), Vec3::new(-half.x, half.y, 0.0), &kit.rim),
        (Vec3::new(strip, strip, DEPTH), Vec3::new(half.x, half.y, 0.0), &kit.rim),
        (Vec3::new(ARENA.x - 2.0 * WALL, strip, strip), Vec3::new(0.0, -half.y + WALL, DEPTH / 2.0), &kit.rim),
    ];
    for (size, pos, mat) in slabs {
        commands.spawn((Mesh3d(meshes.add(flat(Cuboid::from_size(size)))), MeshMaterial3d(mat.clone()), Transform::from_translation(pos)));
    }
}

fn dress_walls(mut commands: Commands, kit: Res<Kit>, mut meshes: ResMut<Assets<Mesh>>, walls: Query<(Entity, &Wall), Added<Wall>>) {
    for (e, wall) in &walls {
        commands.entity(e).insert((Mesh3d(meshes.add(flat(Cuboid::new(wall.0.x, wall.0.y, DEPTH)))), MeshMaterial3d(kit.wall.clone())));
    }
}

/// The player's visual root: a primitive figure in rig units, scaled to the collider and turned toward `facing`.
#[derive(Component, Default)]
struct Rig {
    /// Walk-cycle phase, advanced by distance travelled.
    stride: f32,
}

/// A swinging limb: +1 or -1 picks which way it swings with the stride.
#[derive(Component)]
struct Limb(f32);

/// Rig units to world units: the figure is ~2.45 rig units tall, the collider `SIZE.y`.
const RIG_SCALE: f32 = SIZE.y / 2.45;
/// Rig-unit height of the feet below the rig's origin.
const RIG_FEET: f32 = 0.775;

fn dress_player(mut commands: Commands, kit: Res<Kit>, players: Query<Entity, Added<Player>>) {
    for e in &players {
        commands.entity(e).insert(Visibility::default()).with_children(|p| {
            p.spawn((
                Rig::default(),
                Transform::from_xyz(0.0, -SIZE.y / 2.0 + RIG_FEET * RIG_SCALE, 0.0).with_scale(Vec3::splat(RIG_SCALE)),
                Visibility::default(),
            ))
            .with_children(|r| {
                r.spawn((Mesh3d(kit.torso.clone()), MeshMaterial3d(kit.body.clone()), Transform::from_xyz(0.0, 0.5, 0.0)));
                r.spawn((Mesh3d(kit.head.clone()), MeshMaterial3d(kit.skin.clone()), Transform::from_xyz(0.0, 1.35, 0.0)));
                r.spawn((Mesh3d(kit.visor.clone()), MeshMaterial3d(kit.glow.clone()), Transform::from_xyz(0.0, 1.4, 0.3)));
                for (dx, dy, swing, mat) in [
                    (-0.48, 0.45, 1.0, &kit.body),
                    (0.48, 0.45, -1.0, &kit.body),
                    (-0.18, -0.4, -1.0, &kit.glow),
                    (0.18, -0.4, 1.0, &kit.body),
                ] {
                    r.spawn((Mesh3d(kit.limb.clone()), MeshMaterial3d(mat.clone()), Transform::from_xyz(dx, dy, 0.0), Limb(swing)));
                }
            });
        });
    }
}

fn animate_player(
    time: Res<Time>,
    players: Query<(&Player, &LinearVelocity, &Children)>,
    mut rigs: Query<(&mut Rig, &mut Transform, &Children), Without<Limb>>,
    mut limbs: Query<(&Limb, &mut Transform), Without<Rig>>,
) {
    for (player, velocity, children) in &players {
        for &child in children {
            let Ok((mut rig, mut tf, rig_children)) = rigs.get_mut(child) else { continue };
            // Three-quarter view toward the facing side; a dash leans forward.
            let lean = if player.0.dashing() { -0.35 * player.0.facing } else { 0.0 };
            tf.rotation = Quat::from_rotation_y(0.5 * player.0.facing) * Quat::from_rotation_z(lean);
            let speed = (velocity.x.abs() / RUN_SPEED).min(1.0);
            rig.stride += velocity.x.abs() * time.delta_secs() * 0.045;
            for &limb in rig_children {
                if let Ok((l, mut lt)) = limbs.get_mut(limb) {
                    lt.rotation = Quat::from_rotation_x(rig.stride.sin() * 0.7 * speed * l.0);
                }
            }
        }
    }
}
