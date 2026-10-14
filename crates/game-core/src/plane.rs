//! Moving over the 3D game's floor: run velocity toward an 8-way input, and a facing that turns smoothly toward the
//! last move direction (decision Q49).
//!
//! Plane vectors are `[x, y]` with y pointing away from the camera; angles are radians, counter-clockwise from +x.
//! Distances are metres (the 2.5D game's 32 world units).

use std::f32::consts::{PI, TAU};

/// Top run speed, m/s.
pub const RUN_SPEED: f32 = 6.0;
/// How quickly velocity reaches its target, m/s²: full speed (or a stop) in about a tenth of a second.
pub const RUN_ACCEL: f32 = 60.0;
/// How fast the facing turns, rad/s: a full about-turn takes about 0.2 s.
pub const TURN_RATE: f32 = 16.0;

/// Moves `from` toward `to` by at most `step`, without overshooting.
pub fn approach(from: [f32; 2], to: [f32; 2], step: f32) -> [f32; 2] {
    let d = [to[0] - from[0], to[1] - from[1]];
    let len = d[0].hypot(d[1]);
    if len <= step { to } else { [from[0] + d[0] / len * step, from[1] + d[1] / len * step] }
}

/// Next velocity on the floor for a move direction `dir` (unit length or zero).
pub fn run_velocity(current: [f32; 2], dir: [f32; 2], dt: f32) -> [f32; 2] {
    approach(current, [dir[0] * RUN_SPEED, dir[1] * RUN_SPEED], RUN_ACCEL * dt)
}

/// The heading of a direction, or `None` for no direction.
pub fn heading(dir: [f32; 2]) -> Option<f32> {
    (dir != [0.0, 0.0]).then(|| dir[1].atan2(dir[0]))
}

/// `angle` wrapped into -π..=π (unchanged, bit for bit, if it already is).
pub fn wrap(angle: f32) -> f32 {
    if (-PI..=PI).contains(&angle) {
        return angle;
    }
    let a = (angle + PI).rem_euclid(TAU) - PI;
    if a == -PI { PI } else { a }
}

/// Turns `from` toward `to` the short way round by at most `step`, landing exactly on `to`; the result is wrapped.
pub fn turn_toward(from: f32, to: f32, step: f32) -> f32 {
    let delta = wrap(to - from);
    if delta.abs() <= step { wrap(to) } else { wrap(from + delta.signum() * step) }
}

/// Facing on the floor: the heading of the last move (`target`) and where the body points now (`yaw`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Facing {
    pub yaw: f32,
    pub target: f32,
}

impl Facing {
    /// Advances one frame: a move direction re-aims the facing; standing still keeps the last one.
    pub fn step(&mut self, dir: [f32; 2], dt: f32) {
        if let Some(h) = heading(dir) {
            self.target = h;
        }
        self.yaw = turn_toward(self.yaw, self.target, TURN_RATE * dt);
    }

    /// The direction the body points now, unit length.
    pub fn forward(&self) -> [f32; 2] {
        [self.yaw.cos(), self.yaw.sin()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    #[test]
    fn running_reaches_full_speed_in_about_a_tenth_of_a_second_and_stops_as_fast() {
        let diag = std::f32::consts::FRAC_1_SQRT_2;
        let mut v = [0.0, 0.0];
        for _ in 0..7 {
            v = run_velocity(v, [diag, diag], DT);
        }
        assert!((v[0].hypot(v[1]) - RUN_SPEED).abs() < 1e-4, "diagonals run at the same speed: {v:?}");
        for _ in 0..7 {
            v = run_velocity(v, [0.0, 0.0], DT);
        }
        assert_eq!(v, [0.0, 0.0]);
    }

    #[test]
    fn turning_takes_the_short_way_round_and_lands_exactly() {
        let a = turn_toward(0.9 * PI, -0.9 * PI, 0.1);
        assert!(a > 0.9 * PI || a < -0.9 * PI, "crosses the back, not the front: {a}");
        assert_eq!(turn_toward(0.0, 0.05, 0.1), 0.05);
        assert!((turn_toward(0.0, -1.0, 0.25) + 0.25).abs() < 1e-6);
        assert!((wrap(3.0 * PI) - PI).abs() < 1e-5 && (wrap(-0.5) + 0.5).abs() < 1e-6);
    }

    #[test]
    fn facing_turns_smoothly_to_the_last_move_and_keeps_it_when_the_stick_is_released() {
        let mut f = Facing::default();
        f.step([0.0, 1.0], DT);
        assert!(f.yaw > 0.0 && f.yaw < PI / 2.0, "turning, not snapped: {}", f.yaw);
        for _ in 0..12 {
            f.step([0.0, 1.0], DT);
        }
        assert!((f.yaw - PI / 2.0).abs() < 1e-6, "faces the move after ~0.2 s: {}", f.yaw);
        for _ in 0..30 {
            f.step([0.0, 0.0], DT);
        }
        assert!((f.yaw - PI / 2.0).abs() < 1e-6, "standing still keeps the facing");
        let fw = f.forward();
        assert!(fw[0].abs() < 1e-6 && (fw[1] - 1.0).abs() < 1e-6);
    }
}
