//! Engine-free rules for the escape-the-box game.
//!
//! Everything here is plain data and functions so it can be tested natively without Bevy; `crates/game`
//! wires it into ECS systems.

/// Player movement tuning, in world units (the arena is 1280×720) and seconds.
pub mod movement {
    pub const RUN_SPEED: f32 = 420.0;
    /// How quickly horizontal velocity reaches its target, units/s².
    pub const RUN_ACCEL: f32 = 3600.0;
    pub const JUMP_SPEED: f32 = 900.0;

    /// Moves `from` toward `to` by at most `step`, without overshooting.
    pub fn move_toward(from: f32, to: f32, step: f32) -> f32 {
        if (to - from).abs() <= step { to } else { from + (to - from).signum() * step }
    }

    /// Next horizontal velocity for an input axis in -1..=1.
    pub fn run_velocity(current: f32, axis: f32, dt: f32) -> f32 {
        move_toward(current, axis.clamp(-1.0, 1.0) * RUN_SPEED, RUN_ACCEL * dt)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn move_toward_steps_without_overshooting() {
            assert_eq!(move_toward(0.0, 10.0, 3.0), 3.0);
            assert_eq!(move_toward(9.0, 10.0, 3.0), 10.0);
            assert_eq!(move_toward(0.0, -10.0, 4.0), -4.0);
        }

        #[test]
        fn running_reaches_full_speed_in_about_a_tenth_of_a_second_and_stops_as_fast() {
            let mut v = 0.0;
            for _ in 0..7 {
                v = run_velocity(v, 1.0, 1.0 / 60.0);
            }
            assert_eq!(v, RUN_SPEED);
            for _ in 0..7 {
                v = run_velocity(v, 0.0, 1.0 / 60.0);
            }
            assert_eq!(v, 0.0);
        }
    }
}
