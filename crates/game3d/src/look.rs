//! The 3D look: the hybrid of decision Q50 in Rumble Box's colours (Q75). Flat-shaded cubes under one fixed light
//! with no shadow map, no bloom (the WebGL2 budget, research section 7) and no tonemapping; thick black edges
//! (`outline`, and the rim bars here); and each box in its stage's tiles (`game_core::palette`). This module dresses
//! the arena and owns the palette; `figure` dresses the fighters and their debris.
//!
//! Gameplay entities carry no visuals; the look dresses them when they appear, so the headless scenarios run the same
//! game without assets or a renderer.

use bevy::prelude::*;
use game_core::palette::Stage;

use crate::arena::{Arena, Kind, Slab, WALL};
use crate::flat::Flat;
use crate::paint::{Paints, Role, colour};
use crate::player::Player;
use crate::run::BoxStarted;

/// The current box's stage (`game_core::palette::Stage`): Rumble Box's looks, box by box. It changes as each box
/// starts. Colour things through `paint`: `paint::colour(role, &palette)` for a colour, and `Paints::get` for a shared
/// material that repaints with the stage; `Palette::nth` for box `n`'s. Only `paint` reads the stage's rules (a pre-commit
/// check keeps it that way).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub stage: &'static Stage,
}

/// How many stages on the run's colours start: the page's `stage=` (`demo`), for reviewing each box's look.
#[derive(Resource)]
struct FirstStage(usize);

pub fn plugin(app: &mut App) {
    // No lights: `flat` shades every surface from one fixed key light and ambient of its own.
    let first = FirstStage(crate::demo::setting("stage").and_then(|s| s.parse().ok()).unwrap_or(0));
    let palette = Palette::nth(first.0);
    app.insert_resource(ClearColor(colour(Role::Sky, &palette)))
        .insert_resource(palette)
        .insert_resource(first)
        .add_plugins((
            crate::flat::plugin,
            crate::paint::plugin,
            crate::camera::plugin,
            crate::outline::plugin,
            crate::figure::plugin,
            crate::impact::plugin,
            crate::hud::plugin,
            crate::touchpad::plugin,
        ))
        .add_systems(Startup, load_kit)
        .add_systems(Update, (next_stage, dress_slabs, fade_walls, clear_glass).chain().in_set(crate::Frame::Show));
}

/// Each box takes its stage.
fn next_stage(mut started: MessageReader<BoxStarted>, first: Res<FirstStage>, mut palette: ResMut<Palette>) {
    let Some(BoxStarted(spec)) = started.read().last() else { return };
    let next = Palette::nth(first.0 + spec.stage);
    if *palette != next {
        *palette = next;
    }
}

/// Shared meshes and materials, so every cube batches.
#[derive(Resource)]
struct Kit {
    cube: Handle<Mesh>,
    /// The floor's and walls' tiles.
    tiles: Handle<Flat>,
    glass: Handle<Flat>,
    /// Black, unlit: the rim and glass bars.
    edge: Handle<Flat>,
    /// The stage's tile lines.
    line: Handle<Flat>,
}

/// Makes the kit: the tiles and their lines in their shared paints (`paint`), the glass and the black edges.
fn load_kit(
    mut commands: Commands,
    palette: Res<Palette>,
    mut paints: ResMut<Paints>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<Flat>>,
) {
    let kit = Kit {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        tiles: paints.get(Role::Tiles, &palette, &mut mats),
        // Barely there: the pane's faces stack, and anything more washes out the floor behind it.
        glass: mats.add(Flat::unlit(Color::srgba(0.7, 0.85, 1.0, 0.035)).blend()),
        edge: mats.add(Flat::unlit(Color::BLACK)),
        line: paints.get(Role::Line, &palette, &mut mats),
    };
    commands.insert_resource(kit);
}

/// The black bars along the rim and round the glass, metres thick (Rumble Box's thick black edges, Q66).
const EDGE: f32 = 0.14;
/// The ground outside the box, at the rim (David: "it'd be cool if somehow you could see the lip of the wall and the
/// floor above without messing up the camera view"): how far it reaches out from each solid wall, and how thick it is,
/// metres. Far enough to fill the view behind the back wall.
const LEDGE: f32 = 12.0;
const LEDGE_THICK: f32 = 0.3;
/// Rumble Box's tiles: a line every metre across the floor and up the walls (the checker's tiles too), this thick.
pub const TILE: f32 = 1.0;
const LINE: f32 = 0.05;

/// A wall's solid face, its rim bar, one of its tile lines, or the ground outside it at the rim. A wall the camera is
/// behind turns to glass and loses the rest, so it never hides the fight: in particular the ground outside a wall only
/// shows while the camera is on the box's side of that wall, where it can't be between the camera and the box.
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
        let mut cube = |size: Vec3, at: Vec3, mat: &Handle<Flat>, face: Option<WallFace>| {
            let mut c = commands.spawn((
                Mesh3d(kit.cube.clone()),
                MeshMaterial3d(mat.clone()),
                Transform::from_translation(at).with_scale(size),
                ChildOf(e),
            ));
            if let Some(face) = face {
                c.insert(face);
            }
            c.id()
        };
        let mut rail = None;
        match slab.kind {
            Kind::Floor => {
                cube(s, Vec3::ZERO, &kit.tiles, None);
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
                cube(s, Vec3::ZERO, &kit.tiles, face(true));
                // The rim bar goes with the wall when the camera is behind it, or it would slice across the fight.
                cube(Vec3::new(s.x + 0.02, EDGE, s.z + 0.02), Vec3::Y * s.y / 2.0, &kit.edge, face(false));
                // The ground above: outside the wall, its top level with the rim, so the bar reads as the lip of the
                // pit you climb out of. The back wall's spans the corners behind the side walls' too.
                let out = Vec3::new(outward.x, 0.0, outward.y) * (WALL / 2.0 + LEDGE / 2.0);
                let ledge = if along_z {
                    Vec3::new(s.x + 2.0 * LEDGE, LEDGE_THICK, LEDGE)
                } else {
                    Vec3::new(LEDGE, LEDGE_THICK, s.z + 2.0 * WALL)
                };
                cube(ledge, out + Vec3::Y * (s.y - LEDGE_THICK) / 2.0, &kit.tiles, face(false));
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
                rail = Some(cube(Vec3::new(s.x, EDGE, EDGE), Vec3::Y * (s.y - EDGE) / 2.0, &kit.edge, None));
                for x in [-1.0, 1.0] {
                    cube(Vec3::new(EDGE, s.y, EDGE), Vec3::X * x * (s.x - EDGE) / 2.0, &kit.edge, None);
                }
            }
        }
        if let Some(rail) = rail {
            commands.entity(rail).insert(GlassRail);
        }
    }
}

/// The glass's top rail. The camera always looks in over the front wall, so with the player close to it the rail
/// crosses the fight; it goes while the player is within [`CLEAR`] of the glass.
#[derive(Component)]
struct GlassRail;

/// How close to the front wall the player is before its rail goes, metres.
const CLEAR: f32 = 2.0;

fn clear_glass(
    arena: Res<Arena>,
    player: Query<&Transform, With<Player>>,
    mut rails: Query<&mut Visibility, With<GlassRail>>,
) {
    let Ok(at) = player.single() else { return };
    let close = at.translation.z > arena.floor.y / 2.0 - CLEAR;
    let want = if close { Visibility::Hidden } else { Visibility::Inherited };
    for mut shown in &mut rails {
        if *shown != want {
            *shown = want;
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
    mut faces: Query<(&WallFace, &mut MeshMaterial3d<Flat>, &mut Visibility)>,
) {
    let Some(eye) = cameras.iter().next().map(|c| c.translation().xz()) else { return };
    for (face, mut mat, mut shown) in &mut faces {
        let behind = (eye - face.centre).dot(face.outward) > 0.0;
        if face.solid {
            let want = if behind { &kit.glass } else { &kit.tiles };
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
