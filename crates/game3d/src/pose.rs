//! How a figure moves, after Rumble Box (Q65-Q69, `docs/research/rumble-box-reference.md`): figures are loose parts
//! that fly into position. A punch snaps the arm's parts out along a line to full reach in a few frames, holds, and
//! pulls back; a kick does the same with a leg; a drop-kick lays the body back with both feet out; a held thing goes
//! up in both fists; the walk is a big, snappy step with a bob.
//!
//! Everything here is a plain function of the figure's state, so it is tested headless; `look` (the player) and
//! `figure` (enemies) apply it. Figure space: +x forward, +y up, +z the right-hand side; offsets are metres, added to
//! each part's rest position.

use bevy::prelude::*;
use game_core::combat::{Move, Swing};
use game_core::debris::solid::Limb;

/// What the figure is doing besides walking.
#[derive(Clone, Copy, Debug)]
pub enum Act {
    None,
    /// A blow, `side` the arm or leg it uses (+1 right, -1 left).
    Strike {
        swing: Swing,
        side: f32,
    },
    /// Something held overhead in both fists.
    Hold,
    /// Both arms out, whirling a held thing round (the body turns with it, in `look`).
    Whirl,
    /// The follow-through after a throw, `t` seconds since the release.
    Release {
        t: f32,
    },
}

/// How long a throw's follow-through lasts, seconds.
pub const RELEASE: f32 = 0.25;

/// +1 for a right-hand limb, -1 for a left, 0 for the trunk.
pub fn side(limb: Limb) -> f32 {
    match limb {
        Limb::RightArm | Limb::RightLeg => 1.0,
        Limb::LeftArm | Limb::LeftLeg => -1.0,
        Limb::Trunk => 0.0,
    }
}

fn arm(limb: Limb) -> bool {
    matches!(limb, Limb::LeftArm | Limb::RightArm)
}

fn leg(limb: Limb) -> bool {
    matches!(limb, Limb::LeftLeg | Limb::RightLeg)
}

/// How far out a blow is, 0 at rest to 1 at full reach: out fast (Rumble Box's punch reaches full extension in ~80
/// ms), held through the active window, then pulled back early in the recovery; eased at both ends.
pub fn extension(swing: &Swing) -> f32 {
    let s = swing.mv.spec();
    let t = swing.elapsed;
    let out = (s.startup + s.active * 0.5).max(1e-3);
    if t < out {
        smooth(t / out)
    } else if t < s.startup + s.active {
        1.0
    } else {
        // Snapped back over the first 60% of the recovery (Rumble Box pulls back within a few frames), then still.
        let x = ((t - s.startup - s.active) / (0.6 * s.recovery).max(1e-3)).clamp(0.0, 1.0);
        1.0 - smooth(x)
    }
}

/// Smoothstep: eases in and out, so a blow starts and stops without a jolt.
fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// How fast a part follows its pose, per second: every part eases toward where `part` puts it (a 33 ms time constant,
/// two frames), so changes of action, starting and stopping, and the odd long frame never jump.
pub const FOLLOW: f32 = 30.0;

/// The share of the way to its pose a part moves this frame.
pub fn follow(dt: f32) -> f32 {
    1.0 - (-FOLLOW * dt).exp()
}

/// A walk cycle: big alternating steps that linger at each end of the stride, smooth throughout (a sine with a
/// little of its third harmonic).
fn step(phase: f32) -> f32 {
    (phase.sin() + 0.15 * (3.0 * phase).sin()) / 0.85
}

/// The figure as a whole: lifted (metres) and turned (lean and twist) on top of its facing. A turn about +z by a
/// positive angle tips the head back (toward -x); about +y, it brings the right side (+z) forward.
pub fn body(phase: f32, amount: f32, act: Act) -> (Vec3, Quat) {
    let bob = Vec3::Y * 0.06 * amount * phase.sin().abs();
    match act {
        Act::Strike { swing, side } => {
            let e = extension(&swing);
            match swing.mv {
                // Laid back with both feet out, off the floor.
                Move::DropKick => (bob + Vec3::Y * 0.25 * e, Quat::from_rotation_z(1.0 * e)),
                // Leaning back from the kicking leg.
                Move::Kick => (bob, Quat::from_rotation_z(0.25 * e)),
                // A thug's overhead blow comes down from a lean back into a lean forward.
                Move::Smash => (bob, Quat::from_rotation_z(-0.35 * e)),
                // Punches and slashes twist into the blow and lean into it.
                _ => (bob, Quat::from_rotation_y(0.35 * side * e) * Quat::from_rotation_z(-0.12 * e)),
            }
        }
        _ => (bob, Quat::IDENTITY),
    }
}

/// One part's offset from rest: `reach` is how far down its limb it sits (0 at the shoulder or hip, 1 at the fist or
/// foot); `rest` is where it rests, `phase` and `amount` (0..=1, by speed) the walk.
pub fn part(limb: Limb, reach: f32, rest: Vec3, phase: f32, amount: f32, act: Act) -> Vec3 {
    let s = side(limb);
    let gait = step(phase) * s;
    let walk = if leg(limb) {
        // A foot steps out and lifts as it comes forward.
        let lift = (phase.cos() * s).max(0.0) * 0.14 * reach;
        Vec3::new(0.28 * gait * (0.3 + 0.7 * reach), lift, 0.0) * amount
    } else if arm(limb) {
        // Arms swing against the legs.
        Vec3::X * -0.2 * gait * (0.3 + 0.7 * reach) * amount
    } else {
        Vec3::ZERO
    };
    let line = |e: f32, x: f32, y: f32| {
        // Toward the line through the figure's middle, up to its height `y`, out to `x` ahead, by how far down the
        // limb the part is: the parts spread out evenly along the arm or leg.
        Vec3::new(x * (0.3 + 0.7 * reach), (y - rest.y) * reach, -rest.z * 0.5 * reach) * e
    };
    match act {
        Act::Strike { swing, side: hits } => {
            let e = extension(&swing);
            let mine = s == hits;
            match swing.mv {
                Move::Punch | Move::Slash if arm(limb) && mine => line(e, 0.75, 0.2),
                // The other arm pulls back to guard.
                Move::Punch | Move::Slash if arm(limb) => walk * (1.0 - e) + Vec3::new(-0.12, 0.06, 0.0) * e,
                Move::Kick if leg(limb) && mine => line(e, 0.7, -0.35),
                Move::Kick if arm(limb) => walk * (1.0 - e) + Vec3::new(-0.15, 0.12, 0.0) * e,
                // Both feet out, both arms flung back.
                Move::DropKick if leg(limb) => line(e, 0.6, -0.45),
                Move::DropKick if arm(limb) => Vec3::new(-0.25, 0.15, 0.0) * e,
                // Overhead and down: up past the head, then ahead at chest height.
                Move::Smash if arm(limb) && mine => line(e, 0.6, 0.35) + Vec3::Y * 0.4 * reach * (1.0 - e),
                _ => walk,
            }
        }
        Act::Hold if arm(limb) => line(1.0, 0.05, 0.95),
        Act::Whirl if arm(limb) => line(1.0, 0.6, 0.15),
        Act::Release { t } if arm(limb) => line((1.0 - t / RELEASE).clamp(0.0, 1.0), 0.75, 0.2),
        _ => walk,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(mv: Move, t: f32) -> Swing {
        let mut s = Swing::new(mv);
        s.advance(t);
        s
    }

    const SHOULDER: Vec3 = Vec3::new(0.0, 0.24, 0.42);
    const FIST: Vec3 = Vec3::new(0.06, 0.0, 0.44);

    #[test]
    fn a_punch_reaches_full_extension_in_under_a_tenth_of_a_second_holds_then_pulls_back() {
        let s = Move::Punch.spec();
        assert!(extension(&at(Move::Punch, 0.0)) < 0.05);
        let full = s.startup + s.active * 0.5;
        assert!(full < 0.1, "Rumble Box: full reach in ~80 ms; ours {full}");
        assert!((extension(&at(Move::Punch, full)) - 1.0).abs() < 1e-4);
        assert!((extension(&at(Move::Punch, s.startup + s.active - 0.001)) - 1.0).abs() < 1e-4, "holds while active");
        let end = s.startup + s.active + s.recovery;
        assert!(extension(&at(Move::Punch, end)) < 1e-4, "back by the end of the recovery");
        let back = s.startup + s.active + 0.6 * s.recovery;
        assert!(extension(&at(Move::Punch, back)) < 1e-4, "snapped back by 60% of the recovery");
        assert!(extension(&at(Move::Punch, s.startup + s.active + 0.1 * s.recovery)) > 0.6, "but not instantly");
    }

    #[test]
    fn the_punching_arm_lines_up_ahead_fist_furthest_and_the_other_arm_guards() {
        let s = Move::Punch.spec();
        let act = Act::Strike { swing: at(Move::Punch, s.startup + s.active / 2.0), side: 1.0 };
        let fist = FIST + part(Limb::RightArm, 1.0, FIST, 0.0, 0.0, act);
        let shoulder = SHOULDER + part(Limb::RightArm, 0.35, SHOULDER, 0.0, 0.0, act);
        assert!(fist.x > 0.6 && fist.x > shoulder.x + 0.3, "fist out ahead of the shoulder: {fist} {shoulder}");
        assert!(fist.z.abs() < FIST.z.abs(), "toward the middle: {fist}");
        let guard = part(Limb::LeftArm, 1.0, FIST * Vec3::new(1.0, 1.0, -1.0), 0.0, 0.0, act);
        assert!(guard.x < 0.0, "the left fist pulls back: {guard}");
    }

    #[test]
    fn a_kick_lifts_the_foot_out_ahead_and_a_drop_kick_lays_the_body_back() {
        let s = Move::Kick.spec();
        let act = Act::Strike { swing: at(Move::Kick, s.startup + s.active / 2.0), side: -1.0 };
        let foot = Vec3::new(0.04, -0.78, -0.15);
        let out = part(Limb::LeftLeg, 1.0, foot, 0.0, 0.0, act);
        assert!(out.x > 0.6 && out.y > 0.3, "{out}");
        assert_eq!(part(Limb::RightLeg, 1.0, foot * Vec3::new(1.0, 1.0, -1.0), 0.0, 0.0, act), Vec3::ZERO);
        let d = Move::DropKick.spec();
        let (lift, lean) = body(0.0, 0.0, Act::Strike { swing: at(Move::DropKick, d.active / 2.0), side: 1.0 });
        assert!(lift.y > 0.2 && (lean * Vec3::Y).x < -0.6, "laid back, off the floor: {lift} {}", lean * Vec3::Y);
    }

    #[test]
    fn walking_steps_the_feet_in_turn_and_bobs_and_standing_still_rests() {
        let foot = |limb, phase| part(limb, 1.0, Vec3::new(0.0, -0.78, 0.0), phase, 1.0, Act::None);
        let quarter = std::f32::consts::FRAC_PI_2;
        assert!(foot(Limb::RightLeg, quarter).x > 0.2 && foot(Limb::LeftLeg, quarter).x < -0.2, "in turn");
        assert!(body(quarter, 1.0, Act::None).0.y > 0.05, "bobs");
        assert_eq!(part(Limb::RightLeg, 1.0, Vec3::ZERO, quarter, 0.0, Act::None), Vec3::ZERO, "still at rest");
    }

    #[test]
    fn a_punch_twists_its_shoulder_forward() {
        let s = Move::Punch.spec();
        let (_, turn) = body(0.0, 0.0, Act::Strike { swing: at(Move::Punch, s.startup), side: 1.0 });
        assert!((turn * SHOULDER).x > SHOULDER.x + 0.1, "{}", turn * SHOULDER);
    }

    #[test]
    fn a_held_thing_goes_up_in_both_fists() {
        for limb in [Limb::LeftArm, Limb::RightArm] {
            let rest = FIST * Vec3::new(1.0, 1.0, side(limb));
            let up = rest + part(limb, 1.0, rest, 0.0, 0.0, Act::Hold);
            assert!(up.y > 0.9, "{limb:?}: {up}");
        }
    }
}
