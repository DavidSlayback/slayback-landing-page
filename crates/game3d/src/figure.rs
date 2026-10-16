//! Every fighter's figure, Rumble Box style (Q61, Q65-Q69): loose parts that `pose` moves. The player is built from
//! [`PLAYER`] (ball head and fists, a four-cube torso, two cubes to each leg) in contrasting colours; each enemy kind
//! from its pieces in `game_core::debris::solid::figure` (the fodder's eleven, the thug's fourteen, the ninja's nine)
//! in its kind's Rumble Box colours ([`enemy_paint`]), drawn a little smaller so gaps show. A bomb-head of any kind
//! wears a bomb (`bomb::view`) in place of its ball head. When an enemy dies, the same pieces become debris, dressed
//! here in a pale wash of the colour each wore.
//!
//! One path dresses and animates every fighter, so what one figure does (a hit flash, the softer flash under reduced
//! motion, a wind-up glow) the others do too; what each does is `pose::look`'s to decide. Colours are `paint`'s. Gameplay entities carry no visuals; this plugin dresses them
//! when they appear, so the headless scenarios run the same game without a renderer.

use std::collections::HashMap;

use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;
use game_core::debris::solid::{Limb, Solid, figure};
use game_core::enemy::Kind;
use game_core::palette::Fighter;

use crate::bomb::{self, BombHead};
use crate::camera::Warm;
use crate::carry::{Carried, Carry};
use crate::comfort::{Comfort, Squash};
use crate::debris::{Kin, Piece};
use crate::enemy::Brain;
use crate::fighter::{Attack, Body, Flash};
use crate::flat::Flat;
use crate::look::Palette;
use crate::paint::{Paint, Paints, Role};
use crate::player::{Dasher, Facing, Player};
use crate::pose::{self, FigureState, Motion, Tint};

/// How much smaller than its solid each part is drawn, and how far its limbs spread sideways from the middle, so the
/// loose parts show clear gaps between them, Rumble Box style (Q71, Q73). The player's table is sized with gaps already,
/// so it shrinks less. Parts spread only sideways, so the feet stay on the floor.
const GAP: f32 = 0.62;
const PLAYER_GAP: f32 = 0.76;
const SPREAD: Vec3 = Vec3::new(1.25, 1.0, 1.25);

const BLOCK: [f32; 3] = [0.27; 3];
const UPPER: [f32; 3] = [0.2; 3];
const THIGH: [f32; 3] = [0.22, 0.3, 0.22];
const FOOT: [f32; 3] = [0.3, 0.18, 0.22];

/// The player, Rumble Box style: loose parts with gaps between them, a ball head, a torso of four cubes, an upper arm
/// and a big ball fist each side, two cubes to each leg. (solid, centre relative to the capsule's centre, limb, how
/// far down the limb, paint); +z is the right-hand side, the feet at -HEIGHT / 2.
pub const PLAYER: [(Solid, [f32; 3], Limb, f32, Paint); 14] = [
    (Solid::Cube(THIGH), [0.0, -0.42, -0.15], Limb::LeftLeg, 0.35, Paint::Trim),
    (Solid::Cube(THIGH), [0.0, -0.42, 0.15], Limb::RightLeg, 0.35, Paint::Trim),
    (Solid::Cube(FOOT), [0.04, -0.785, -0.15], Limb::LeftLeg, 1.0, Paint::Trim),
    (Solid::Cube(FOOT), [0.04, -0.785, 0.15], Limb::RightLeg, 1.0, Paint::Trim),
    (Solid::Cube(BLOCK), [0.0, -0.1, -0.145], Limb::Trunk, 0.0, Paint::Body),
    (Solid::Cube(BLOCK), [0.0, -0.1, 0.145], Limb::Trunk, 0.0, Paint::Body),
    (Solid::Cube(BLOCK), [0.0, 0.2, -0.145], Limb::Trunk, 0.0, Paint::Body),
    (Solid::Cube(BLOCK), [0.0, 0.2, 0.145], Limb::Trunk, 0.0, Paint::Body),
    (Solid::Cube(UPPER), [0.0, 0.24, -0.42], Limb::LeftArm, 0.35, Paint::Body),
    (Solid::Cube(UPPER), [0.0, 0.24, 0.42], Limb::RightArm, 0.35, Paint::Body),
    (Solid::Ball(0.15), [0.06, -0.02, -0.45], Limb::LeftArm, 1.0, Paint::Fist),
    (Solid::Ball(0.15), [0.06, -0.02, 0.45], Limb::RightArm, 1.0, Paint::Fist),
    (Solid::Ball(0.21), [0.0, 0.62, 0.0], Limb::Trunk, 0.0, Paint::Head),
    // A visor on the face shows which way the player faces.
    (Solid::Cube([0.06, 0.1, 0.26]), [0.2, 0.64, 0.0], Limb::Trunk, 0.0, Paint::Trim),
];

/// The figure under a fighter, turned by its facing, lifted, leaned and squashed by its look (`pose::look`).
#[derive(Component)]
pub struct Rig;

/// One part of a live figure: its limb, where it rests, how far down the limb it is (0 at the shoulder or hip, 1 at
/// the fist or foot), and its paint.
#[derive(Component)]
struct Part {
    limb: Limb,
    rest: Vec3,
    reach: f32,
    paint: Paint,
}

/// Meshes per solid, and the flash, shadow and ring, shared so every piece batches (colours are in `Paints`).
#[derive(Resource, Default)]
struct Kit {
    meshes: HashMap<[u32; 3], Handle<Mesh>>,
    flash: Handle<Flat>,
    blob: Handle<Mesh>,
    blob_mat: Handle<Flat>,
    ring: Handle<Mesh>,
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
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Kit>()
        .add_systems(Startup, load)
        .add_systems(Update, (dress_fighters, dress_debris, animate).chain().in_set(crate::Frame::Show));
}

fn load(
    mut commands: Commands,
    mut kit: ResMut<Kit>,
    comfort: Res<Comfort>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<Flat>>,
) {
    // A hit flashes the body full white; under reduced motion, a softer lit pale grey instead (as the 2.5D game tints).
    kit.flash = mats.add(if comfort.reduced_motion {
        Flat::lit(Color::srgb(0.78, 0.8, 0.85))
    } else {
        Flat::unlit(Color::WHITE)
    });
    // The flash is built while the page loads, not on the first hit (`camera::Warm`).
    let cube = kit.mesh(Solid::Cube([0.2; 3]), &mut meshes);
    commands.spawn((Warm, Mesh3d(cube), MeshMaterial3d(kit.flash.clone())));
    // A soft dark disc under the feet stands in for a shadow (no shadow map).
    kit.blob = meshes.add(Circle::new(0.42).mesh().resolution(24));
    kit.blob_mat = mats.add(Flat::unlit(Color::srgba(0.0, 0.0, 0.0, 0.4)).blend());
    // The player's ring (Q60): flat on the floor under the feet, in its paint (`Role::Ring`).
    kit.ring = meshes.add(Annulus::new(0.5, 0.64).mesh().resolution(48));
}

/// One part to build: its solid, rest position, limb, reach, paint and drawn scale.
struct Build {
    solid: Solid,
    rest: Vec3,
    limb: Limb,
    reach: f32,
    paint: Paint,
    scale: f32,
}

/// What `who`'s figure is built from.
fn parts(who: Fighter) -> Vec<Build> {
    match who {
        Fighter::Player => PLAYER
            .iter()
            .map(|&(solid, at, limb, reach, paint)| Build {
                solid,
                rest: Vec3::from_array(at) * SPREAD,
                limb,
                reach,
                paint,
                scale: PLAYER_GAP,
            })
            .collect(),
        Fighter::Enemy(kind) => {
            let pieces = figure(kind);
            pieces
                .iter()
                .enumerate()
                .map(|(i, p)| Build {
                    solid: p.solid,
                    rest: Vec3::from_array(p.at) * SPREAD,
                    limb: p.limb,
                    reach: reach(pieces, p.limb, p.at[1]),
                    paint: enemy_paint(kind, i),
                    scale: GAP,
                })
                .collect()
        }
    }
}

/// Dresses each fighter as it appears: its shadow, the player's ring, and its figure's parts.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn dress_fighters(
    mut commands: Commands,
    mut kit: ResMut<Kit>,
    mut paints: ResMut<Paints>,
    bombs: Res<bomb::view::Kit>,
    palette: Res<Palette>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<Flat>>,
    fighters: Query<(Entity, &Body, Option<&Brain>, Has<Player>, Has<BombHead>), Added<Body>>,
) {
    for (e, body, brain, is_player, bomb_head) in &fighters {
        let who = match brain {
            Some(b) => Fighter::Enemy(b.0.style().kind),
            None if is_player => Fighter::Player,
            None => continue,
        };
        if matches!(who, Fighter::Enemy(_)) {
            paints.get(Role::Windup(who), &palette, &mut mats);
        }
        // The player's stride is full at its run speed; enemies walk slower, so theirs is full at half that.
        let full = game_core::plane::RUN_SPEED * if is_player { 1.0 } else { 0.5 };
        commands.entity(e).insert((Visibility::default(), Motion::new(full, is_player)));
        let flat = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
        let feet = -body.height / 2.0;
        commands.spawn((
            Mesh3d(kit.blob.clone()),
            MeshMaterial3d(kit.blob_mat.clone()),
            Transform::from_xyz(0.0, feet + 0.01, 0.0).with_rotation(flat).with_scale(Vec3::splat(body.radius / 0.35)),
            ChildOf(e),
        ));
        if is_player {
            commands.spawn((
                Mesh3d(kit.ring.clone()),
                MeshMaterial3d(paints.get(Role::Ring, &palette, &mut mats)),
                Transform::from_xyz(0.0, feet + 0.02, 0.0).with_rotation(flat),
                ChildOf(e),
            ));
        }
        let rig = commands.spawn((Rig, Transform::default(), Visibility::default(), ChildOf(e))).id();
        for b in parts(who) {
            if bomb_head && matches!(b.solid, Solid::Ball(_)) && b.limb == Limb::Trunk {
                // A bomb for a head (a shape cue, Q35): its own dark shell and fuse, untouched by the flash.
                commands.spawn((bomb::view::head(&bombs), Transform::from_translation(b.rest), ChildOf(rig)));
                continue;
            }
            commands.spawn((
                Mesh3d(kit.mesh(b.solid, &mut meshes)),
                MeshMaterial3d(paints.get(Role::Fighter(who, b.paint), &palette, &mut mats)),
                Transform::from_translation(b.rest).with_scale(Vec3::splat(b.scale)),
                Part { limb: b.limb, rest: b.rest, reach: b.reach, paint: b.paint },
                ChildOf(rig),
            ));
        }
    }
}

fn dress_debris(
    mut commands: Commands,
    mut kit: ResMut<Kit>,
    mut paints: ResMut<Paints>,
    palette: Res<Palette>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<Flat>>,
    pieces: Query<(Entity, &Piece, Option<&Kin>), Added<Piece>>,
) {
    for (e, piece, kin) in &pieces {
        // Each piece pales the colour it wore, so a heap's pieces read apart under the outlines as Rumble Box's do.
        let kind = kin.map_or(Kind::Fodder, |k| k.0);
        let role = Role::Debris(Fighter::Enemy(kind), enemy_paint(kind, piece.part));
        let debris = paints.get(role, &palette, &mut mats);
        commands.entity(e).insert((Mesh3d(kit.mesh(piece.solid, &mut meshes)), MeshMaterial3d(debris)));
    }
}

/// Which of its kind's colours piece `i` of an enemy's figure wears: the ball head its head colour, the hand end of an
/// arm of two or more pieces its fist colour, the legs the trim, the rest (torso, upper arms) the body.
fn enemy_paint(kind: Kind, i: usize) -> Paint {
    let pieces = figure(kind);
    let p = pieces[i % pieces.len()];
    match (p.solid, p.limb) {
        (Solid::Ball(_), _) => Paint::Head,
        (_, Limb::LeftLeg | Limb::RightLeg) => Paint::Trim,
        (_, Limb::LeftArm | Limb::RightArm) if reach(pieces, p.limb, p.at[1]) == 1.0 => Paint::Fist,
        _ => Paint::Body,
    }
}

/// How far down its limb a piece at height `y` is, 0 at the shoulder or hip to 1 at the hand or foot (a one-piece
/// limb is all of it, 0.5).
fn reach(pieces: &[game_core::debris::solid::Piece], limb: Limb, y: f32) -> f32 {
    let arm: Vec<f32> = pieces.iter().filter(|p| p.limb == limb).map(|p| p.at[1]).collect();
    if arm.len() < 2 {
        return 0.5;
    }
    arm.iter().filter(|&&other| other > y).count() as f32 / (arm.len() - 1) as f32
}

/// Poses every figure on the game clock, so hit-stop freezes it: gathers each fighter's [`FigureState`], takes its
/// [`pose::look`], and eases the rig and every part toward it (`pose::follow`); the parts take the look's tint (the
/// hit flash, the wind-up glow, or their own paint). The only writer of a figure's rig, its squash included.
#[allow(clippy::type_complexity)]
fn animate(
    time: Res<Time>,
    kit: Res<Kit>,
    paints: Res<Paints>,
    mut fighters: Query<(
        &Facing,
        &LinearVelocity,
        &Attack,
        &Flash,
        Option<&Carry>,
        Option<&Dasher>,
        Option<&Brain>,
        Option<&Squash>,
        Has<Carried>,
        &mut Motion,
        &Children,
    )>,
    mut rigs: Query<(&mut Transform, &Children), (With<Rig>, Without<Part>)>,
    mut parts: Query<(&Part, &mut Transform, &mut MeshMaterial3d<Flat>), Without<Rig>>,
) {
    let dt = time.delta_secs();
    let k = pose::follow(dt);
    for (facing, v, attack, flash, carry, dasher, brain, squash, carried, mut m, children) in &mut fighters {
        let who = brain.map_or(Fighter::Player, |b| Fighter::Enemy(b.0.style().kind));
        let state = FigureState {
            yaw: facing.0.yaw,
            speed: v.0.with_y(0.0).length(),
            swing: attack.swing,
            carry: carry.map(|c| c.phase),
            carried,
            dashing: dasher.is_some_and(|d| d.0.dashing()),
            flashing: flash.0 > 0.0,
            windup: brain.map(|b| (b.0.style().attack, b.0.telegraph())),
            squash: squash.map_or(0.0, |s| s.0),
        };
        let look = pose::look(&mut m, &state, dt);
        let (lift, turn) = look.rig();
        for &c in children {
            let Ok((mut rig, pieces)) = rigs.get_mut(c) else { continue };
            // The lift is in figure space (a lunge goes forward), so it turns with the figure.
            rig.translation = rig.translation.lerp(lift, k);
            rig.rotation = rig.rotation.slerp(turn, k);
            if rig.scale != look.scale {
                rig.scale = look.scale;
            }
            for &p in pieces {
                let Ok((part, mut tf, mut material)) = parts.get_mut(p) else { continue };
                let to = tf.translation.lerp(part.rest + look.part(part.limb, part.reach, part.rest), k);
                if tf.translation != to {
                    tf.translation = to;
                }
                let mat = match look.tint {
                    Tint::Flash => Some(&kit.flash),
                    Tint::Windup => paints.made(Role::Windup(who)),
                    Tint::Own => paints.made(Role::Fighter(who, part.paint)),
                };
                let Some(mat) = mat else { continue };
                if material.0 != *mat {
                    material.0 = mat.clone();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_player_has_one_ball_head_two_ball_fists_and_parts_down_each_limb() {
        let balls: Vec<_> = PLAYER.iter().filter(|p| matches!(p.0, Solid::Ball(_))).collect();
        assert_eq!(balls.iter().filter(|p| p.4 == Paint::Head).count(), 1);
        assert_eq!(balls.iter().filter(|p| p.4 == Paint::Fist).count(), 2);
        for limb in [Limb::LeftArm, Limb::RightArm, Limb::LeftLeg, Limb::RightLeg] {
            let reaches: Vec<f32> = PLAYER.iter().filter(|p| p.2 == limb).map(|p| p.3).collect();
            assert!(reaches.contains(&1.0) && reaches.iter().any(|r| *r < 1.0), "{limb:?}: {reaches:?}");
        }
        let top = |p: &(Solid, [f32; 3], Limb, f32, Paint)| p.1[1] + if let Solid::Ball(r) = p.0 { r } else { 0.0 };
        let top = PLAYER.iter().map(top).fold(f32::MIN, f32::max);
        assert!(top <= crate::player::HEIGHT / 2.0 + 1e-3, "inside the capsule: {top}");
    }
}
