//! Every fighter's figure, Rumble Box style (Q61, Q65-Q69): loose parts that `pose` moves. The player is built from
//! [`PLAYER`] (ball head and fists, a four-cube torso, two cubes to each leg) in contrasting colours; each enemy kind
//! from its pieces in `game_core::debris::solid::figure` (the fodder's eleven, the thug's fourteen, the ninja's nine)
//! in its own vivid colour, drawn a little smaller so gaps show. A bomb-head of any kind wears a bomb (`bomb::view`)
//! in place of its ball head. When an enemy dies, the same pieces become debris, dressed here in the arena's muted
//! tones.
//!
//! One path dresses and animates every fighter, so what one figure does (a hit flash, the softer flash under reduced
//! motion, a wind-up glow) the others do too. Colours are `paint`'s. Gameplay entities carry no visuals; this plugin dresses them
//! when they appear, so the headless scenarios run the same game without a renderer.

use std::collections::HashMap;

use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;
use game_core::combat::Move;
use game_core::debris::solid::{Limb, Solid, figure};
use game_core::palette::{DEBRIS_SHADES, Fighter};

use crate::bomb::{self, BombHead};
use crate::comfort::Comfort;
use crate::debris::Piece;
use crate::enemy::Brain;
use crate::fighter::{Attack, Body, Flash};
use crate::look::Palette;
use crate::paint::{Paint, Paints, Role};
use crate::player::{Dasher, Facing, Holding, Player};
use crate::pose::{self, Act};

/// How much smaller than its solid an enemy's part is drawn, so the loose parts show gaps between them. The player's
/// parts are sized with gaps already.
const GAP: f32 = 0.86;

/// What a part looks like this frame: a hit's flash over everything, then the wind-up glow, then its own paint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shade {
    Flash,
    Windup,
    Paint(Paint),
}

/// The rule every figure's parts follow.
pub fn shade(paint: Paint, flashing: bool, winding_up: bool) -> Shade {
    if flashing {
        Shade::Flash
    } else if winding_up {
        Shade::Windup
    } else {
        Shade::Paint(paint)
    }
}

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

/// The figure under a fighter, turned by its facing, lifted and leaned by its pose, and squashed by hits (`impact`).
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

/// A fighter's motion between frames: walk phase; the side its last blow came from and how far into the swing it was
/// last frame (negative for none), so a new swing can switch arms; whether it was holding something and seconds since
/// it let go; and the ground speed at which its stride is full.
#[derive(Component)]
struct Motion {
    phase: f32,
    side: f32,
    last: f32,
    held: bool,
    released: Option<f32>,
    full_stride: f32,
    /// Whether blows alternate arms (the player's) or always come from the right (an enemy's, whose wind-up shows).
    alternate: bool,
}

impl Motion {
    fn new(full_stride: f32, alternate: bool) -> Self {
        let side = if alternate { -1.0 } else { 1.0 };
        Self { phase: 0.0, side, last: -1.0, held: false, released: None, full_stride, alternate }
    }
}

/// Meshes per solid, and the flash, shadow and ring, shared so every piece batches (colours are in `Paints`).
#[derive(Resource, Default)]
struct Kit {
    meshes: HashMap<[u32; 3], Handle<Mesh>>,
    flash: Handle<StandardMaterial>,
    blob: Handle<Mesh>,
    blob_mat: Handle<StandardMaterial>,
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
        .add_systems(Update, (dress_fighters, dress_debris, animate).chain());
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
                rest: Vec3::from_array(at),
                limb,
                reach,
                paint,
                scale: 1.0,
            })
            .collect(),
        Fighter::Enemy(kind) => {
            let pieces = figure(kind);
            pieces
                .iter()
                .map(|p| Build {
                    solid: p.solid,
                    rest: Vec3::from_array(p.at),
                    limb: p.limb,
                    reach: reach(pieces, p.limb, p.at[1]),
                    paint: Paint::Body,
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
    mut mats: ResMut<Assets<StandardMaterial>>,
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
    mut mats: ResMut<Assets<StandardMaterial>>,
    pieces: Query<(Entity, &Piece), Added<Piece>>,
) {
    for (e, piece) in &pieces {
        // Neighbouring parts take different shades, so a heap's pieces read apart under the outlines.
        let debris = paints.get(Role::Debris(piece.part % DEBRIS_SHADES.len()), &palette, &mut mats);
        commands.entity(e).insert((Mesh3d(kit.mesh(piece.solid, &mut meshes)), MeshMaterial3d(debris)));
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

/// An enemy's striking (right) arm's wind-up before a blow comes out (`telegraph`, 0..=1): pulled back, or for a
/// thug's smash raised overhead. Returns the offset from rest at the shoulder and at the hand; the pieces between take
/// their share.
fn windup(mv: Move, telegraph: f32) -> (Vec3, Vec3) {
    if mv == Move::Smash {
        (Vec3::new(-0.06, 0.18, 0.0) * telegraph, Vec3::new(-0.12, 0.62, 0.0) * telegraph)
    } else {
        let back = Vec3::new(-0.16, 0.04, 0.0) * telegraph;
        (back, back)
    }
}

/// Poses every figure (`pose`) on the game clock, so hit-stop freezes it: the walk; each blow from the arm or leg it
/// uses (the player's punches alternate); an enemy's wind-up; a held thing overhead, a whirl (the body turning toward
/// the thing) and the throw's follow-through; a lean while dashing. Every part and the rig ease toward the pose
/// (`pose::follow`). Parts take their [`shade`]: the hit flash, the wind-up glow, or their own paint.
#[allow(clippy::type_complexity)]
fn animate(
    time: Res<Time>,
    kit: Res<Kit>,
    paints: Res<Paints>,
    mut fighters: Query<(
        &GlobalTransform,
        &Facing,
        &LinearVelocity,
        &Attack,
        &Flash,
        Option<&Holding>,
        Option<&Dasher>,
        Option<&Brain>,
        &mut Motion,
        &Children,
    )>,
    held: Query<&GlobalTransform, Without<Motion>>,
    mut rigs: Query<(&mut Transform, &Children), (With<Rig>, Without<Part>)>,
    mut parts: Query<(&Part, &mut Transform, &mut MeshMaterial3d<StandardMaterial>), Without<Rig>>,
) {
    let dt = time.delta_secs();
    let k = pose::follow(dt);
    for (me, facing, v, attack, flash, holding, dasher, brain, mut m, children) in &mut fighters {
        let who = brain.map_or(Fighter::Player, |b| Fighter::Enemy(b.0.style().kind));
        let speed = v.0.with_y(0.0).length();
        let amount = (speed / m.full_stride).min(1.0);
        m.phase += speed * 3.0 * dt;
        let mut turn = facing.rotation();
        let act = match (holding, attack.swing) {
            (Some(h), _) => {
                m.held = true;
                if h.spinning() {
                    // Face the thing as it goes round.
                    if let Ok(t) = held.get(h.target) {
                        let to = (t.translation() - me.translation()).with_y(0.0);
                        if to.length() > 1e-3 {
                            turn = Quat::from_rotation_arc(Vec3::X, to.normalize());
                        }
                    }
                    Act::Whirl
                } else {
                    Act::Hold
                }
            }
            (None, Some(swing)) => {
                if m.alternate && (swing.elapsed < m.last || m.last < 0.0) {
                    m.side = -m.side;
                }
                m.last = swing.elapsed;
                Act::Strike { swing, side: m.side }
            }
            (None, None) => Act::None,
        };
        if attack.swing.is_none() {
            m.last = -1.0;
        }
        if holding.is_none() && std::mem::take(&mut m.held) {
            m.released = Some(0.0);
        }
        let act = match (act, m.released) {
            (Act::None, Some(t)) => Act::Release { t },
            (act, _) => act,
        };
        m.released = m.released.map(|t| t + dt).filter(|t| *t < pose::RELEASE);
        let (lift, mut lean) = pose::body(m.phase, amount, act);
        if dasher.is_some_and(|d| d.0.dashing()) && matches!(act, Act::None) {
            lean = Quat::from_rotation_z(-0.3);
        }
        // An enemy telegraphs its blow: the arm winds up and the figure glows.
        let telegraph = brain.map_or(0.0, |b| b.0.telegraph());
        let wind = match brain {
            Some(b) if attack.swing.is_none() => windup(b.0.style().attack, telegraph),
            _ => (Vec3::ZERO, Vec3::ZERO),
        };
        for &c in children {
            let Ok((mut rig, pieces)) = rigs.get_mut(c) else { continue };
            rig.translation = rig.translation.lerp(lift, k);
            rig.rotation = rig.rotation.slerp(turn * lean, k);
            for &p in pieces {
                let Ok((part, mut tf, mut material)) = parts.get_mut(p) else { continue };
                let mut to = part.rest + pose::part(part.limb, part.reach, part.rest, m.phase, amount, act);
                if part.limb == Limb::RightArm {
                    to += wind.0.lerp(wind.1, part.reach);
                }
                let to = tf.translation.lerp(to, k);
                if tf.translation != to {
                    tf.translation = to;
                }
                let mat = match shade(part.paint, flash.0 > 0.0, telegraph > 0.3) {
                    Shade::Flash => Some(&kit.flash),
                    Shade::Windup => paints.made(Role::Windup(who)),
                    Shade::Paint(p) => paints.made(Role::Fighter(who, p)),
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
    fn a_hit_flashes_every_part_of_every_fighter_over_its_wind_up_and_paint() {
        for paint in [Paint::Body, Paint::Head, Paint::Fist, Paint::Trim] {
            assert_eq!(shade(paint, true, false), Shade::Flash);
            assert_eq!(shade(paint, true, true), Shade::Flash, "the flash beats the wind-up glow");
            assert_eq!(shade(paint, false, true), Shade::Windup);
            assert_eq!(shade(paint, false, false), Shade::Paint(paint));
        }
    }

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
