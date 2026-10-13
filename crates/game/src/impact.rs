//! Hit impact, after Rumble Box's comic punch: a star burst where a blow lands (scheme-coloured, white-hot core,
//! bigger for heavy moves and kills), a squash on the fighter who takes it, and the points of each kill floating up
//! from where the enemy fell. The camera's shake and the finisher's longer hit-stop live in `camera` and
//! `game_core::combat`. All of it runs on real time, so it plays through hit-stop: the frozen frame is the squashed,
//! burst-lit one.

use std::f32::consts::PI;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use game_core::combat::Move;

use crate::comfort::Comfort;
use crate::fighter::Hit;
use crate::look::Scheme;
use crate::score::Scored;

/// A burst's lifetime, seconds (real time).
const BURST_LIFE: f32 = 0.16;
/// How long a struck fighter stays squashed.
pub const SQUASH_TIME: f32 = 0.14;
/// A score number's lifetime and how far it rises, in pixels.
const POPUP_LIFE: f32 = 0.9;
const POPUP_RISE: f32 = 48.0;

/// A burst's radius for one blow, world units (a fodder is 52 tall).
pub fn burst_size(mv: Move, finisher: bool) -> f32 {
    let base = match mv {
        Move::Punch | Move::Slash => 18.0,
        Move::Kick => 26.0,
        Move::DropKick => 30.0,
        Move::Smash => 32.0,
    };
    if finisher { base * 1.8 } else { base }
}

/// A burst's scale over its life (`t` from 0 to 1): it snaps open in the first 30%, then shrinks away.
pub fn pop(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.3 { t / 0.3 } else { 1.0 - ((t - 0.3) / 0.7).powi(2) }
}

/// The squash on a struck fighter's figure at `left` seconds remaining: flattened and widened at the moment of the
/// blow, back to normal as it runs out.
pub fn squash(left: f32) -> Vec3 {
    let k = (left / SQUASH_TIME).clamp(0.0, 1.0);
    Vec3::new(1.0 + 0.18 * k, 1.0 - 0.18 * k, 1.0 + 0.18 * k)
}

/// A struck fighter's squash, seconds remaining (applied to its figure by `look`).
#[derive(Component)]
pub struct Squash(pub f32);

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
    app.add_systems(Startup, load).add_systems(
        Update,
        (recolour, (spawn_bursts, spawn_popups, start_squash), (grow_bursts, rise_popups, ease_squash)).chain(),
    );
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

fn glow(colour: LinearRgba) -> StandardMaterial {
    StandardMaterial { base_color: Color::BLACK, emissive: colour, cull_mode: None, ..default() }
}

fn load(
    mut commands: Commands,
    scheme: Res<Scheme>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(BurstKit {
        star: meshes.add(star(8, 0.42)),
        outer: mats.add(glow(scheme.accents[3].to_linear() * 9.0)),
        core: mats.add(glow(LinearRgba::rgb(14.0, 13.0, 10.0))),
    });
}

/// The bursts take the warning accent of whichever scheme is up.
fn recolour(scheme: Res<Scheme>, kit: Res<BurstKit>, mut mats: ResMut<Assets<StandardMaterial>>) {
    if scheme.is_changed()
        && let Some(mut m) = mats.get_mut(&kit.outer)
    {
        m.emissive = scheme.accents[3].to_linear() * 9.0;
    }
}

fn spawn_bursts(mut commands: Commands, kit: Res<BurstKit>, mut hits: MessageReader<Hit>, mut seed: Local<u32>) {
    for hit in hits.read() {
        *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let turn = (*seed >> 16) as f32 / 65535.0 * PI;
        commands
            .spawn((
                Burst { life: BURST_LIFE, size: burst_size(hit.mv, hit.finisher) },
                Mesh3d(kit.star.clone()),
                MeshMaterial3d(kit.outer.clone()),
                // In front of the fighters, so the burst covers the point of contact.
                Transform::from_translation(hit.at.extend(40.0))
                    .with_rotation(Quat::from_rotation_z(turn))
                    .with_scale(Vec3::ZERO),
            ))
            .with_child((
                Mesh3d(kit.star.clone()),
                MeshMaterial3d(kit.core.clone()),
                Transform::from_xyz(0.0, 0.0, 1.0)
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

fn start_squash(mut commands: Commands, comfort: Res<Comfort>, mut hits: MessageReader<Hit>) {
    for hit in hits.read() {
        if !comfort.reduced_motion {
            // The victim may already be gone (a kill shatters it this frame).
            commands.entity(hit.victim).try_insert(Squash(SQUASH_TIME));
        }
    }
}

fn ease_squash(mut commands: Commands, real: Res<Time<Real>>, mut squashed: Query<(Entity, &mut Squash)>) {
    for (e, mut s) in &mut squashed {
        s.0 -= real.delta_secs();
        if s.0 <= 0.0 {
            commands.entity(e).remove::<Squash>();
        }
    }
}

fn spawn_popups(mut commands: Commands, scheme: Res<Scheme>, mut scored: MessageReader<Scored>) {
    for s in scored.read() {
        // Rumble Box shows a kill's points where it happened; the streak count rides along once it's going.
        let label = if s.streak >= 2 { format!("+{}  x{}", s.points, s.streak) } else { format!("+{}", s.points) };
        commands.spawn((
            Popup { life: POPUP_LIFE, at: s.at.extend(0.0) + Vec3::Y * 30.0 },
            Text::new(label),
            TextFont::from_font_size(20.0 + 3.0 * s.streak.min(5) as f32),
            TextColor(scheme.accents[3]),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heavier_blows_and_kills_burst_bigger() {
        assert!(burst_size(Move::Kick, false) > burst_size(Move::Punch, false));
        assert!(burst_size(Move::Punch, true) > burst_size(Move::Smash, false), "a kill outshines any plain hit");
    }

    #[test]
    fn a_burst_snaps_open_then_shrinks_away() {
        assert_eq!(pop(0.0), 0.0);
        assert_eq!(pop(0.3), 1.0, "fully open at 30% of its life");
        assert!(pop(0.6) < 1.0 && pop(0.6) > pop(0.9));
        assert_eq!(pop(1.0), 0.0);
    }

    #[test]
    fn the_squash_flattens_at_the_blow_and_recovers() {
        let hit = squash(SQUASH_TIME);
        assert!(hit.y < 1.0 && hit.x > 1.0, "flattened and widened");
        assert_eq!(squash(0.0), Vec3::ONE);
    }

    #[test]
    fn the_star_is_a_closed_fan() {
        let mesh = star(8, 0.42);
        let n = mesh.count_vertices();
        assert_eq!(n, 17, "centre plus 16 rim points");
        let Some(Indices::U32(idx)) = mesh.indices() else { panic!("u32 indices") };
        assert_eq!(idx.len(), 16 * 3, "one triangle per rim edge, including the closing one");
        assert!(idx.iter().all(|&i| (i as usize) < n));
    }
}
