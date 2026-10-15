//! Debris: a defeated enemy falls apart into its own pieces, which persist as solid, climbable terrain. The rules are
//! in [`solid`] (the 2.5D game's side-view parts and budget went with it in G10).

/// The 3D game's debris (spec #58): figures are built from cubes and a ball, Rumble Box style, and burst into exactly
/// those pieces. Tumbling pieces freeze to static once they have rested for [`SETTLE`]; frozen heaps then cost almost
/// nothing to simulate and can't wake as a whole (research section 7). Budget from spike S2: at most [`MAX_MOVING`]
/// moving and [`MAX_FROZEN`] frozen pieces. Metres, y up, figures built looking along +x.
pub mod solid {
    use crate::enemy::Kind;

    /// Seconds a piece must stay at rest before it freezes.
    pub const SETTLE: f32 = 1.5;
    /// A piece still moving this long after it broke off freezes anyway (a pile's jitter must not keep it awake).
    pub const MAX_LOOSE: f32 = 6.0;
    /// Moving pieces allowed at once; past this the oldest moving ones freeze early.
    pub const MAX_MOVING: usize = 300;
    /// Frozen pieces kept; past this the oldest frozen ones go.
    pub const MAX_FROZEN: usize = 1_500;
    /// Below these speeds (m/s and rad/s) a piece counts as at rest.
    pub const REST_SPEED: f32 = 0.15;
    pub const REST_SPIN: f32 = 1.0;

    /// A piece's solid: a box (full size) or a ball.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum Solid {
        Cube([f32; 3]),
        Ball(f32),
    }

    /// Which part of the body a piece belongs to, so a figure can swing its limbs (left is the figure's left, -z).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Limb {
        Trunk,
        LeftArm,
        RightArm,
        LeftLeg,
        RightLeg,
    }

    /// One piece of a figure: its solid, where its centre sits relative to the body's centre, and its limb.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Piece {
        pub solid: Solid,
        pub at: [f32; 3],
        pub limb: Limb,
    }

    /// The fodder's height, metres (feet at minus half of it).
    pub const FODDER_HEIGHT: f32 = 1.6;
    /// The thug's: a head taller than the fodder and half again as wide, as in the 2.5D game.
    pub const THUG_HEIGHT: f32 = 1.85;
    /// The ninja's: slight and short.
    pub const NINJA_HEIGHT: f32 = 1.5;

    const fn head(r: f32, y: f32) -> Piece {
        Piece { solid: Solid::Ball(r), at: [0.0, y, 0.0], limb: Limb::Trunk }
    }

    const fn cube(size: [f32; 3], at: [f32; 3]) -> Piece {
        Piece { solid: Solid::Cube(size), at, limb: Limb::Trunk }
    }

    /// A pair of limb pieces, one each side at `z` from the middle: left (-z) then right.
    const fn pair(size: [f32; 3], y: f32, z: f32, left: Limb, right: Limb) -> [Piece; 2] {
        [
            Piece { solid: Solid::Cube(size), at: [0.0, y, -z], limb: left },
            Piece { solid: Solid::Cube(size), at: [0.0, y, z], limb: right },
        ]
    }

    const fn arms(size: [f32; 3], y: f32, z: f32) -> [Piece; 2] {
        pair(size, y, z, Limb::LeftArm, Limb::RightArm)
    }

    const fn legs(size: [f32; 3], y: f32, z: f32) -> [Piece; 2] {
        pair(size, y, z, Limb::LeftLeg, Limb::RightLeg)
    }

    /// The fodder figure, 11 pieces: a ball head, torso and hips, and two cubes to each arm and leg.
    pub const FODDER: [Piece; 11] = {
        let [upper_l, upper_r] = arms([0.16, 0.3, 0.16], 0.27, 0.34);
        let [fore_l, fore_r] = arms([0.15, 0.28, 0.15], -0.02, 0.34);
        let [thigh_l, thigh_r] = legs([0.22, 0.34, 0.22], -0.29, 0.11);
        let [shin_l, shin_r] = legs([0.2, 0.34, 0.2], -0.63, 0.11);
        [
            head(0.19, 0.61),
            cube([0.32, 0.4, 0.5], [0.0, 0.22, 0.0]),
            cube([0.28, 0.14, 0.42], [0.0, -0.05, 0.0]),
            // Arms: upper then lower, left then right; legs: thigh then shin.
            upper_l,
            fore_l,
            upper_r,
            fore_r,
            thigh_l,
            shin_l,
            thigh_r,
            shin_r,
        ]
    };

    /// The thug figure, 14 pieces: a ball head on a broad chest, a belly and hips, three cubes to each arm (ending in a
    /// big fist), and two to each leg.
    pub const THUG: [Piece; 14] = {
        let [upper_l, upper_r] = arms([0.24, 0.32, 0.24], 0.36, 0.51);
        let [fore_l, fore_r] = arms([0.22, 0.28, 0.22], 0.06, 0.51);
        let [fist_l, fist_r] = arms([0.26, 0.2, 0.26], -0.18, 0.51);
        let [thigh_l, thigh_r] = legs([0.26, 0.34, 0.26], -0.395, 0.13);
        let [shin_l, shin_r] = legs([0.24, 0.36, 0.24], -0.745, 0.13);
        [
            head(0.2, 0.725),
            cube([0.38, 0.4, 0.76], [0.0, 0.325, 0.0]),
            cube([0.36, 0.2, 0.6], [0.0, 0.025, 0.0]),
            cube([0.32, 0.15, 0.5], [0.0, -0.15, 0.0]),
            upper_l,
            fore_l,
            fist_l,
            upper_r,
            fore_r,
            fist_r,
            thigh_l,
            shin_l,
            thigh_r,
            shin_r,
        ]
    };

    /// The ninja figure, 9 pieces: a ball head, a slim torso and sash, one long cube to each arm and two to each leg.
    pub const NINJA: [Piece; 9] = {
        let [arm_l, arm_r] = arms([0.13, 0.44, 0.13], 0.19, 0.28);
        let [thigh_l, thigh_r] = legs([0.17, 0.32, 0.17], -0.23, 0.09);
        let [shin_l, shin_r] = legs([0.15, 0.36, 0.15], -0.57, 0.09);
        [
            head(0.17, 0.58),
            cube([0.26, 0.36, 0.4], [0.0, 0.23, 0.0]),
            cube([0.24, 0.12, 0.34], [0.0, -0.01, 0.0]),
            arm_l,
            arm_r,
            thigh_l,
            shin_l,
            thigh_r,
            shin_r,
        ]
    };

    /// The pieces `kind`'s figure is built from, and bursts into.
    pub fn figure(kind: Kind) -> &'static [Piece] {
        match kind {
            Kind::Fodder => &FODDER,
            Kind::Thug => &THUG,
            Kind::Ninja => &NINJA,
        }
    }

    /// `kind`'s height, metres, feet to crown.
    pub fn height(kind: Kind) -> f32 {
        match kind {
            Kind::Fodder => FODDER_HEIGHT,
            Kind::Thug => THUG_HEIGHT,
            Kind::Ninja => NINJA_HEIGHT,
        }
    }

    /// Whether a piece moving at `speed` and spinning at `spin` is at rest.
    pub fn at_rest(speed: f32, spin: f32) -> bool {
        speed < REST_SPEED && spin < REST_SPIN
    }

    /// A loose piece's clock: how long since it broke off and how long it has been at rest.
    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct Settle {
        pub age: f32,
        pub rest: f32,
    }

    impl Settle {
        /// Advances one frame; returns whether the piece should freeze now. Any motion restarts the rest clock.
        pub fn step(&mut self, dt: f32, at_rest: bool) -> bool {
            self.age += dt;
            self.rest = if at_rest { self.rest + dt } else { 0.0 };
            self.rest >= SETTLE || self.age >= MAX_LOOSE
        }
    }

    /// How many of the oldest moving pieces to freeze early so at most [`MAX_MOVING`] move.
    pub fn moving_over_cap(moving: usize) -> usize {
        moving.saturating_sub(MAX_MOVING)
    }

    /// How many of the oldest frozen pieces to remove so at most [`MAX_FROZEN`] stay.
    pub fn frozen_over_cap(frozen: usize) -> usize {
        frozen.saturating_sub(MAX_FROZEN)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        const DT: f32 = 1.0 / 60.0;

        fn half_extent(p: &Piece) -> [f32; 3] {
            match p.solid {
                Solid::Cube(s) => [s[0] / 2.0, s[1] / 2.0, s[2] / 2.0],
                Solid::Ball(r) => [r; 3],
            }
        }

        const KINDS: [Kind; 3] = [Kind::Fodder, Kind::Thug, Kind::Ninja];

        #[test]
        fn each_figure_is_its_own_count_of_pieces_with_one_ball_head_standing_inside_its_height() {
            for (kind, count) in KINDS.into_iter().zip([11, 14, 9]) {
                let pieces = figure(kind);
                assert_eq!(pieces.len(), count, "{kind:?}");
                let balls: Vec<_> = pieces.iter().filter(|p| matches!(p.solid, Solid::Ball(_))).collect();
                assert_eq!(balls.len(), 1, "{kind:?}: one ball, the head");
                assert_eq!(balls[0].limb, Limb::Trunk, "{kind:?}");
                let half = height(kind) / 2.0;
                let (mut low, mut high) = (f32::MAX, f32::MIN);
                for p in pieces {
                    let e = half_extent(p);
                    assert!(e.iter().all(|&x| x > 0.05), "{kind:?}: {p:?} is a real block");
                    low = low.min(p.at[1] - e[1]);
                    high = high.max(p.at[1] + e[1]);
                }
                assert!(
                    (low + half).abs() < 0.01 && (high - half).abs() < 0.01,
                    "{kind:?} feet to crown: {low}..{high}"
                );
                assert!(balls[0].at[1] > high - 0.5, "{kind:?}: the ball is on top");
            }
            // The thug is bigger than the fodder, the ninja slighter, as in the 2.5D game.
            const { assert!(THUG_HEIGHT > FODDER_HEIGHT && NINJA_HEIGHT < FODDER_HEIGHT) };
        }

        #[test]
        fn each_figure_has_two_arms_and_two_legs_mirrored_left_and_right() {
            for kind in KINDS {
                let pieces = figure(kind);
                let of = |limb: Limb| pieces.iter().filter(move |p| p.limb == limb);
                for (left, right) in [(Limb::LeftArm, Limb::RightArm), (Limb::LeftLeg, Limb::RightLeg)] {
                    assert!(of(left).count() > 0, "{kind:?}: {left:?}");
                    for (l, r) in of(left).zip(of(right)) {
                        assert!(l.at[2] < 0.0 && l.solid == r.solid, "{kind:?}: {l:?} and {r:?}");
                        assert_eq!([l.at[0], l.at[1], -l.at[2]], r.at, "{kind:?}: mirrored");
                    }
                    assert_eq!(of(left).count(), of(right).count(), "{kind:?}");
                }
                // Legs below the arms' tops, and the arms outside the legs.
                let reach = |limb: Limb| of(limb).map(|p| p.at[2].abs()).fold(0.0, f32::max);
                assert!(reach(Limb::RightArm) > reach(Limb::RightLeg), "{kind:?}");
            }
        }

        #[test]
        fn no_figures_pieces_overlap_so_they_burst_apart_cleanly() {
            for kind in KINDS {
                let pieces = figure(kind);
                for (i, a) in pieces.iter().enumerate() {
                    for b in &pieces[i + 1..] {
                        let (ea, eb) = (half_extent(a), half_extent(b));
                        let overlap = (0..3).all(|k| (a.at[k] - b.at[k]).abs() < ea[k] + eb[k] - 1e-3);
                        assert!(!overlap, "{kind:?}: {a:?} overlaps {b:?}");
                    }
                }
            }
        }

        #[test]
        fn a_piece_freezes_after_resting_for_the_settle_time_and_motion_restarts_the_clock() {
            let mut s = Settle::default();
            let frames = (SETTLE / DT).round() as usize;
            for _ in 0..frames - 2 {
                assert!(!s.step(DT, true));
            }
            assert!(!s.step(DT, false), "a nudge just before it froze");
            for _ in 0..frames - 1 {
                assert!(!s.step(DT, true), "rests again from zero");
            }
            assert!(s.step(DT, true) || s.step(DT, true), "frozen once rested {SETTLE} s");
            assert!(at_rest(0.05, 0.1) && !at_rest(0.5, 0.1) && !at_rest(0.05, 2.0));
        }

        #[test]
        fn a_piece_that_never_settles_freezes_anyway() {
            let mut s = Settle::default();
            let frozen_at = (0..1000).position(|_| s.step(DT, false)).expect("froze") as f32 * DT;
            assert!((frozen_at - MAX_LOOSE).abs() < 2.0 * DT, "{frozen_at}");
        }

        #[test]
        fn the_caps_hold_three_hundred_moving_and_fifteen_hundred_frozen_pieces() {
            assert_eq!(MAX_MOVING, 300);
            assert_eq!(MAX_FROZEN, 1_500);
            assert_eq!(moving_over_cap(250), 0);
            assert_eq!(moving_over_cap(MAX_MOVING + 11), 11);
            assert_eq!(frozen_over_cap(MAX_FROZEN), 0);
            assert_eq!(frozen_over_cap(MAX_FROZEN + 1), 1);
        }
    }
}
