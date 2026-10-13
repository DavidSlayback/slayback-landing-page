//! Grab and spin-throw: the player picks up a live enemy, a debris part (frozen debris thaws) or a lit bomb, and whirls it into
//! the crowd. A thrown thing is a projectile for a moment: it hurts every enemy it passes through, then lands as
//! whatever it was (a stunned enemy, or loose debris that freezes again).

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
