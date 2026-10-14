//! Cube-built enemy figures and their debris, Rumble Box style (Q61): the fodder's eleven pieces
//! (`game_core::debris::solid::FODDER`) in a vivid colour, walking, winding up and punching; and the same pieces as
//! debris in a muted one once they burst apart. A bomb-head wears a bomb (`bomb::view`) in place of the ball head.
//!
//! Gameplay entities carry no visuals; this plugin dresses them when they appear. Colours come from [`colours`] alone,
//! so the look (G4) can restyle figures in one place.

use std::collections::HashMap;

use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;
use game_core::combat::Phase;
use game_core::debris::solid::{FODDER, Solid};
use game_core::enemy::Kind;
use game_core::palette::{self, DEBRIS_SHADES, Fighter};

use crate::bomb::{self, BombHead};
use crate::debris::Piece;
use crate::enemy::Brain;
use crate::fighter::{Attack, Body, Flash};
use crate::look::{Palette, srgb, vivid};
use crate::player::Facing;

/// A figure's colours: live body, the wind-up glow before it strikes, and its debris's shades.
pub struct Colours {
    pub body: Color,
    pub windup: Color,
    pub debris: [Color; DEBRIS_SHADES.len()],
}

/// Every figure colour, by `game_core::palette`'s rules (Q50): live fighters in the scheme's vivid accents, debris
/// in its muted tone like the arena, so the dead read apart from the living.
pub fn colours(kind: Kind, palette: &Palette) -> Colours {
    let body = palette::fighter_colour(palette.scheme, Fighter::Enemy(kind)).body;
    Colours {
        body: vivid(body),
        windup: srgb(body).mix(&Color::WHITE, 0.6),
        debris: std::array::from_fn(|i| srgb(palette::debris_tone(palette.scheme, i))),
    }
}

/// One kind's materials.
#[derive(Clone)]
struct Mats {
    skin: Handle<StandardMaterial>,
    windup: Handle<StandardMaterial>,
    debris: [Handle<StandardMaterial>; DEBRIS_SHADES.len()],
}

/// The figure under a fighter, turned by its facing and squashed by hits.
#[derive(Component)]
pub struct Rig;

/// One piece of a live figure: which one, and where it rests.
#[derive(Component)]
struct Part {
    index: usize,
    rest: Vec3,
}

/// Walk cycle phase, radians.
#[derive(Component, Default)]
struct Stride(f32);

/// Meshes per solid and materials per kind, shared so every piece batches.
#[derive(Resource, Default)]
struct Kit {
    meshes: HashMap<[u32; 3], Handle<Mesh>>,
    mats: HashMap<Kind, Mats>,
    flash: Handle<StandardMaterial>,
    blob: Handle<Mesh>,
    blob_mat: Handle<StandardMaterial>,
}

impl Kit {
    fn mesh(&mut self, solid: Solid, meshes: &mut Assets<Mesh>) -> Handle<Mesh> {
        let key = match solid {
            Solid::Cube(s) => s.map(f32::to_bits),
            Solid::Ball(r) => [r.to_bits(), 0, 0],
        };
        self.meshes
            .entry(key)
            .or_insert_with(|| match solid {
                Solid::Cube([x, y, z]) => meshes.add(Cuboid::new(x, y, z)),
                // A low-poly ball, faceted like the cubes.
                Solid::Ball(r) => meshes.add(Sphere::new(r).mesh().ico(1).expect("a valid icosphere")),
            })
            .clone()
    }

    /// Body, wind-up and debris materials for `kind`.
    fn mats(&mut self, kind: Kind, palette: &Palette, mats: &mut Assets<StandardMaterial>) -> Mats {
        self.mats
            .entry(kind)
            .or_insert_with(|| {
                let c = colours(kind, palette);
                let mut matte =
                    |c: Color| mats.add(StandardMaterial { base_color: c, perceptual_roughness: 0.85, ..default() });
                let skin = matte(c.body);
                let debris = c.debris.map(&mut matte);
                Mats {
                    skin,
                    debris,
                    windup: mats.add(StandardMaterial { base_color: c.windup, unlit: true, ..default() }),
                }
            })
            .clone()
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Kit>().add_systems(Startup, load).add_systems(
        Update,
        (repaint.run_if(resource_changed::<Palette>), dress_enemies, dress_debris, animate).chain(),
    );
}

fn load(mut kit: ResMut<Kit>, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>) {
    kit.flash = mats.add(StandardMaterial { base_color: Color::WHITE, unlit: true, ..default() });
    // A soft dark disc under the feet stands in for a shadow (no shadow map).
    kit.blob = meshes.add(Circle::new(0.42).mesh().resolution(24));
    kit.blob_mat = mats.add(StandardMaterial {
        base_color: Color::srgba(0.0, 0.0, 0.0, 0.4),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
}

/// Repaints every kind's materials when a box changes the scheme (live figures and their debris alike).
fn repaint(kit: Res<Kit>, palette: Res<Palette>, mut mats: ResMut<Assets<StandardMaterial>>) {
    for (&kind, m) in &kit.mats {
        let c = colours(kind, &palette);
        let paints = [(&m.skin, c.body), (&m.windup, c.windup)].into_iter();
        for (mat, colour) in paints.chain(m.debris.iter().zip(c.debris)) {
            if let Some(mut mat) = mats.get_mut(mat) {
                mat.base_color = colour;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn dress_enemies(
    mut commands: Commands,
    mut kit: ResMut<Kit>,
    bombs: Res<bomb::view::Kit>,
    palette: Res<Palette>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    enemies: Query<(Entity, &Brain, &Body, Has<BombHead>), Added<Brain>>,
) {
    for (e, brain, body, bomb_head) in &enemies {
        let skin = kit.mats(brain.0.style().kind, &palette, &mut mats).skin;
        commands.entity(e).insert((Visibility::default(), Stride::default()));
        commands.spawn((
            Mesh3d(kit.blob.clone()),
            MeshMaterial3d(kit.blob_mat.clone()),
            Transform::from_xyz(0.0, -body.height / 2.0 + 0.01, 0.0)
                .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            ChildOf(e),
        ));
        let rig = commands.spawn((Rig, Transform::default(), Visibility::default(), ChildOf(e))).id();
        // G5 gives thugs and ninjas their own builds; until then every enemy wears the fodder's.
        for (index, p) in FODDER.iter().enumerate() {
            let rest = Vec3::from_array(p.at);
            if bomb_head && matches!(p.solid, Solid::Ball(_)) {
                // A bomb for a head (a shape cue, Q35): its own dark shell and fuse, untouched by the flash.
                commands.spawn((bomb::view::head(&bombs), Transform::from_translation(rest), ChildOf(rig)));
                continue;
            }
            commands.spawn((
                Mesh3d(kit.mesh(p.solid, &mut meshes)),
                MeshMaterial3d(skin.clone()),
                Transform::from_translation(rest),
                Part { index, rest },
                ChildOf(rig),
            ));
        }
    }
}

fn dress_debris(
    mut commands: Commands,
    mut kit: ResMut<Kit>,
    palette: Res<Palette>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    pieces: Query<(Entity, &Piece), Added<Piece>>,
) {
    for (e, piece) in &pieces {
        // Neighbouring parts take different shades, so a heap's pieces read apart under the outlines.
        let debris = kit.mats(Kind::Fodder, &palette, &mut mats).debris[piece.part % DEBRIS_SHADES.len()].clone();
        commands.entity(e).insert((Mesh3d(kit.mesh(piece.solid, &mut meshes)), MeshMaterial3d(debris)));
    }
}

/// Which side of the body a piece is on (-1 the figure's left, +1 its right, 0 the middle) and whether it is an arm.
fn side(index: usize) -> (f32, bool) {
    match index {
        3 | 4 => (-1.0, true),
        5 | 6 => (1.0, true),
        7 | 8 => (-1.0, false),
        9 | 10 => (1.0, false),
        _ => (0.0, false),
    }
}

/// Where the striking (right) arm is: pulled back during the wind-up (`telegraph`, 0..=1), thrust out while a blow is
/// out, and back on recovery. Returns the offset from rest for the upper arm and for the forearm.
fn strike(telegraph: f32, swing: Option<Phase>) -> (Vec3, Vec3) {
    match swing {
        Some(Phase::Startup | Phase::Active) => (Vec3::new(0.28, 0.08, -0.06), Vec3::new(0.62, 0.32, -0.06)),
        Some(Phase::Recovery) => (Vec3::new(0.12, 0.0, 0.0), Vec3::new(0.25, 0.12, 0.0)),
        None => {
            let back = Vec3::new(-0.16, 0.04, 0.0) * telegraph;
            (back, back)
        }
    }
}

/// Turns each figure to its facing, swings its limbs with its speed, poses the striking arm, and lights it while it
/// winds up or flashes from a hit.
#[allow(clippy::type_complexity)]
fn animate(
    time: Res<Time>,
    kit: Res<Kit>,
    mut enemies: Query<(&Brain, &Facing, &LinearVelocity, &Attack, &Flash, &mut Stride, &Children)>,
    mut rigs: Query<(&mut Transform, &Children), (With<Rig>, Without<Part>)>,
    mut parts: Query<(&Part, &mut Transform, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    let dt = time.delta_secs();
    for (brain, facing, v, attack, flash, mut stride, children) in &mut enemies {
        let speed = v.0.with_y(0.0).length();
        stride.0 = if speed > 0.1 { stride.0 + speed * 3.2 * dt } else { 0.0 };
        let swing = stride.0.sin() * 0.14 * (speed / game_core::plane::RUN_SPEED * 2.0).min(1.0);
        let telegraph = brain.0.telegraph();
        let pose = strike(telegraph, attack.swing.and_then(|s| s.phase()));
        let Some(Mats { skin, windup, .. }) = kit.mats.get(&brain.0.style().kind) else { continue };
        let mat = if flash.0 > 0.0 {
            &kit.flash
        } else if telegraph > 0.3 {
            windup
        } else {
            skin
        };
        for &c in children {
            let Ok((mut rig, pieces)) = rigs.get_mut(c) else { continue };
            rig.rotation = facing.rotation();
            for &p in pieces {
                let Ok((part, mut tf, mut material)) = parts.get_mut(p) else { continue };
                let (side, arm) = side(part.index);
                let mut at = part.rest + Vec3::X * swing * side * if arm { -1.0 } else { 1.0 };
                if part.index == 5 {
                    at = part.rest + pose.0;
                } else if part.index == 6 {
                    at = part.rest + pose.1;
                }
                if tf.translation != at {
                    tf.translation = at;
                }
                if material.0 != *mat {
                    material.0 = mat.clone();
                }
            }
        }
    }
}
