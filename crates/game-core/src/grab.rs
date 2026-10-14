//! Grab and spin-throw: the player picks up a live enemy, a debris part (frozen debris thaws) or a lit bomb, and whirls it into
//! the crowd. A thrown thing is a projectile for a moment: it hurts every enemy it passes through, then lands as
//! whatever it was (a stunned enemy, or loose debris that freezes again).
//!
//! The constants and functions at the top level are the 2.5D game's (side view, world units); [`flat`] holds the 3D
//! game's (on the floor, metres). [`Held`], [`impact`], [`SPIN`], [`FLIGHT`] and [`HITSTOP`] are shared.

/// What the player can hold (decision Q33; lit bomb-heads join with D9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Held {
    Enemy,
    Debris,
    /// A lit bomb-head: its fuse keeps burning in your hands.
    Bomb,
}

/// How far past the player's edge a grab reaches, world units.
pub const REACH: f32 = 30.0;
/// The spin before release: the held thing makes one turn around the player, seconds.
pub const SPIN: f32 = 0.22;
/// Release speed along the facing direction and upward, world units/s.
pub const THROW: (f32, f32) = (950.0, 260.0);
/// Seconds a thrown thing stays dangerous.
pub const FLIGHT: f32 = 0.9;
/// Hit-stop when a thrown thing lands a hit (kept short: a throw can hit a whole crowd).
pub const HITSTOP: f32 = 0.04;

/// Launch velocity for a throw toward `facing` (+1 / -1).
pub fn throw_velocity(facing: f32) -> (f32, f32) {
    (THROW.0 * facing, THROW.1)
}

/// What a thrown thing does to each enemy it hits: damage and knockback (before facing), and the damage the thrown
/// enemy itself takes on its first impact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Impact {
    pub damage: f32,
    pub knockback: (f32, f32),
    pub self_damage: f32,
}

pub fn impact(held: Held) -> Impact {
    match held {
        // A whole body: the heaviest thing the player can swing.
        Held::Enemy => Impact { damage: 16.0, knockback: (700.0, 380.0), self_damage: 10.0 },
        Held::Debris => Impact { damage: 9.0, knockback: (450.0, 260.0), self_damage: 0.0 },
        // The bomb's real damage is its blast; the throw itself only clips.
        Held::Bomb => Impact { damage: 4.0, knockback: (250.0, 150.0), self_damage: 0.0 },
    }
}

/// Where the held thing sits relative to the player's centre, as (x, y, depth): overhead while carried; during the
/// spin (`spin` in 0..=1) it swings down behind the shoulders, then circles round the body through the depth axis (in
/// front of the player, toward the viewer) to release level in front. Physics is 2D, so depth is only drawn.
pub fn hold_offset(facing: f32, half_height: f32, spin: Option<f32>) -> (f32, f32, f32) {
    let r = half_height + 14.0;
    let shoulder = half_height * 0.3;
    match spin.map(|t| t.clamp(0.0, 1.0)) {
        None => (0.0, r, 0.0),
        // Overhead to behind the shoulders.
        Some(t) if t < 0.3 => {
            let a = std::f32::consts::FRAC_PI_2 * (1.0 + t / 0.3);
            (facing * r * a.cos(), shoulder + (r - shoulder) * a.sin(), 0.0)
        }
        // Behind, round the front of the body (toward the viewer), to straight ahead.
        Some(t) => {
            let a = std::f32::consts::PI * (1.0 + (t - 0.3) / 0.7);
            (facing * r * a.cos(), shoulder, -r * a.sin())
        }
    }
}

/// Grab and spin-throw on the 3D game's floor (decisions Q49, Q62): a grab takes the nearest thing in a cone in front;
/// the held thing whirls once and a half round the player and flies flat (no lob) along the facing, which aim assist
/// bends toward the biggest enemy group within 30°. Plane vectors and angles as in [`crate::plane`]; metres.
pub mod flat {
    use std::f32::consts::PI;

    use super::{FLIGHT, Held, impact};
    use crate::aim::{Cone, shove};
    use crate::plane::{UNITS_PER_METRE, heading, wrap};

    /// How far past the player's edge a grab reaches: the 2.5D reach, about a metre.
    pub const REACH: f32 = super::REACH / UNITS_PER_METRE;
    /// Throw speed across the floor, m/s: three-quarters of a dash, flat.
    pub const SPEED: f32 = 12.0;
    /// How far a throw carries in its flight, metres: where aim assist looks for a group.
    pub const RANGE: f32 = SPEED * FLIGHT;
    /// Aim assist bends a throw at most this far off the facing, radians (30°).
    pub const ASSIST: f32 = 30.0 * PI / 180.0;
    /// Enemies this close to one another across the floor (centre to centre, metres) belong to one group: about two
    /// fighters' widths, so a crowd jostling round the player is one group and a straggler is not.
    pub const GROUP_GAP: f32 = 1.5;
    /// The share of the spin spent bringing the held thing down from overhead and out to arm's length.
    const DROP: f32 = 0.3;

    /// Of `bodies` (centres and radii), the index of the nearest one a player of `radius` at `apex` facing `yaw` can
    /// grab: inside the blows' cone opening, its edge within [`REACH`] of the player's.
    pub fn pick<I: IntoIterator<Item = ([f32; 2], f32)>>(
        apex: [f32; 2],
        yaw: f32,
        radius: f32,
        bodies: I,
    ) -> Option<usize> {
        let cone = Cone { apex, yaw, reach: radius + REACH };
        let dist = |p: [f32; 2]| (p[0] - apex[0]).hypot(p[1] - apex[1]);
        bodies
            .into_iter()
            .enumerate()
            .filter(|(_, (p, r))| cone.contains(*p, *r))
            .min_by(|a, b| dist(a.1.0).total_cmp(&dist(b.1.0)))
            .map(|(i, _)| i)
    }

    /// Throw aim assist (Q62): the heading a throw from `me` facing `yaw` flies along. Of the enemies within
    /// [`ASSIST`] of the facing and [`RANGE`] of the player, the biggest group (enemies linked by gaps of at most
    /// [`GROUP_GAP`]; the nearer one on a tie) draws the throw toward its centre, never more than [`ASSIST`] off the
    /// facing. With no enemy there, the throw goes straight along the facing.
    pub fn throw_heading(me: [f32; 2], yaw: f32, enemies: &[[f32; 2]]) -> f32 {
        let gap = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).hypot(a[1] - b[1]);
        let ahead: Vec<[f32; 2]> = enemies
            .iter()
            .copied()
            .filter(|p| {
                let d = [p[0] - me[0], p[1] - me[1]];
                let dist = d[0].hypot(d[1]);
                dist > 1e-3 && dist <= RANGE && wrap(d[1].atan2(d[0]) - yaw).abs() <= ASSIST
            })
            .collect();
        // Groups by flood fill: each enemy joins the group of any enemy within the gap.
        let mut group = vec![usize::MAX; ahead.len()];
        let mut groups: Vec<Vec<[f32; 2]>> = Vec::new();
        for seed in 0..ahead.len() {
            if group[seed] != usize::MAX {
                continue;
            }
            let (id, mut open, mut members) = (groups.len(), vec![seed], Vec::new());
            group[seed] = id;
            while let Some(i) = open.pop() {
                members.push(ahead[i]);
                for j in 0..ahead.len() {
                    if group[j] == usize::MAX && gap(ahead[i], ahead[j]) <= GROUP_GAP {
                        group[j] = id;
                        open.push(j);
                    }
                }
            }
            groups.push(members);
        }
        let centre = |m: &[[f32; 2]]| {
            let n = m.len() as f32;
            [m.iter().map(|p| p[0]).sum::<f32>() / n, m.iter().map(|p| p[1]).sum::<f32>() / n]
        };
        let best = groups
            .iter()
            .map(|m| (m.len(), centre(m)))
            .max_by(|a, b| a.0.cmp(&b.0).then(gap(b.1, me).total_cmp(&gap(a.1, me))));
        let Some((_, c)) = best else { return wrap(yaw) };
        let toward = heading([c[0] - me[0], c[1] - me[1]]).unwrap_or(yaw);
        wrap(yaw + wrap(toward - yaw).clamp(-ASSIST, ASSIST))
    }

    /// The held thing's offset from the player's centre: across the floor (a plane vector) and up. Carried
    /// (`spin` `None`), it rides `overhead`. During the spin (`spin` 0..=1) it comes down to the player's centre height
    /// and out to `reach` while it whirls once and a half round the body, from behind, to finish straight ahead along
    /// `yaw`, where it is let go.
    pub fn swing(yaw: f32, overhead: f32, reach: f32, spin: Option<f32>) -> ([f32; 2], f32) {
        let Some(t) = spin.map(|t| t.clamp(0.0, 1.0)) else { return ([0.0, 0.0], overhead) };
        let out = (t / DROP).min(1.0);
        let a = yaw + PI * (1.0 + 3.0 * t);
        ([reach * out * a.cos(), reach * out * a.sin()], overhead * (1.0 - out))
    }

    /// A throw's velocity along `heading`: flat across the floor, m/s.
    pub fn throw_velocity(heading: f32) -> [f32; 2] {
        [SPEED * heading.cos(), SPEED * heading.sin()]
    }

    /// The launch a thrown `held` thing gives each enemy it hits, flying along `heading`: across the floor and up, m/s.
    pub fn knock(held: Held, heading: f32) -> ([f32; 2], f32) {
        shove(impact(held).knockback, heading)
    }

    /// Whether a thrown thing that moved from `from` to `to` this frame passed within `reach` of `target` (on the
    /// floor), so a fast throw on a slow frame can't skip over an enemy.
    pub fn passes(from: [f32; 2], to: [f32; 2], target: [f32; 2], reach: f32) -> bool {
        let d = [to[0] - from[0], to[1] - from[1]];
        let len2 = d[0] * d[0] + d[1] * d[1];
        let t = if len2 > 0.0 {
            (((target[0] - from[0]) * d[0] + (target[1] - from[1]) * d[1]) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (from[0] + d[0] * t - target[0]).hypot(from[1] + d[1] * t - target[1]) <= reach
    }

    #[cfg(test)]
    mod tests {
        use std::f32::consts::{FRAC_PI_2, PI};

        use super::*;

        fn at(angle_deg: f32, dist: f32) -> [f32; 2] {
            let a = angle_deg.to_radians();
            [dist * a.cos(), dist * a.sin()]
        }

        fn deg(rad: f32) -> f32 {
            rad.to_degrees()
        }

        #[test]
        fn a_grab_takes_the_nearest_thing_in_front_within_reach() {
            let r = 0.35;
            let bodies = [(at(0.0, 1.1), 0.35), (at(30.0, 0.8), 0.15), (at(180.0, 0.5), 0.35), (at(90.0, 0.7), 0.2)];
            assert_eq!(pick([0.0, 0.0], 0.0, r, bodies), Some(1), "the nearer of the two in front");
            assert_eq!(pick([0.0, 0.0], 0.0, r, [(at(0.0, 1.5), 0.15)]), None, "out of reach");
            assert_eq!(pick([0.0, 0.0], 0.0, r, [(at(0.0, 1.5), 0.35)]), Some(0), "a wider body's edge is nearer");
            assert_eq!(pick([0.0, 0.0], PI, r, [(at(180.0, 1.0), 0.2)]), Some(0), "turns with the facing");
            assert_eq!(pick([0.0, 0.0], 0.0, r, []), None);
        }

        #[test]
        fn with_nobody_ahead_a_throw_goes_straight_along_the_facing() {
            assert_eq!(throw_heading([1.0, 2.0], 0.4, &[]), 0.4);
            let behind_beside_beyond = [at(180.0, 2.0), at(90.0, 2.0), at(-60.0, 3.0), at(0.0, RANGE + 1.0)];
            assert_eq!(throw_heading([0.0, 0.0], 0.0, &behind_beside_beyond), 0.0);
        }

        #[test]
        fn a_throw_bends_toward_the_biggest_group_not_the_nearest_enemy() {
            // A lone enemy straight ahead and a group of three 20° to the left.
            let mut enemies = vec![at(0.0, 3.0)];
            enemies.extend([at(18.0, 5.0), at(22.0, 5.4), at(20.0, 4.6)]);
            let h = throw_heading([0.0, 0.0], 0.0, &enemies);
            assert!((deg(h) - 20.0).abs() < 1.5, "toward the group's middle: {}", deg(h));
            // Two of them straggle off: one plus one plus one plus the lone one, all singles; the nearest wins.
            let scattered = [at(0.0, 3.0), at(25.0, 5.0), at(-20.0, 8.0)];
            assert!(throw_heading([0.0, 0.0], 0.0, &scattered).abs() < 1e-5, "the nearest of equal groups");
        }

        #[test]
        fn a_group_outside_the_assist_angle_or_range_does_not_pull_the_throw() {
            let mut enemies = vec![at(5.0, 4.0)];
            enemies.extend([at(40.0, 4.0), at(42.0, 4.5), at(38.0, 4.3)]);
            let h = throw_heading([0.0, 0.0], 0.0, &enemies);
            assert!((deg(h) - 5.0).abs() < 1e-3, "the group 40° off doesn't count: {}", deg(h));
            let far = [at(10.0, 3.0), at(-10.0, RANGE + 0.5), at(-11.0, RANGE + 0.8)];
            assert!((deg(throw_heading([0.0, 0.0], 0.0, &far)) - 10.0).abs() < 1e-3, "out of range");
        }

        #[test]
        fn a_group_straddling_the_edge_bends_the_throw_at_most_30_degrees() {
            // Members just inside the edge, linked to more outside it: only those inside count, and the bend is capped.
            let enemies = [at(29.0, 3.0), at(29.5, 4.0), at(28.0, 3.5), at(45.0, 3.2), at(50.0, 3.5)];
            let h = throw_heading([0.0, 0.0], 0.0, &enemies);
            assert!(deg(h) > 25.0 && deg(h) <= 30.0 + 1e-3, "{}", deg(h));
        }

        #[test]
        fn throw_aim_works_from_anywhere_and_across_the_back_seam() {
            // Facing -x (yaw π) from (5, 5): a group just across the -π/π seam.
            let me = [5.0, 5.0];
            let off = |deg: f32, d: f32| {
                let p = at(deg, d);
                [me[0] + p[0], me[1] + p[1]]
            };
            let enemies = [off(-170.0, 3.0), off(-171.0, 3.6), off(-169.0, 3.3), off(176.0, 1.0)];
            let h = throw_heading(me, PI, &enemies);
            assert!((wrap(h - (-170f32).to_radians())).abs() < 0.03, "toward the group across the seam: {}", deg(h));
            assert!((-PI..=PI).contains(&h), "wrapped: {h}");
        }

        #[test]
        fn the_held_thing_rides_overhead_then_whirls_round_to_release_straight_ahead() {
            let (across, up) = swing(0.0, 1.5, 0.9, None);
            assert_eq!(across, [0.0, 0.0]);
            assert_eq!(up, 1.5, "overhead while carried");
            let (across, up) = swing(FRAC_PI_2, 1.5, 0.9, Some(1.0));
            assert!(across[0].abs() < 1e-4 && (across[1] - 0.9).abs() < 1e-4, "ahead along the facing: {across:?}");
            assert!(up.abs() < 1e-6, "at the player's centre height");
            let (across, up) = swing(0.0, 1.5, 0.9, Some(0.3));
            assert!(up.abs() < 1e-6 && (across[0].hypot(across[1]) - 0.9).abs() < 1e-4, "down and out by 30%");
            // Then it sweeps all the way round: every side of the player in turn.
            let sides: Vec<f32> = (3..=10).map(|i| swing(0.0, 1.5, 0.9, Some(i as f32 / 10.0)).0[0]).collect();
            assert!(sides.iter().any(|x| *x < -0.5) && sides.iter().any(|x| *x > 0.5), "{sides:?}");
        }

        #[test]
        fn a_throw_flies_flat_and_fast_and_knocks_enemies_along_its_line() {
            let v = throw_velocity(FRAC_PI_2);
            assert!(v[0].abs() < 1e-4 && (v[1] - SPEED).abs() < 1e-4);
            const { assert!(SPEED > crate::plane::DASH_SPEED * 0.5 && RANGE > 8.0, "it crosses most of the box") };
            let (across, up) = knock(Held::Enemy, 0.0);
            let (debris, _) = knock(Held::Debris, 0.0);
            assert!(across[0] > debris[0] && across[1].abs() < 1e-5 && up > 0.0, "a body knocks harder");
        }

        #[test]
        fn a_fast_throw_hits_what_it_passes_between_frames() {
            assert!(passes([0.0, 0.0], [3.0, 0.0], [1.5, 0.6], 0.7), "passed it mid-frame");
            assert!(!passes([0.0, 0.0], [3.0, 0.0], [1.5, 0.8], 0.7), "passed beside it");
            assert!(!passes([0.0, 0.0], [3.0, 0.0], [4.0, 0.0], 0.7), "not reached yet");
            assert!(passes([1.0, 1.0], [1.0, 1.0], [1.2, 1.0], 0.7), "standing still on it");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_throw_flies_forward_and_up() {
        let (x, y) = throw_velocity(-1.0);
        assert!(x < 0.0 && y > 0.0);
        assert!(x.abs() > 2.0 * crate::movement::RUN_SPEED, "much faster than running");
    }

    #[test]
    fn a_thrown_enemy_hits_harder_than_a_debris_part_and_is_hurt_itself() {
        let (e, d) = (impact(Held::Enemy), impact(Held::Debris));
        assert!(e.damage > d.damage && e.knockback.0 > d.knockback.0);
        assert!(e.self_damage > 0.0 && d.self_damage == 0.0);
        assert!(e.damage > crate::combat::Move::Punch.spec().damage, "a throw beats a punch");
    }

    #[test]
    fn the_held_thing_rides_overhead_then_swings_round_the_body_to_release_in_front() {
        let (x, y, _) = hold_offset(1.0, 28.0, None);
        assert!(x.abs() < 1e-3 && y > 28.0, "overhead");
        let (x, y, z) = hold_offset(1.0, 28.0, Some(0.3));
        assert!(x < -28.0 && y < 28.0 && z.abs() < 1e-3, "behind the shoulders: {x}, {y}");
        let (x, y, z) = hold_offset(1.0, 28.0, Some(0.65));
        assert!(
            x.abs() < 1.0 && z > 28.0 && y > 0.0,
            "passing in front of the body, toward the viewer, never below it"
        );
        let (x, _, z) = hold_offset(1.0, 28.0, Some(1.0));
        assert!(x > 28.0 && z.abs() < 1.0, "straight ahead at release");
        assert!(hold_offset(-1.0, 28.0, Some(1.0)).0 < -28.0, "ahead when facing left too");
    }
}
