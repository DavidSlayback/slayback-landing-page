//! The camera, as Rumble Box's (decision Q65, `docs/research/rumble-box-reference.md`): yawed off the box's axis so a
//! back corner is in view, swinging round as the player crosses the box, and dropping a little closer when enemies
//! close in.
//! Each box opens with it down among the debris, swooping up and out.
//!
//! [`targets`] says where the camera wants to be and [`place`] puts it there; both are plain functions, so tests
//! check them headless. The rig eases toward its targets and writes its yaw to `input::ViewYaw`, which turns the
//! move keys to match the view.

use bevy::prelude::*;

use crate::arena::Arena;

/// Vertical field of view, degrees.
pub const FOV: f32 = 40.0;
/// The yaw with the player in the middle of the box, and how far it swings either way as the player reaches a side
/// wall, degrees. It swings away from the wall the player nears, so that wall stays behind the player rather than
/// between the camera and the player.
const YAW: f32 = 30.0;
const SWING: f32 = 35.0;
/// Pitch (down from the horizontal, degrees) and distance from the focus (metres): roaming, and fighting. The box
/// stays filling the frame either way; a fight only drops and closes the view a little.
const ROAM: (f32, f32) = (38.0, 12.0);
const FIGHT: (f32, f32) = (30.0, 9.5);
/// Where a box's swoop starts: down among the debris, close in.
const SWOOP: (f32, f32) = (6.0, 3.0);
/// The focus's height above the player's feet, metres: about the fighters' chests.
const CHEST: f32 = 0.9;
/// Enemies this close to the player (metres) pull the camera in; the focus leans this far toward their middle.
const ENGAGE: f32 = 4.0;
const LEAN: f32 = 0.3;

/// Where the camera wants to be.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aim {
    pub focus: Vec3,
    /// Radians.
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
}

/// The camera's aim for a player standing at `feet` with `foes` (positions) nearby, in `arena`.
pub fn targets(arena: &Arena, feet: Vec3, foes: impl IntoIterator<Item = Vec3>) -> Aim {
    let near: Vec<Vec3> = foes.into_iter().filter(|f| f.xz().distance(feet.xz()) < ENGAGE).collect();
    let engaged = !near.is_empty();
    let mut focus = feet + Vec3::Y * CHEST;
    if engaged {
        let middle = near.iter().sum::<Vec3>() / near.len() as f32;
        focus += (middle - feet).with_y(0.0) * LEAN;
    }
    let across = (feet.x / (arena.floor.x / 2.0)).clamp(-1.0, 1.0);
    let (pitch, distance) = if engaged { FIGHT } else { ROAM };
    Aim { focus, yaw: (YAW - SWING * across).to_radians(), pitch: pitch.to_radians(), distance }
}

/// The aim each box opens with: low and close, at the yaw it will settle on.
pub fn swoop(aim: Aim) -> Aim {
    Aim { pitch: SWOOP.0.to_radians(), distance: SWOOP.1, ..aim }
}

/// The camera for `aim`: `distance` back from the focus, looking down at it by `pitch`, round by `yaw`.
pub fn place(aim: Aim) -> Transform {
    let rotation = Quat::from_euler(EulerRot::YXZ, aim.yaw, -aim.pitch, 0.0);
    Transform { translation: aim.focus + rotation * Vec3::Z * aim.distance, rotation, ..default() }
}

/// Eases `from` toward `to`, `rate` per second over `dt`.
pub fn ease(from: Aim, to: Aim, dt: f32) -> Aim {
    let k = |rate: f32| 1.0 - (-rate * dt).exp();
    Aim {
        focus: from.focus.lerp(to.focus, k(4.0)),
        yaw: from.yaw + (to.yaw - from.yaw) * k(1.5),
        pitch: from.pitch + (to.pitch - from.pitch) * k(2.0),
        distance: from.distance + (to.distance - from.distance) * k(2.0),
    }
}

#[cfg(feature = "render")]
pub use view::{fit, plugin};

#[cfg(feature = "render")]
mod view {
    use bevy::core_pipeline::tonemapping::Tonemapping;
    use bevy::prelude::*;
    use bevy::transform::TransformSystems;

    use super::{Aim, FOV, ease, place, swoop, targets};
    use crate::arena::Arena;
    use crate::enemy::Brain;
    use crate::input::ViewYaw;
    use crate::player::{HEIGHT, Player};
    use crate::run::BoxStarted;

    /// Where the camera is aiming now, easing toward [`targets`].
    #[derive(Component)]
    pub struct Rig(Aim);

    pub fn plugin(app: &mut App) {
        app.add_systems(Startup, spawn).add_systems(PostUpdate, fit.before(TransformSystems::Propagate));
    }

    fn spawn(mut commands: Commands) {
        let aim = targets(&Arena::default(), Vec3::ZERO, []);
        commands.spawn((
            Camera3d::default(),
            Projection::Perspective(PerspectiveProjection { fov: FOV.to_radians(), ..default() }),
            // No tonemapping: ACES washed the fighters' vivid colours toward white (Q66).
            Tonemapping::None,
            Rig(swoop(aim)),
            place(swoop(aim)),
        ));
    }

    /// Follows the player on real time (so it keeps moving through hit-stop), and turns the move keys with it.
    #[allow(clippy::type_complexity)]
    pub fn fit(
        real: Res<Time<Real>>,
        arena: Res<Arena>,
        mut started: MessageReader<BoxStarted>,
        mut view: ResMut<ViewYaw>,
        // `Transform`, not `GlobalTransform`: fighters are top-level, and their transforms are this frame's
        // interpolated ones, where `GlobalTransform` is still last frame's. Following it made the player wobble.
        player: Query<&Transform, (With<Player>, Without<Camera3d>)>,
        foes: Query<&Transform, (With<Brain>, Without<Camera3d>)>,
        mut cameras: Query<(&mut Rig, &mut Transform), With<Camera3d>>,
    ) {
        let swooping = started.read().count() > 0;
        // The player's transform is its capsule's centre.
        let feet = player.single().map(|t| t.translation - Vec3::Y * HEIGHT / 2.0).unwrap_or(Vec3::ZERO);
        let want = targets(&arena, feet, foes.iter().map(|t| t.translation));
        for (mut rig, mut tf) in &mut cameras {
            rig.0 = if swooping { swoop(want) } else { ease(rig.0, want, real.delta_secs()) };
            *tf = place(rig.0);
            if view.0 != rig.0.yaw {
                view.0 = rig.0.yaw;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::to_floor;
    use crate::plane;

    fn aim(yaw_deg: f32) -> Aim {
        Aim { focus: Vec3::new(1.0, 0.9, -2.0), yaw: yaw_deg.to_radians(), pitch: 0.6, distance: 9.0 }
    }

    #[test]
    fn the_camera_looks_at_its_focus_from_its_distance() {
        for yaw in [-40.0, 0.0, 30.0, 55.0] {
            let a = aim(yaw);
            let tf = place(a);
            assert!((tf.translation.distance(a.focus) - a.distance).abs() < 1e-4);
            let ahead = tf.translation + tf.forward() * a.distance;
            assert!(ahead.distance(a.focus) < 1e-4, "{yaw}: looks at the focus");
        }
    }

    #[test]
    fn up_on_the_keys_is_up_the_screen_at_any_yaw() {
        for yaw in [-40.0, 0.0, 30.0, 55.0, 90.0] {
            let tf = place(aim(yaw));
            // Up the screen, laid flat on the floor.
            let up = plane(tf.forward().with_y(0.0).normalize());
            let right = plane(tf.right().with_y(0.0).normalize());
            let (u, r) = (to_floor([0.0, 1.0], yaw.to_radians()), to_floor([1.0, 0.0], yaw.to_radians()));
            assert!((u[0] - up[0]).abs() < 1e-4 && (u[1] - up[1]).abs() < 1e-4, "{yaw}: up {u:?} vs {up:?}");
            assert!((r[0] - right[0]).abs() < 1e-4 && (r[1] - right[1]).abs() < 1e-4, "{yaw}: right {r:?}");
        }
    }

    #[test]
    fn the_camera_swings_with_the_player_across_the_box() {
        let arena = Arena::default();
        let half = arena.floor.x / 2.0;
        let yaw = |x: f32| targets(&arena, Vec3::new(x, 0.0, 0.0), []).yaw.to_degrees();
        assert!((yaw(0.0) - YAW).abs() < 1e-3, "yawed off the axis in the middle");
        assert!(yaw(-half) > yaw(0.0) && yaw(0.0) > yaw(half), "it swings as the player crosses");
        assert!((yaw(half * 2.0) - yaw(half)).abs() < 1e-3, "and no further than the walls");
        // Near a side wall, the camera stays on the box's side of it: the wall is behind the player.
        for x in [-half, half] {
            let a = targets(&arena, Vec3::new(x, 0.0, 0.0), []);
            let eye = place(a).translation;
            assert!(eye.x.signum() != x.signum() || eye.x.abs() < arena.floor.x / 2.0, "at {x}: camera at {eye}");
        }
    }

    #[test]
    fn enemies_close_by_pull_the_camera_in_low_and_lean_the_focus_toward_them() {
        let arena = Arena::default();
        let alone = targets(&arena, Vec3::ZERO, [Vec3::new(6.0, 0.0, 0.0)]);
        let fight = targets(&arena, Vec3::ZERO, [Vec3::new(2.0, 0.0, 0.0)]);
        assert!(fight.distance < alone.distance && fight.pitch < alone.pitch);
        assert!(fight.focus.x > alone.focus.x, "leans toward the fight: {}", fight.focus);
        assert_eq!(alone.focus, Vec3::Y * CHEST, "a far enemy doesn't pull");
    }

    #[test]
    fn a_box_opens_low_and_close_then_eases_out_to_its_aim() {
        let want = targets(&Arena::default(), Vec3::ZERO, []);
        let mut now = swoop(want);
        assert!(place(now).translation.y < 1.5, "starts down among the debris");
        for _ in 0..(3.0 * 60.0) as usize {
            now = ease(now, want, 1.0 / 60.0);
        }
        assert!((now.distance - want.distance).abs() < 0.1 && (now.pitch - want.pitch).abs() < 0.01, "{now:?}");
    }
}
