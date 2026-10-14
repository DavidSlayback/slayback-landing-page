//! The 3D look for G1: flat-shaded slabs and cubes under one directional light with no shadow map and no bloom (the
//! WebGL2 budget, research section 7), in the page's colour scheme. The hybrid look (outlines, muted arena, vivid
//! fighters, impact bursts) is G4's.
//!
//! Gameplay entities carry no visuals; this plugin dresses them when they appear, so the headless scenarios run the
//! same game without assets or a renderer.

use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;
use game_core::scheme;

use crate::arena::{Kind, Slab};
use crate::player::{Facing, HEIGHT, Player};

/// The page's colour scheme: four neon accents (`game_core::scheme`), the first one primary.
#[derive(Resource, Clone, Copy, Debug)]
pub struct Palette {
    pub accents: [Color; 4],
}

impl Palette {
    pub fn nth(i: usize) -> Self {
        let s = &scheme::SCHEMES[i % scheme::SCHEMES.len()];
        Self { accents: s.accents.map(|[r, g, b]| Color::srgb(r, g, b)) }
    }

    /// The page's scheme: `?palette=` first, then the visitor's saved pick (the hero's `slayback-scheme`).
    fn page() -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            let window = web_sys::window();
            let query = window.as_ref().and_then(|w| w.location().search().ok()).unwrap_or_default();
            let saved = window.as_ref().and_then(|w| w.local_storage().ok()??.get_item("slayback-scheme").ok()?);
            Self::nth(scheme::page_choice(&query, saved.as_deref()))
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self::nth(0)
        }
    }
}

pub fn plugin(app: &mut App) {
    app.insert_resource(ClearColor(Color::srgb(0.035, 0.04, 0.055)))
        .insert_resource(GlobalAmbientLight { color: Color::srgb(0.75, 0.82, 1.0), brightness: 900.0, ..default() })
        .insert_resource(Palette::page())
        .add_plugins(crate::camera::plugin)
        .add_systems(Startup, (load_kit, spawn_light))
        .add_systems(Update, (dress_slabs, dress_player, (turn_figures, walk)).chain());
}

/// Shared meshes and materials, so every cube batches.
#[derive(Resource)]
struct Kit {
    cube: Handle<Mesh>,
    floor: Handle<StandardMaterial>,
    wall: Handle<StandardMaterial>,
    glass: Handle<StandardMaterial>,
    /// The bright edge along the rim, and the dimmer one round the glass.
    edge: Handle<StandardMaterial>,
    glass_edge: Handle<StandardMaterial>,
    body: Handle<StandardMaterial>,
    trim: Handle<StandardMaterial>,
    ring: Handle<Mesh>,
    ring_mat: Handle<StandardMaterial>,
    blob: Handle<Mesh>,
    blob_mat: Handle<StandardMaterial>,
}

fn load_kit(
    mut commands: Commands,
    palette: Res<Palette>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let accent = palette.accents[0];
    let mut matte = |c: Color| mats.add(StandardMaterial { base_color: c, perceptual_roughness: 0.9, ..default() });
    let floor = matte(Color::srgb(0.3, 0.32, 0.38).mix(&accent, 0.04));
    let wall = matte(Color::srgb(0.46, 0.49, 0.57).mix(&accent, 0.06));
    let body = matte(Color::srgb(0.9, 0.9, 0.93));
    let unlit = |c: Color| StandardMaterial { base_color: c, unlit: true, ..default() };
    let kit = Kit {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        floor,
        wall,
        body,
        // Barely there: the pane's faces stack, and anything more washes out the floor behind it.
        glass: mats.add(StandardMaterial {
            base_color: Color::srgba(0.7, 0.85, 1.0, 0.035),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
        edge: mats.add(unlit(Color::srgb(0.82, 0.86, 0.95))),
        glass_edge: mats.add(unlit(Color::srgb(0.55, 0.62, 0.72))),
        trim: mats.add(unlit(accent)),
        // The player's ring (Q60): flat on the floor under the feet, in the main accent, unlit so it stays bright.
        ring: meshes.add(Annulus::new(0.5, 0.64).mesh().resolution(48)),
        ring_mat: mats.add(unlit(accent)),
        // A soft dark disc under the feet stands in for a shadow (no shadow map).
        blob: meshes.add(Circle::new(0.45).mesh().resolution(32)),
        blob_mat: mats.add(StandardMaterial {
            base_color: Color::srgba(0.0, 0.0, 0.0, 0.4),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
    };
    commands.insert_resource(kit);
}

/// One directional light (WebGL2 keeps only one), from above and in front, a little to the right, so the two side
/// walls' faces read apart; no shadow map.
fn spawn_light(mut commands: Commands) {
    commands.spawn((
        DirectionalLight { illuminance: 9_000.0, shadow_maps_enabled: false, ..default() },
        Transform::from_xyz(5.0, 14.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// Edge bars, metres thick.
const EDGE: f32 = 0.07;

/// Dresses each slab in scaled unit cubes (children, so the slab's own transform stays unscaled for its collider).
fn dress_slabs(mut commands: Commands, kit: Res<Kit>, slabs: Query<(Entity, &Slab), Added<Slab>>) {
    for (e, slab) in &slabs {
        let s = slab.size;
        commands.entity(e).insert(Visibility::default());
        let mut cube = |size: Vec3, at: Vec3, mat: &Handle<StandardMaterial>| {
            commands.spawn((
                Mesh3d(kit.cube.clone()),
                MeshMaterial3d(mat.clone()),
                Transform::from_translation(at).with_scale(size),
                ChildOf(e),
            ));
        };
        match slab.kind {
            Kind::Floor => cube(s, Vec3::ZERO, &kit.floor),
            // A bright line along the rim.
            Kind::Wall => {
                cube(s, Vec3::ZERO, &kit.wall);
                cube(Vec3::new(s.x + 0.02, EDGE, s.z + 0.02), Vec3::Y * s.y / 2.0, &kit.edge);
            }
            // The glass is a faint pane drawn by its outline: a thin rail along the rim and the two corner posts.
            Kind::Glass => {
                cube(s, Vec3::ZERO, &kit.glass);
                cube(Vec3::new(s.x, EDGE, EDGE), Vec3::Y * (s.y - EDGE) / 2.0, &kit.glass_edge);
                for x in [-1.0, 1.0] {
                    cube(Vec3::new(EDGE, s.y, EDGE), Vec3::X * x * (s.x - EDGE) / 2.0, &kit.glass_edge);
                }
            }
        }
    }
}

/// The figure, turned by the fighter's facing; it is built looking along +x.
#[derive(Component)]
struct Rig;

/// A limb that swings while walking: its rest position and swing sign.
#[derive(Component)]
struct Limb {
    rest: Vec3,
    sign: f32,
}

/// Walk cycle phase, radians.
#[derive(Component, Default)]
struct Stride(f32);

/// The player's cubes: (size, centre relative to the capsule's centre, swing sign, trim). Feet at -HEIGHT / 2.
const FIGURE: [(Vec3, Vec3, f32, bool); 7] = [
    (Vec3::new(0.22, 0.52, 0.22), Vec3::new(0.0, -0.615, -0.13), 1.0, false),
    (Vec3::new(0.22, 0.52, 0.22), Vec3::new(0.0, -0.615, 0.13), -1.0, false),
    (Vec3::new(0.36, 0.6, 0.54), Vec3::new(0.0, 0.0, 0.0), 0.0, false),
    (Vec3::new(0.18, 0.5, 0.18), Vec3::new(0.04, 0.02, -0.37), -1.0, false),
    (Vec3::new(0.18, 0.5, 0.18), Vec3::new(0.04, 0.02, 0.37), 1.0, false),
    (Vec3::new(0.42, 0.42, 0.42), Vec3::new(0.0, 0.58, 0.0), 0.0, false),
    // A visor on the face shows which way the player faces.
    (Vec3::new(0.06, 0.12, 0.32), Vec3::new(0.22, 0.6, 0.0), 0.0, true),
];

fn dress_player(mut commands: Commands, kit: Res<Kit>, players: Query<Entity, Added<Player>>) {
    for e in &players {
        let feet = -HEIGHT / 2.0;
        commands.entity(e).insert((Visibility::default(), Stride::default()));
        commands.spawn((
            Mesh3d(kit.blob.clone()),
            MeshMaterial3d(kit.blob_mat.clone()),
            Transform::from_xyz(0.0, feet + 0.01, 0.0)
                .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            ChildOf(e),
        ));
        commands.spawn((
            Mesh3d(kit.ring.clone()),
            MeshMaterial3d(kit.ring_mat.clone()),
            Transform::from_xyz(0.0, feet + 0.02, 0.0)
                .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            ChildOf(e),
        ));
        let rig = commands.spawn((Rig, Transform::default(), Visibility::default(), ChildOf(e))).id();
        for (size, at, sign, trim) in FIGURE {
            let mat = if trim { kit.trim.clone() } else { kit.body.clone() };
            commands.spawn((
                Mesh3d(kit.cube.clone()),
                MeshMaterial3d(mat),
                Transform::from_translation(at).with_scale(size),
                Limb { rest: at, sign },
                ChildOf(rig),
            ));
        }
    }
}

fn turn_figures(fighters: Query<(&Facing, &Children)>, mut rigs: Query<&mut Transform, With<Rig>>) {
    for (facing, children) in &fighters {
        for &c in children {
            if let Ok(mut tf) = rigs.get_mut(c) {
                tf.rotation = facing.rotation();
            }
        }
    }
}

/// Legs and arms swing with ground speed; standing still settles them back.
fn walk(
    time: Res<Time>,
    mut fighters: Query<(&LinearVelocity, &mut Stride, &Children)>,
    rigs: Query<&Children, With<Rig>>,
    mut limbs: Query<(&Limb, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (v, mut stride, children) in &mut fighters {
        let speed = v.0.with_y(0.0).length();
        stride.0 = if speed > 0.1 { stride.0 + speed * 2.2 * dt } else { 0.0 };
        let swing = stride.0.sin() * 0.16 * (speed / game_core::plane::RUN_SPEED).min(1.0);
        for rig in children.iter().filter_map(|c| rigs.get(c).ok()) {
            for &c in rig {
                if let Ok((limb, mut tf)) = limbs.get_mut(c) {
                    tf.translation = limb.rest + Vec3::X * swing * limb.sign;
                }
            }
        }
    }
}
