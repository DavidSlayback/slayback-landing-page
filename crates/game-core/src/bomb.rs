//! Bomb-heads (decision Q32): from box 2 any enemy can carry a bomb for a head. On defeat the bomb is lit: a 1.5 s
//! blinking fuse, then a blast that hurts nearby enemies and the player and scatters debris. The blast never deletes
//! debris (the pile never loses progress outright); it only knocks parts loose. A lit bomb can be grabbed and thrown.
//!
//! The blast is in metres (plane vectors as in [`crate::plane`]), by the full 3D distance from its centre, so a bomb
//! on a pile doesn't reach the floor far below.

use crate::aim::shove;
use crate::combat::Blow;
use crate::plane::heading;

/// Seconds from defeat to blast.
pub const FUSE: f32 = 1.5;
/// Damage at the centre and at the edge of the blast.
pub const DAMAGE: (f32, f32) = (32.0, 8.0);
/// Launch speed at the centre of the blast, m/s (falling off to a third at the edge), before `aim::shove` scales it
/// as it scales a blow's knockback.
pub const PUSH: f32 = 34.375;
/// How fast a blast lifts what it pushes, as a share of the push.
pub const LIFT: f32 = 0.35;
/// How soon a lit bomb caught in another's blast goes off, seconds (the 3D game's chains).
pub const CHAIN: f32 = 0.15;
/// Hit-stop when a bomb goes off, seconds.
pub const HITSTOP: f32 = 0.12;
/// How long a blast stuns those it hurts, seconds (as in the 2.5D game).
pub const STUN: f32 = 0.4;

/// A lit fuse.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fuse {
    pub left: f32,
}

impl Default for Fuse {
    fn default() -> Self {
        Self { left: FUSE }
    }
}

impl Fuse {
    /// Burns down; returns true on the frame it goes off.
    pub fn tick(&mut self, dt: f32) -> bool {
        let was = self.left;
        self.left -= dt;
        was > 0.0 && self.left <= 0.0
    }

    /// Caught in another bomb's blast: it goes off [`CHAIN`] seconds later (or sooner, if its own fuse is shorter),
    /// so a chain ripples out from bomb to bomb rather than going off in one frame.
    pub fn chain(&mut self) {
        self.left = self.left.min(CHAIN);
    }

    /// Whether the bomb's light is on this instant: the blink speeds up as the fuse runs out.
    pub fn blink(&self) -> bool {
        let burned = (FUSE - self.left).max(0.0);
        // 3 Hz rising to 12 Hz: the phase is the integral of a linearly rising frequency.
        let phase = 3.0 * burned + 0.5 * (12.0 - 3.0) / FUSE * burned * burned;
        phase.fract() < 0.5
    }
}

/// Blast radius, metres. Not the 2.5D radius converted (4.4 m): that would cover two-fifths of the 15 × 10 m box.
/// Three metres covers a crowd (enemies within aim assist's 1.5 m group gap of one another) and leaves the player
/// room to run from a lit bomb in its fuse (6 m/s for 1.5 s).
pub const RADIUS: f32 = 3.0;
/// Nearer the centre than this, metres, all of the push goes up (a thousandth of a unit of the 2.5D blast's 140).
const DEAD_CENTRE: f32 = RADIUS * 1e-3 / 140.0;

/// Falloff from 1 at the centre to 0 at the edge, or `None` outside the blast.
fn reach(distance: f32) -> Option<f32> {
    (distance <= RADIUS).then(|| 1.0 - distance / RADIUS)
}

/// Damage to a body whose centre is `distance` metres from the blast, or `None` if out of range.
pub fn damage(distance: f32) -> Option<f32> {
    reach(distance).map(|k| DAMAGE.1 + (DAMAGE.0 - DAMAGE.1) * k)
}

/// The launch for something `across` (a plane vector) and `up` from the blast, or `None` out of range: straight
/// away across the floor at [`PUSH`] (strongest at the centre) and up at [`LIFT`] of that, scaled as blows are by
/// [`shove`]. Dead above or below the blast it goes straight up.
pub fn push(across: [f32; 2], up: f32) -> Option<([f32; 2], f32)> {
    let distance = across[0].hypot(across[1]).hypot(up);
    let speed = PUSH * (1.0 / 3.0 + 2.0 / 3.0 * reach(distance)?);
    let (away, lift) = if distance > DEAD_CENTRE { (speed, LIFT * speed) } else { (0.0, speed + LIFT * speed) };
    Some(match heading(across) {
        Some(yaw) => shove((away, lift), yaw),
        None => shove((0.0, lift), 0.0),
    })
}

/// The blow a blast deals a fighter `across` (a plane vector) and `up` from its centre, or `None` out of range. It
/// reaches even what the player holds, armour never shrugs it off, and it raises no hit of its own: the blast itself
/// freezes the game once ([`HITSTOP`]) and is the spectacle.
pub fn blow(across: [f32; 2], up: f32) -> Option<Blow> {
    let distance = across[0].hypot(across[1]).hypot(up);
    Some(Blow {
        damage: damage(distance)?,
        stun: STUN,
        launch: push(across, up)?,
        freeze: None,
        shrugged_by: None,
        reaches_held: true,
        feel: None,
    })
}

#[cfg(test)]
mod blast_tests {
    use super::*;

    #[test]
    fn the_3d_blast_hurts_by_distance_and_reaches_its_radius_in_any_direction() {
        assert_eq!(damage(0.0), Some(super::DAMAGE.0));
        assert_eq!(damage(RADIUS), Some(super::DAMAGE.1));
        assert_eq!(damage(RADIUS + 0.01), None);
        assert!(damage(1.0).unwrap() > damage(2.0).unwrap());
        assert!(push([0.0, 0.0], RADIUS + 0.01).is_none(), "out of range straight up too");
        assert!(push([2.1, 2.1], 0.0).is_some() && push([2.2, 2.2], 0.0).is_none(), "on a diagonal across the floor");
    }

    #[test]
    fn the_3d_blast_pushes_away_across_the_floor_and_up_harder_near_the_centre() {
        let (across, up) = push([1.0, 0.0], 0.8).unwrap();
        assert!(across[0] > 0.0 && across[1].abs() < 1e-5 && up > 0.0, "away and up: {across:?} {up}");
        let (across, _) = push([0.0, -1.0], 0.0).unwrap();
        assert!(across[1] < 0.0 && across[0].abs() < 1e-5, "toward the camera when in front of it");
        let near = push([0.5, 0.0], 0.0).unwrap();
        let far = push([2.5, 0.0], 0.0).unwrap();
        assert!(near.0[0] > 1.5 * far.0[0] && near.1 > far.1, "near {near:?}, far {far:?}");
        let (across, up) = push([0.0, 0.0], 0.5).unwrap();
        assert!(across == [0.0, 0.0] && up > 0.0, "dead above: straight up");
        let drop_kick = crate::aim::launch(crate::combat::Move::DropKick, 0.0).0[0];
        assert!(push([0.1, 0.0], 0.0).unwrap().0[0] > drop_kick, "the centre launches harder than any blow");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bomb_caught_in_a_blast_goes_off_soon_after_but_never_later_than_its_own_fuse() {
        let mut f = Fuse::default();
        f.chain();
        assert_eq!(f.left, CHAIN);
        let mut nearly = Fuse { left: 0.05 };
        nearly.chain();
        assert_eq!(nearly.left, 0.05);
    }

    #[test]
    fn the_fuse_burns_for_a_second_and_a_half_then_goes_off_once() {
        let mut f = Fuse::default();
        let mut went_off = Vec::new();
        for i in 0..120 {
            if f.tick(1.0 / 60.0) {
                went_off.push(i);
            }
        }
        assert_eq!(went_off.len(), 1);
        assert!((went_off[0] as f32 / 60.0 - FUSE).abs() < 0.03);
    }

    #[test]
    fn the_blink_speeds_up_as_the_fuse_runs_out() {
        let flips = |from: f32, to: f32| {
            let mut n = 0;
            let mut last = None;
            let mut t = from;
            while t < to {
                let b = Fuse { left: FUSE - t }.blink();
                if last.is_some_and(|l| l != b) {
                    n += 1;
                }
                last = Some(b);
                t += 0.002;
            }
            n
        };
        assert!(flips(1.0, 1.5) > 2 * flips(0.0, 0.5), "faster at the end");
    }
}
