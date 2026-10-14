//! Where blows land in 3D (decision Q49): a swing hits a cone on the floor in front of the attacker, and light aim
//! assist turns it toward the nearest enemy inside that cone, so the 3/4 view's depth never makes a blow whiff on an
//! enemy that looked lined up.
//!
//! Plane vectors and angles as in [`crate::plane`]; distances in metres.

use crate::combat::Move;
use crate::plane::wrap;

/// Half the cone's opening angle, radians (50°: a 100° wedge in front).
pub const HALF_ANGLE: f32 = 50.0 * std::f32::consts::PI / 180.0;
/// How far above or below the attacker's centre a target's centre may be and still be struck, metres: about half a
/// fighter, so a blow doesn't reach someone standing on a pile above.
pub const HEIGHT_BAND: f32 = 0.9;
/// The 3D game's scale on a knockback (`combat::MoveSpec::knockback`, m/s), for its smaller box and gentler gravity:
/// across the floor a little under the run speeds' ratio (a punch shoves about a metre), and upward so a kick's hop
/// lasts about as long as it did in the side view.
pub const ACROSS: f32 = 0.35;
pub const UP: f32 = 0.4;

/// A swing's reach: a sector of the floor around `apex`, opening toward `yaw`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cone {
    pub apex: [f32; 2],
    pub yaw: f32,
    pub reach: f32,
}

impl Cone {
    /// The cone of `mv` thrown by an attacker of `radius` at `apex` facing `yaw`: the move's reach past the body's
    /// edge.
    pub fn of(mv: Move, apex: [f32; 2], yaw: f32, radius: f32) -> Cone {
        Cone { apex, yaw, reach: radius + mv.spec().reach }
    }

    /// The bearing of `p` from the apex relative to the cone's axis (-π..=π) and its distance.
    fn bearing(&self, p: [f32; 2]) -> (f32, f32) {
        let d = [p[0] - self.apex[0], p[1] - self.apex[1]];
        (wrap(d[1].atan2(d[0]) - self.yaw), d[0].hypot(d[1]))
    }

    /// Whether a body of `radius` centred at `p` overlaps the cone: near enough, and inside the opening widened by the
    /// angle the body covers. A body overlapping the apex counts.
    pub fn contains(&self, p: [f32; 2], radius: f32) -> bool {
        let (off, dist) = self.bearing(p);
        if dist <= radius {
            return true;
        }
        dist - radius <= self.reach && off.abs() <= HALF_ANGLE + (radius / dist).asin()
    }

    /// Aim assist: of the bodies (centres, all of `radius`) inside the cone, the nearest one's index and its heading
    /// from the apex.
    pub fn assist<I: IntoIterator<Item = [f32; 2]>>(&self, bodies: I, radius: f32) -> Option<(usize, f32)> {
        bodies
            .into_iter()
            .enumerate()
            .filter(|(_, p)| self.contains(*p, radius))
            .map(|(i, p)| (i, self.bearing(p)))
            .min_by(|a, b| a.1.1.total_cmp(&b.1.1))
            .map(|(i, (off, _))| (i, wrap(self.yaw + off)))
    }
}

/// Whether two centres are level enough for a blow to connect (see [`HEIGHT_BAND`]).
pub fn level(attacker_y: f32, target_y: f32) -> bool {
    (target_y - attacker_y).abs() <= HEIGHT_BAND
}

/// Launch velocity for a target struck by `mv` along `yaw`: across the floor (a plane vector) and upward, m/s.
pub fn launch(mv: Move, yaw: f32) -> ([f32; 2], f32) {
    shove(mv.spec().knockback, yaw)
}

/// Any knockback (across and up, m/s, as in [`crate::combat::MoveSpec::knockback`]) as a launch along `yaw`, scaled
/// by [`ACROSS`] and [`UP`] as [`launch`] scales a blow's.
pub fn shove(knockback: (f32, f32), yaw: f32) -> ([f32; 2], f32) {
    let k = knockback.0 * ACROSS;
    ([yaw.cos() * k, yaw.sin() * k], knockback.1 * UP)
}

#[cfg(test)]
mod tests {
    use std::f32::consts::{FRAC_PI_2, PI};

    use super::*;

    const R: f32 = 0.35;

    fn at(angle_deg: f32, dist: f32) -> [f32; 2] {
        let a = angle_deg.to_radians();
        [dist * a.cos(), dist * a.sin()]
    }

    #[test]
    fn a_punch_reaches_about_a_metre_and_a_half_past_the_body_and_a_kick_further() {
        let punch = Cone::of(Move::Punch, [0.0, 0.0], 0.0, R);
        let kick = Cone::of(Move::Kick, [0.0, 0.0], 0.0, R);
        // Q70 lengthened it from about a metre.
        assert!((1.6..2.0).contains(&punch.reach), "punch {}", punch.reach);
        assert!(kick.reach > punch.reach);
    }

    #[test]
    fn the_cone_holds_bodies_in_front_and_within_reach_only() {
        let cone = Cone::of(Move::Punch, [0.0, 0.0], 0.0, R);
        assert!(cone.contains(at(0.0, 1.2), R), "straight ahead");
        assert!(cone.contains(at(45.0, 1.2), R) && cone.contains(at(-45.0, 1.2), R), "off to either side");
        assert!(!cone.contains(at(90.0, 1.2), R), "beside");
        assert!(!cone.contains(at(180.0, 1.0), R), "behind");
        assert!(!cone.contains(at(0.0, cone.reach + R + 0.05), R), "out of reach");
        assert!(cone.contains(at(0.0, cone.reach + R - 0.05), R), "its edge just inside reach");
        assert!(cone.contains(at(150.0, 0.3), R), "overlapping the attacker counts, whatever the bearing");
    }

    #[test]
    fn the_cone_turns_with_the_yaw_and_wraps_round_the_back() {
        let back = Cone::of(Move::Kick, [1.0, 1.0], PI, R);
        assert!(back.contains([0.0, 1.1], R) && !back.contains([2.0, 1.0], R));
        let up = Cone::of(Move::Kick, [0.0, 0.0], FRAC_PI_2, R);
        assert!(up.contains([0.0, 1.5], R) && !up.contains([1.5, 0.0], R));
        // Just either side of the -π/π seam behind the facing.
        let west = Cone::of(Move::Punch, [0.0, 0.0], -PI + 0.01, R);
        assert!(west.contains(at(179.0, 1.0), R) && west.contains(at(-179.0, 1.0), R));
    }

    #[test]
    fn aim_assist_turns_toward_the_nearest_body_in_the_cone() {
        let cone = Cone::of(Move::Punch, [0.0, 0.0], 0.0, R);
        let bodies = [at(0.0, 1.5), at(40.0, 0.9), at(100.0, 0.6), at(180.0, 0.5)];
        let (i, heading) = cone.assist(bodies, R).expect("someone is in the cone");
        assert_eq!(i, 1, "the nearest inside the cone, not the nearer ones beside and behind");
        assert!((heading - 40f32.to_radians()).abs() < 1e-5, "{heading}");
        assert_eq!(cone.assist([at(90.0, 1.0), at(0.0, 3.0)], R), None, "nobody in the cone: no assist");
    }

    #[test]
    fn a_blow_reaches_level_targets_but_not_one_on_a_pile_above() {
        assert!(level(0.8, 1.2) && level(1.5, 0.8));
        assert!(!level(0.8, 2.0));
    }

    #[test]
    fn knockback_launches_along_the_yaw_and_upward_harder_for_heavier_moves() {
        let (across, up) = launch(Move::Kick, FRAC_PI_2);
        assert!(across[0].abs() < 1e-5 && across[1] > 5.0 && up > 2.0, "{across:?} {up}");
        let speed = |mv| launch(mv, 0.0).0[0];
        assert!(speed(Move::DropKick) > speed(Move::Kick) && speed(Move::Kick) > speed(Move::Punch));
    }
}
