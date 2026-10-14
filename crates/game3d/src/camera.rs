//! The camera: fixed, at a 3/4 angle, framing the whole box (decision Q48). A following or zooming camera waits until
//! boxes outgrow one screen.
//!
//! [`frame`] is exact: with the view direction fixed, each edge of the view is a plane through the camera, so every
//! corner of the box gives one linear limit on where the camera can sit. The tightest position meets the two
//! binding limits of one axis (left and right, or top and bottom) and centres the other.

use bevy::prelude::*;

use crate::arena::Arena;

/// How far the view looks down from the horizontal: a 3/4 view that still shows the back wall's face.
pub const PITCH: f32 = 50.0;
/// Vertical field of view, degrees: narrow, so the box's near and far ends stay close in scale.
pub const FOV: f32 = 34.0;
/// Fraction of the screen the box may fill on its binding axis.
const FILL: f32 = 0.94;
/// Headroom above the rim, metres: a fighter standing on the rim stays in view.
const HEADROOM: f32 = 1.0;

/// The camera's fixed orientation.
pub fn orientation() -> Quat {
    Quat::from_rotation_x(-PITCH.to_radians())
}

/// Corners of everything the camera must show: the box's outer footprint, from the floor to above the rim.
pub fn corners(arena: &Arena) -> [Vec3; 8] {
    let h = arena.outer_half();
    let top = arena.rim + HEADROOM;
    let mut out = [Vec3::ZERO; 8];
    for (i, c) in out.iter_mut().enumerate() {
        let sx = if i & 1 == 0 { -1.0 } else { 1.0 };
        let sz = if i & 2 == 0 { -1.0 } else { 1.0 };
        *c = Vec3::new(sx * h.x, if i & 4 == 0 { 0.0 } else { top }, sz * h.y);
    }
    out
}

/// Where the camera sits to frame `arena` in a viewport `aspect` (width over height) wide.
pub fn frame(arena: &Arena, aspect: f32) -> Transform {
    let rotation = orientation();
    let (right, up, forward) = (rotation * Vec3::X, rotation * Vec3::Y, rotation * Vec3::NEG_Z);
    let kv = (FOV.to_radians() / 2.0).tan() * FILL;
    let kh = kv * aspect;
    // In view coordinates (a, b, e) = (right, up, forward), point p is in view from camera (cr, cu, cf) when
    // |a - cr| <= kh (e - cf) and |b - cu| <= kv (e - cf). Each side's binding corner bounds cr ∓ k cf.
    let (mut r, mut l, mut u, mut d) = (f32::MIN, f32::MIN, f32::MIN, f32::MIN);
    for p in corners(arena) {
        let (a, b, e) = (p.dot(right), p.dot(up), p.dot(forward));
        r = r.max(a - kh * e);
        l = l.max(-a - kh * e);
        u = u.max(b - kv * e);
        d = d.max(-b - kv * e);
    }
    // Each axis's apex: where its two limits meet. The camera stands at whichever apex is further back.
    let (cr, cf_h) = ((r - l) / 2.0, -(r + l) / (2.0 * kh));
    let (cu, cf_v) = ((u - d) / 2.0, -(u + d) / (2.0 * kv));
    let translation = right * cr + up * cu + forward * cf_h.min(cf_v);
    Transform { translation, rotation, ..default() }
}

#[cfg(feature = "render")]
pub use view::plugin;

#[cfg(feature = "render")]
mod view {
    use bevy::core_pipeline::tonemapping::Tonemapping;
    use bevy::prelude::*;
    use bevy::transform::TransformSystems;

    use super::{FOV, frame};
    use crate::arena::Arena;

    pub fn plugin(app: &mut App) {
        app.add_systems(Startup, spawn).add_systems(PostUpdate, fit.before(TransformSystems::Propagate));
    }

    fn spawn(mut commands: Commands) {
        commands.spawn((
            Camera3d::default(),
            Projection::Perspective(PerspectiveProjection { fov: FOV.to_radians(), ..default() }),
            // ACES, as the 2.5D game: Bevy's default tonemapper needs the LUTs this build leaves out.
            Tonemapping::AcesFitted,
            Transform::default(),
        ));
    }

    /// Keeps the framing right for the window's shape and the box's size.
    fn fit(arena: Res<Arena>, mut cameras: Query<(&mut Transform, &Projection), With<Camera3d>>) {
        for (mut tf, projection) in &mut cameras {
            let aspect = match projection {
                Projection::Perspective(p) => p.aspect_ratio,
                _ => 16.0 / 9.0,
            };
            let want = frame(&arena, aspect);
            if *tf != want {
                *tf = want;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIDE: f32 = 16.0 / 9.0;

    /// A point's position on screen, -1..=1 on both axes, for the camera at `tf`.
    fn ndc(tf: &Transform, aspect: f32, p: Vec3) -> Vec2 {
        let local = tf.rotation.inverse() * (p - tf.translation);
        let k = (FOV.to_radians() / 2.0).tan();
        Vec2::new(local.x / (-local.z * k * aspect), local.y / (-local.z * k))
    }

    fn extent(arena: &Arena, aspect: f32) -> (Vec2, Vec2) {
        let tf = frame(arena, aspect);
        let pts = corners(arena).map(|p| ndc(&tf, aspect, p));
        let lo = pts.iter().fold(Vec2::MAX, |m, p| m.min(*p));
        let hi = pts.iter().fold(Vec2::MIN, |m, p| m.max(*p));
        (lo, hi)
    }

    #[test]
    fn the_whole_box_is_in_view_and_fills_its_binding_axis() {
        for aspect in [WIDE, 4.0 / 3.0, 21.0 / 9.0, 1.0] {
            let arena = Arena::default();
            let (lo, hi) = extent(&arena, aspect);
            assert!(lo.min_element() >= -FILL - 1e-3 && hi.max_element() <= FILL + 1e-3, "{aspect}: {lo} {hi}");
            let touches = |v: f32| (v.abs() - FILL).abs() < 1e-3;
            assert!(
                (touches(lo.x) && touches(hi.x)) || (touches(lo.y) && touches(hi.y)),
                "{aspect}: tight on one axis: {lo} {hi}"
            );
            assert!((lo.x + hi.x).abs() < 1e-3, "{aspect}: centred across: {lo} {hi}");
        }
    }

    #[test]
    fn on_a_widescreen_the_box_fills_most_of_the_width() {
        let (lo, hi) = extent(&Arena::default(), WIDE);
        assert!(hi.x - lo.x > 1.5, "a 3:2 box spans most of a 16:9 screen: {lo} {hi}");
    }

    #[test]
    fn the_camera_looks_down_at_the_box_from_in_front() {
        let tf = frame(&Arena::default(), WIDE);
        assert!(tf.translation.y > Arena::default().rim && tf.translation.z > 0.0, "{}", tf.translation);
        assert!(tf.translation.x.abs() < 1e-3, "centred across: {}", tf.translation);
    }
}
