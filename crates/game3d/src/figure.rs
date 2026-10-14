//! Cube-built enemy figures and their debris, Rumble Box style (Q61): each kind's pieces
//! (`game_core::debris::solid::figure`: the fodder's eleven, the thug's fourteen, the ninja's nine) in its own vivid
//! colour, walking, winding up and striking; and the same pieces as debris in a muted one once they burst apart. A
//! bomb-head of any kind wears a bomb (`bomb::view`) in place of its ball head.
//!
//! Gameplay entities carry no visuals; this plugin dresses them when they appear. Colours come from [`colours`] alone,
//! so the look (G4) can restyle figures in one place.

use std::collections::HashMap;

use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;
use game_core::combat::{Move, Phase};
use game_core::debris::solid::{Limb, Solid, figure};
use game_core::enemy::Kind;
use game_core::palette::{self, DEBRIS_SHADES, Fighter};

use crate::bomb::{self, BombHead};
use crate::comfort::Comfort;
use crate::debris::Piece;
use crate::enemy::{Brain, RADIUS};
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

/// One piece of a live figure: its limb, where it rests, and for an arm piece how far down the arm it is (0 at the
/// shoulder, 1 at the hand).
#[derive(Component)]
struct Part {
    limb: Limb,
    rest: Vec3,
    reach: f32,
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

fn load(
    mut kit: ResMut<Kit>,
    comfort: Res<Comfort>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    // A hit flashes the body full white; under reduced motion, a softer lit pale grey instead (as the 2.5D game tints).
    kit.flash = mats.add(if comfort.reduced_motion {
        StandardMaterial { base_color: Color::srgb(0.78, 0.8, 0.85), perceptual_roughness: 0.85, ..default() }
    } else {
        StandardMaterial { base_color: Color::WHITE, unlit: true, ..default() }
    });
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
        let kind = brain.0.style().kind;
        let skin = kit.mats(kind, &palette, &mut mats).skin;
        commands.entity(e).insert((Visibility::default(), Stride::default()));
        commands.spawn((
            Mesh3d(kit.blob.clone()),
            MeshMaterial3d(kit.blob_mat.clone()),
            Transform::from_xyz(0.0, -body.height / 2.0 + 0.01, 0.0)
                .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
                .with_scale(Vec3::splat(body.radius / RADIUS)),
            ChildOf(e),
        ));
        let rig = commands.spawn((Rig, Transform::default(), Visibility::default(), ChildOf(e))).id();
        let pieces = figure(kind);
        for p in pieces {
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
                Part { limb: p.limb, rest, reach: reach(pieces, p.limb, p.at[1]) },
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

/// How far down its arm a piece at height `y` is, 0 at the shoulder to 1 at the hand (a one-piece arm is all of it, 0.5).
fn reach(pieces: &[game_core::debris::solid::Piece], limb: Limb, y: f32) -> f32 {
    let arm: Vec<f32> = pieces.iter().filter(|p| p.limb == limb).map(|p| p.at[1]).collect();
    if arm.len() < 2 {
        return 0.5;
    }
    arm.iter().filter(|&&other| other > y).count() as f32 / (arm.len() - 1) as f32
}

/// Which side of the body a piece is on (-1 the figure's left, +1 its right, 0 the middle) and whether it is an arm.
fn side(limb: Limb) -> (f32, bool) {
    match limb {
        Limb::LeftArm => (-1.0, true),
        Limb::RightArm => (1.0, true),
        Limb::LeftLeg => (-1.0, false),
        Limb::RightLeg => (1.0, false),
        Limb::Trunk => (0.0, false),
    }
}

/// Where the striking (right) arm is: pulled back during the wind-up (`telegraph`, 0..=1) or, for a thug's smash,
/// raised overhead; thrust out while a blow is out, and back on recovery. Returns the offset from rest at the shoulder
/// and at the hand; the pieces between take their share.
fn strike(mv: Move, telegraph: f32, swing: Option<Phase>) -> (Vec3, Vec3) {
    match swing {
        Some(Phase::Startup | Phase::Active) => (Vec3::new(0.28, 0.08, -0.06), Vec3::new(0.62, 0.32, -0.06)),
        Some(Phase::Recovery) => (Vec3::new(0.12, 0.0, 0.0), Vec3::new(0.25, 0.12, 0.0)),
        None if mv == Move::Smash => (Vec3::new(-0.06, 0.18, 0.0) * telegraph, Vec3::new(-0.12, 0.62, 0.0) * telegraph),
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
        let pose = strike(brain.0.style().attack, telegraph, attack.swing.and_then(|s| s.phase()));
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
                let (side, arm) = side(part.limb);
                let at = if part.limb == Limb::RightArm {
                    part.rest + pose.0.lerp(pose.1, part.reach)
                } else {
                    part.rest + Vec3::X * swing * side * if arm { -1.0 } else { 1.0 }
                };
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
