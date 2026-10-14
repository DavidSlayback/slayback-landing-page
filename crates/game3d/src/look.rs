//! The 3D look: the hybrid of decisions Q50 and Q66. Flat-shaded cubes under one directional light with no shadow map,
//! no bloom (the WebGL2 budget, research section 7) and no tonemapping; thick black edges (`outline`, the tile grid
//! and rim bars here); and the page's colour scheme by the rules in `game_core::palette`, rotating box by box. This
//! module dresses the arena and owns the palette; `figure` dresses the fighters and their debris.
//!
//! Gameplay entities carry no visuals; the look dresses them when they appear, so the headless scenarios run the same
//! game without assets or a renderer.

use bevy::prelude::*;
use game_core::palette::Rgb;
use game_core::scheme::{self, Scheme};

use crate::arena::{Kind, Slab, WALL};
use crate::paint::{Paints, Role};
use crate::run::BoxStarted;

/// The current box's colour scheme (`game_core::scheme`): the page's at the start, then rotating box by box (Q31).
/// It changes as each box starts, so whatever is coloured from it repaints when it changes. Colour things by
/// `game_core::palette`'s rules for [`Palette::scheme`] through [`srgb`], e.g.
/// `srgb(palette::fighter_colour(p.scheme, who).body)`; `paint::colour` holds every rule the game uses.
#[derive(Resource, Clone, Copy, Debug)]
pub struct Palette {
    pub scheme: &'static Scheme,
}

/// A `game_core::palette` colour as a Bevy colour.
pub fn srgb([r, g, b]: Rgb) -> Color {
    Color::srgb(r, g, b)
}

impl Palette {
    pub fn nth(i: usize) -> Self {
        let scheme = &scheme::SCHEMES[i % scheme::SCHEMES.len()];
        Self { scheme }
    }
}

/// The page's scheme, which the boxes rotate from: `?palette=` first, then the visitor's saved pick (the hero's
/// `slayback-scheme`).
#[derive(Resource)]
struct PageScheme(usize);

impl PageScheme {
    fn read() -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            let window = web_sys::window();
            let query = window.as_ref().and_then(|w| w.location().search().ok()).unwrap_or_default();
            let saved = window.as_ref().and_then(|w| w.local_storage().ok()??.get_item("slayback-scheme").ok()?);
            Self(scheme::page_choice(&query, saved.as_deref()))
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self(0)
        }
    }
}

pub fn plugin(app: &mut App) {
    let page = PageScheme::read();
    app.insert_resource(ClearColor(Color::srgb(0.035, 0.04, 0.055)))
        .insert_resource(GlobalAmbientLight { color: Color::srgb(0.75, 0.82, 1.0), brightness: 900.0, ..default() })
        .insert_resource(Palette::nth(page.0))
        .insert_resource(page)
        .add_plugins((
            crate::paint::plugin,
            crate::camera::plugin,
            crate::outline::plugin,
            crate::figure::plugin,
            crate::impact::plugin,
            crate::hud::plugin,
        ))
        .add_systems(Startup, (load_kit, spawn_light))
        .add_systems(Update, (rotate_scheme, dress_slabs, fade_walls).chain());
}

/// Each box takes the next scheme, starting from the page's (Q31).
fn rotate_scheme(mut started: MessageReader<BoxStarted>, page: Res<PageScheme>, mut palette: ResMut<Palette>) {
    let Some(BoxStarted(spec)) = started.read().last() else { return };
    let next = Palette::nth(page.0 + spec.scheme);
    if palette.scheme.key != next.scheme.key {
        *palette = next;
    }
}

/// Shared meshes and materials, so every cube batches.
#[derive(Resource)]
struct Kit {
    cube: Handle<Mesh>,
    floor: Handle<StandardMaterial>,
    wall: Handle<StandardMaterial>,
    glass: Handle<StandardMaterial>,
    /// Black, unlit: the rim and glass bars and the tile lines.
    line: Handle<StandardMaterial>,
}

/// Makes the kit: the floor and walls in their shared paints (`paint`), the glass and the black lines.
fn load_kit(
    mut commands: Commands,
    palette: Res<Palette>,
    mut paints: ResMut<Paints>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let floor = paints.get(Role::Floor, &palette, &mut mats);
    let wall = paints.get(Role::Wall, &palette, &mut mats);
    let unlit = || StandardMaterial { unlit: true, ..default() };
    let kit = Kit {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        floor,
        wall,
        // Barely there: the pane's faces stack, and anything more washes out the floor behind it.
        glass: mats.add(StandardMaterial {
            base_color: Color::srgba(0.7, 0.85, 1.0, 0.035),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
        line: mats.add(StandardMaterial { base_color: Color::BLACK, ..unlit() }),
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

/// The black bars along the rim and round the glass, metres thick (Rumble Box's thick black edges, Q66).
const EDGE: f32 = 0.14;
/// Rumble Box's tiles: a black line every metre across the floor and up the walls, this thick.
const TILE: f32 = 1.0;
const LINE: f32 = 0.05;

/// A wall's solid face, its rim bar or one of its tile lines. A wall the camera is behind turns to glass and loses
/// its lines and bar, so it never hides the fight.
#[derive(Component)]
struct WallFace {
    /// The wall's centre and its outward normal, on the floor.
    centre: Vec2,
    outward: Vec2,
    solid: bool,
}

/// Dresses each slab in scaled unit cubes (children, so the slab's own transform stays unscaled for its collider).
fn dress_slabs(mut commands: Commands, kit: Res<Kit>, slabs: Query<(Entity, &Slab, &Transform), Added<Slab>>) {
    for (e, slab, at) in &slabs {
        let s = slab.size;
        commands.entity(e).insert(Visibility::default());
        let mut cube = |size: Vec3, at: Vec3, mat: &Handle<StandardMaterial>, face: Option<WallFace>| {
            let mut c = commands.spawn((
                Mesh3d(kit.cube.clone()),
                MeshMaterial3d(mat.clone()),
                Transform::from_translation(at).with_scale(size),
                ChildOf(e),
            ));
            if let Some(face) = face {
                c.insert(face);
            }
        };
        match slab.kind {
            Kind::Floor => {
                cube(s, Vec3::ZERO, &kit.floor, None);
                // Tile lines over the inner floor, just proud of it.
                let inner = Vec2::new(s.x, s.z) - Vec2::splat(2.0 * WALL);
                let y = s.y / 2.0 + 0.004;
                for x in lines(inner.x) {
                    cube(Vec3::new(LINE, 0.008, inner.y), Vec3::new(x, y, 0.0), &kit.line, None);
                }
                for z in lines(inner.y) {
                    cube(Vec3::new(inner.x, 0.008, LINE), Vec3::new(0.0, y, z), &kit.line, None);
                }
            }
            Kind::Wall => {
                // Which way the wall faces: along z for the back wall, x for the side walls. The back wall spans the
                // corners; the side walls only the floor between them.
                let along_z = s.z < s.x;
                let outward =
                    if along_z { Vec2::Y * at.translation.z.signum() } else { Vec2::X * at.translation.x.signum() };
                let face = |solid| Some(WallFace { centre: at.translation.xz(), outward, solid });
                cube(s, Vec3::ZERO, &kit.wall, face(true));
                // The rim bar goes with the wall when the camera is behind it, or it would slice across the fight.
                cube(Vec3::new(s.x + 0.02, EDGE, s.z + 0.02), Vec3::Y * s.y / 2.0, &kit.line, face(false));
                // Tile lines on the inner face: across at each metre of height, and up at each metre along.
                let inward = -Vec3::new(outward.x, 0.0, outward.y) * (WALL / 2.0 + 0.004);
                let span = if along_z { s.x - 2.0 * WALL } else { s.z };
                let bar = |along: f32, up: f32| {
                    if along_z { Vec3::new(along, up, 0.008) } else { Vec3::new(0.008, up, along) }
                };
                let mut y = TILE;
                while y < s.y - TILE / 2.0 {
                    cube(bar(span, LINE), Vec3::Y * (y - s.y / 2.0) + inward, &kit.line, face(false));
                    y += TILE;
                }
                for a in lines(span) {
                    let at = if along_z { Vec3::X * a } else { Vec3::Z * a };
                    cube(bar(LINE, s.y), at + inward, &kit.line, face(false));
                }
            }
            // The glass is a faint pane drawn by its edges: a bar along the rim and the two corner posts.
            Kind::Glass => {
                cube(s, Vec3::ZERO, &kit.glass, None);
                cube(Vec3::new(s.x, EDGE, EDGE), Vec3::Y * (s.y - EDGE) / 2.0, &kit.line, None);
                for x in [-1.0, 1.0] {
                    cube(Vec3::new(EDGE, s.y, EDGE), Vec3::X * x * (s.x - EDGE) / 2.0, &kit.line, None);
                }
            }
        }
    }
}

/// Positions of the tile lines across a span `len` metres long, centred on zero: every [`TILE`], ends included.
fn lines(len: f32) -> impl Iterator<Item = f32> {
    let n = (len / TILE).round() as i32;
    (0..=n).map(move |i| -len / 2.0 + i as f32 * len / n as f32)
}

/// Turns the walls the camera is behind to glass, and hides their tile lines and rim bars.
fn fade_walls(
    kit: Res<Kit>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    mut faces: Query<(&WallFace, &mut MeshMaterial3d<StandardMaterial>, &mut Visibility)>,
) {
    let Some(eye) = cameras.iter().next().map(|c| c.translation().xz()) else { return };
    for (face, mut mat, mut shown) in &mut faces {
        let behind = (eye - face.centre).dot(face.outward) > 0.0;
        if face.solid {
            let want = if behind { &kit.glass } else { &kit.wall };
            if mat.0 != *want {
                mat.0 = want.clone();
            }
        } else {
            let want = if behind { Visibility::Hidden } else { Visibility::Inherited };
            if *shown != want {
                *shown = want;
            }
        }
    }
}
