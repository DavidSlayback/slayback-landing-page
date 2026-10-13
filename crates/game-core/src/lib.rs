//! Engine-free rules for the escape-the-box game.
//!
//! Everything here is plain data and functions so it can be tested natively without Bevy; `crates/game`
//! wires it into ECS systems.

pub mod bomb;
pub mod combat;
pub mod debris;
pub mod doors;
pub mod enemy;
pub mod grab;
pub mod run;
pub mod score;

/// Player movement: run and dash (there is no jump; the player climbs by walking up debris).
///
/// Units are world units (the arena is 1280×720) and seconds.
pub mod movement {
    pub const RUN_SPEED: f32 = 420.0;
    /// How quickly horizontal velocity reaches its target, units/s².
    pub const RUN_ACCEL: f32 = 3600.0;
    /// Dash: a short burst at fixed speed, so it always covers `DASH_SPEED * DASH_TIME` (~176 units, 6 player widths).
    pub const DASH_SPEED: f32 = 1100.0;
    pub const DASH_TIME: f32 = 0.16;
    /// Time from the end of one dash until the next may start.
    pub const DASH_COOLDOWN: f32 = 0.5;

    /// Moves `from` toward `to` by at most `step`, without overshooting.
    pub fn move_toward(from: f32, to: f32, step: f32) -> f32 {
        if (to - from).abs() <= step { to } else { from + (to - from).signum() * step }
    }

    /// Next horizontal velocity for an input axis in -1..=1.
    pub fn run_velocity(current: f32, axis: f32, dt: f32) -> f32 {
        move_toward(current, axis.clamp(-1.0, 1.0) * RUN_SPEED, RUN_ACCEL * dt)
    }

    /// Tallest ledge a fighter walks up without a jump (debris ramps), world units: about a torso lying on its side.
    pub const MAX_STEP: f32 = 24.0;

    /// Upward speed for a hop that just clears a ledge `height` tall under `gravity` (with 15% to spare), or `None`
    /// if the ledge is too tall to step.
    pub fn step_hop(height: f32, gravity: f32) -> Option<f32> {
        (height > 0.0 && height <= MAX_STEP).then(|| (2.0 * gravity * height).sqrt() * 1.15)
    }

    /// One frame of movement input.
    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct Input {
        /// Run axis in -1..=1 (left/right held).
        pub axis: f32,
        /// The dash button went down this frame.
        pub dash: bool,
    }

    /// Per-player movement state: facing, and the dash timers.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Mover {
        /// +1 facing right, -1 facing left.
        pub facing: f32,
        dash_left: f32,
        cooldown: f32,
    }

    impl Default for Mover {
        fn default() -> Self {
            Self { facing: 1.0, dash_left: 0.0, cooldown: 0.0 }
        }
    }

    impl Mover {
        pub fn dashing(&self) -> bool {
            self.dash_left > 0.0
        }

        /// Advances one frame and returns the new horizontal velocity.
        ///
        /// A dash goes in the held direction, or the facing one when no direction is held, and ignores input until
        /// it ends; then the velocity snaps to the run speed for the held axis, so a dash's distance is exact.
        pub fn step(&mut self, vx: f32, input: Input, dt: f32) -> f32 {
            let axis = input.axis.clamp(-1.0, 1.0);
            if !self.dashing() && axis != 0.0 {
                self.facing = axis.signum();
            }
            self.cooldown = (self.cooldown - dt).max(0.0);
            if input.dash && !self.dashing() && self.cooldown == 0.0 {
                self.dash_left = DASH_TIME;
            }
            if self.dashing() {
                self.dash_left = (self.dash_left - dt).max(0.0);
                if !self.dashing() {
                    self.cooldown = DASH_COOLDOWN;
                    return axis * RUN_SPEED;
                }
                return self.facing * DASH_SPEED;
            }
            run_velocity(vx, axis, dt)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        const DT: f32 = 1.0 / 60.0;

        /// Runs `frames` frames of `input` (dash only on the first) and returns (distance, final velocity).
        fn run(m: &mut Mover, mut vx: f32, input: Input, frames: usize) -> (f32, f32) {
            let mut x = 0.0;
            for i in 0..frames {
                vx = m.step(vx, Input { dash: input.dash && i == 0, ..input }, DT);
                x += vx * DT;
            }
            (x, vx)
        }

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
                v = run_velocity(v, 1.0, DT);
            }
            assert_eq!(v, RUN_SPEED);
            for _ in 0..7 {
                v = run_velocity(v, 0.0, DT);
            }
            assert_eq!(v, 0.0);
        }

        #[test]
        fn a_dash_from_standing_covers_its_fixed_distance_in_the_facing_direction() {
            let mut m = Mover::default();
            let (x, _) = run(&mut m, 0.0, Input { axis: 0.0, dash: true }, (DASH_TIME / DT).ceil() as usize);
            assert!((x - DASH_SPEED * DASH_TIME).abs() < DASH_SPEED * DT, "dash distance {x}");
            assert!(!m.dashing());
        }

        #[test]
        fn a_dash_follows_the_held_direction_and_turns_the_player() {
            let mut m = Mover::default();
            let (x, _) = run(&mut m, 0.0, Input { axis: -1.0, dash: true }, 6);
            assert!(x < -100.0, "dashed left: {x}");
            assert_eq!(m.facing, -1.0);
        }

        #[test]
        fn steering_is_ignored_mid_dash() {
            let mut m = Mover::default();
            m.step(0.0, Input { axis: 1.0, dash: true }, DT);
            assert_eq!(m.step(DASH_SPEED, Input { axis: -1.0, dash: false }, DT), DASH_SPEED);
            assert_eq!(m.facing, 1.0);
        }

        #[test]
        fn after_a_dash_the_player_is_back_at_run_speed_for_the_held_direction() {
            let mut m = Mover::default();
            let (_, vx) = run(&mut m, 0.0, Input { axis: 1.0, dash: true }, 12);
            assert_eq!(vx, RUN_SPEED);
            let mut m = Mover::default();
            let (_, vx) = run(&mut m, 0.0, Input { axis: 0.0, dash: true }, 12);
            assert_eq!(vx, 0.0);
        }

        #[test]
        fn a_step_hop_clears_low_ledges_and_refuses_tall_ones() {
            let g = 2000.0;
            let v = step_hop(12.0, g).expect("a 12-unit ledge is a step");
            assert!(v * v / (2.0 * g) > 12.0, "the hop rises above the ledge");
            assert!(v * v / (2.0 * g) < 12.0 * 1.5, "and not much more: it's a step, not a jump");
            assert_eq!(step_hop(MAX_STEP + 1.0, g), None);
            assert_eq!(step_hop(0.0, g), None);
        }

        #[test]
        fn dashes_are_spaced_by_the_cooldown() {
            let mut m = Mover::default();
            run(&mut m, 0.0, Input { axis: 1.0, dash: true }, (DASH_TIME / DT).ceil() as usize);
            assert!(!m.dashing());
            m.step(RUN_SPEED, Input { axis: 1.0, dash: true }, DT);
            assert!(!m.dashing(), "no dash during the cooldown");
            run(&mut m, RUN_SPEED, Input { axis: 1.0, dash: false }, (DASH_COOLDOWN / DT).ceil() as usize);
            m.step(RUN_SPEED, Input { axis: 1.0, dash: true }, DT);
            assert!(m.dashing(), "dash again once the cooldown has passed");
        }
    }
}
