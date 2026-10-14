//! Hit impact, after Rumble Box's comic punch (spec #58, carried over from the 2.5D game): trauma screen shake, a star
//! burst where a blow lands (scheme-coloured with a white-hot core, bigger for heavy moves and kills; biggest, with the
//! strongest shake, for a bomb's blast), a squash on the fighter who takes it, and each kill's points floating up from
//! where it fell, with the streak. The finisher's longer freeze is the hit-stop in `fighter`. Rules:
//! `game_core::impact`. All of it runs on real time, so it plays through hit-stop: the frozen frame is the squashed,
//! burst-lit, shaking one. Blows and blasts start the shake and the squash in `comfort`, which leaves them out under
//! reduced motion and thins the bursts (reduced motion, quality scaling).

use std::f32::consts::PI;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use game_core::impact::{BLAST_BURST, burst_size, pop, squash};

use crate::bomb::Exploded;
use crate::camera;
use crate::comfort::{Comfort, Quality, Shake, Squash, Thin, burst_share};
use crate::fighter::{Hit, Team};
use crate::look::{Palette, srgb};
use crate::score::Scored;

/// Shake at full trauma, metres the camera moves across the view.
const MAX_SHAKE: f32 = 0.3;
/// A burst's lifetime, seconds.
const BURST_LIFE: f32 = 0.16;
/// How far a burst sits toward the camera from the point of contact, metres, so the struck body doesn't hide it.
const BURST_LIFT: f32 = 0.45;
/// A score number's lifetime and how far it rises on screen, pixels.
const POPUP_LIFE: f32 = 0.9;
const POPUP_RISE: f32 = 48.0;

#[derive(Component)]
struct Burst {
    life: f32,
    size: f32,
}

#[derive(Component)]
struct Popup {
    life: f32,
    at: Vec3,
}

#[derive(Resource)]
struct BurstKit {
    star: Handle<Mesh>,
    outer: Handle<StandardMaterial>,
    core: Handle<StandardMaterial>,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, load)
        .add_systems(Update, repaint.run_if(resource_changed::<Palette>))
        .add_systems(Update, ((spawn_bursts, spawn_popups), (grow_bursts, rise_popups, ease_squash)).chain())
        .add_systems(PostUpdate, shake_camera.after(camera::fit).before(TransformSystems::Propagate));
}

/// A flat star in the XY plane facing +Z: `points` spikes of radius 1 between notches of radius `inner`.
fn star(points: usize, inner: f32) -> Mesh {
    let mut positions = vec![[0.0, 0.0, 0.0]];
    for i in 0..points * 2 {
        let r = if i % 2 == 0 { 1.0 } else { inner };
        let a = i as f32 * PI / points as f32;
        positions.push([r * a.cos(), r * a.sin(), 0.0]);
    }
    let n = positions.len() as u32;
    let indices: Vec<u32> = (1..n).flat_map(|i| [0, i, if i + 1 < n { i + 1 } else { 1 }]).collect();
    let normals = vec![[0.0, 0.0, 1.0]; positions.len()];
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_indices(Indices::U32(indices))
}

fn load(
    mut commands: Commands,
    palette: Res<Palette>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    // Unlit, so they read at full strength with no bloom (research section 7).
    let flat = |c: Color| StandardMaterial { base_color: c, unlit: true, cull_mode: None, ..default() };
    commands.insert_resource(BurstKit {
        star: meshes.add(star(8, 0.42)),
        outer: mats.add(flat(signal(&palette))),
        core: mats.add(flat(Color::srgb(1.0, 0.97, 0.85))),
    });
}

/// The bursts' and popups' colour: the palette's signal accent.
fn signal(palette: &Palette) -> Color {
    srgb(game_core::palette::signal(palette.scheme))
}

/// Recolours the bursts when a box changes the scheme.
fn repaint(palette: Res<Palette>, kit: Res<BurstKit>, mut mats: ResMut<Assets<StandardMaterial>>) {
    if let Some(mut m) = mats.get_mut(&kit.outer) {
        m.base_color = signal(&palette);
    }
}

/// Moves the camera off its framing by the shake, across and up the view, after `camera::fit` has placed it.
fn shake_camera(real: Res<Time<Real>>, mut shake: ResMut<Shake>, mut cameras: Query<&mut Transform, With<Camera3d>>) {
    shake.drain(real.delta_secs());
    if shake.trauma == 0.0 {
        return;
    }
    let [x, y] = shake.offset(real.elapsed_secs());
    for mut tf in &mut cameras {
        let jolt = (tf.rotation * Vec3::X * x + tf.rotation * Vec3::Y * y) * MAX_SHAKE;
        tf.translation += jolt;
    }
}

/// A star burst per blow and blast; reduced motion and quality scaling show a share of the blows' (`comfort`), but
/// always a kill's and a blast's.
#[allow(clippy::too_many_arguments)]
fn spawn_bursts(
    mut commands: Commands,
    kit: Res<BurstKit>,
    comfort: Res<Comfort>,
    quality: Res<Quality>,
    mut hits: MessageReader<Hit>,
    mut blasts: MessageReader<Exploded>,
    mut seed: Local<u32>,
    mut thin: Local<Thin>,
    cameras: Query<&Transform, With<Camera3d>>,
) {
    // Bursts face the camera as it is when they go off; they're gone before it has turned far.
    let facing = cameras.iter().next().map_or(Quat::IDENTITY, |t| t.rotation);
    let toward_camera = facing * Vec3::Z;
    let share = burst_share(&comfort, &quality);
    // A blast bursts biggest of all, centred on the bomb.
    let blows = hits.read().filter(|h| h.finisher || thin.keep(share)).map(|h| (h.at, burst_size(h.mv, h.finisher)));
    for (at, size) in blows.chain(blasts.read().map(|b| (b.at, BLAST_BURST))) {
        *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let turn = facing * Quat::from_rotation_z((*seed >> 16) as f32 / 65535.0 * PI);
        commands
            .spawn((
                Burst { life: BURST_LIFE, size },
                Mesh3d(kit.star.clone()),
                MeshMaterial3d(kit.outer.clone()),
                Transform::from_translation(at + toward_camera * BURST_LIFT).with_rotation(turn).with_scale(Vec3::ZERO),
            ))
            .with_child((
                Mesh3d(kit.star.clone()),
                MeshMaterial3d(kit.core.clone()),
                Transform::from_xyz(0.0, 0.0, 0.01)
                    .with_rotation(Quat::from_rotation_z(PI / 8.0))
                    .with_scale(Vec3::splat(0.55)),
            ));
    }
}

fn grow_bursts(mut commands: Commands, real: Res<Time<Real>>, mut bursts: Query<(Entity, &mut Burst, &mut Transform)>) {
    for (e, mut burst, mut tf) in &mut bursts {
        burst.life -= real.delta_secs();
        if burst.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }

        tf.scale = Vec3::splat(burst.size * pop(1.0 - burst.life / BURST_LIFE));
    }
}

/// Squashes each struck fighter's figure (the player's from `look`, enemies' from `figure`) and lets it spring back.
#[allow(clippy::type_complexity)]
fn ease_squash(
    mut commands: Commands,
    real: Res<Time<Real>>,
    mut fighters: Query<(Entity, &mut Squash, &Children), With<Team>>,
    mut rigs: Query<&mut Transform, With<crate::figure::Rig>>,
) {
    for (e, mut s, children) in &mut fighters {
        s.0 = (s.0 - real.delta_secs()).max(0.0);
        if s.0 == 0.0 {
            commands.entity(e).remove::<Squash>();
        }
        for &c in children {
            if let Ok(mut tf) = rigs.get_mut(c) {
                tf.scale = Vec3::from_array(squash(s.0));
            }
        }
    }
}

fn spawn_popups(mut commands: Commands, palette: Res<Palette>, mut scored: MessageReader<Scored>) {
    for s in scored.read() {
        // Rumble Box shows a kill's points where it happened; the streak count rides along once it's going.
        let label = if s.streak >= 2 { format!("+{}  x{}", s.points, s.streak) } else { format!("+{}", s.points) };
        commands.spawn((
            Popup { life: POPUP_LIFE, at: s.at + Vec3::Y * 1.0 },
            Text::new(label),
            TextFont::from_font_size(22.0 + 3.0 * s.streak.min(5) as f32),
            TextColor(signal(&palette)),
            TextShadow::default(),
            Node { position_type: PositionType::Absolute, ..default() },
        ));
    }
}

fn rise_popups(
    mut commands: Commands,
    real: Res<Time<Real>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut popups: Query<(Entity, &mut Popup, &mut Node, &mut TextColor)>,
) {
    let Ok((camera, eye)) = cameras.single() else { return };
    for (e, mut popup, mut node, mut colour) in &mut popups {
        popup.life -= real.delta_secs();
        if popup.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        let t = 1.0 - popup.life / POPUP_LIFE;
        let Ok(screen) = camera.world_to_viewport(eye, popup.at) else { continue };
        node.left = Val::Px(screen.x - 24.0);
        node.top = Val::Px(screen.y - POPUP_RISE * t);
        // Solid for most of its life, fading over the last 40%.
        colour.0.set_alpha(((1.0 - t) / 0.4).min(1.0));
    }
}
