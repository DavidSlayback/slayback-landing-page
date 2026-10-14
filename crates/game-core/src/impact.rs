//! How a hit feels, shared by both games: screen-shake trauma, the comic star burst's size and pop, and the squash on
//! whoever takes the blow. All of it runs on real time, so it plays through hit-stop: the frozen frame is the squashed,
//! burst-lit, shaking one.

use crate::combat::Move;

/// How long a struck fighter stays squashed, seconds.
pub const SQUASH_TIME: f32 = 0.14;
/// Trauma from a bomb blast.
pub const BLAST_TRAUMA: f32 = 0.8;
/// A blast's star burst radius, world units: twice a finishing drop-kick's.
pub const BLAST_BURST: f32 = 108.0;
/// Trauma drained per second.
const DRAIN: f32 = 1.8;

/// A burst's radius for one blow, in the 2.5D game's world units (a fodder is 52 tall; divide by
/// `plane::UNITS_PER_METRE` for metres).
pub fn burst_size(mv: Move, finisher: bool) -> f32 {
    let base = match mv {
        Move::Punch | Move::Slash => 18.0,
        Move::Kick => 26.0,
        Move::DropKick => 30.0,
        Move::Smash => 32.0,
    };
    if finisher { base * 1.8 } else { base }
}

/// A burst's scale over its life (`t` from 0 to 1): it snaps open in the first 30%, then shrinks away.
pub fn pop(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.3 { t / 0.3 } else { 1.0 - ((t - 0.3) / 0.7).powi(2) }
}

/// The squash on a struck fighter's figure (x, y, z scale) at `left` seconds remaining: flattened and widened at the
/// moment of the blow, back to normal as it runs out.
pub fn squash(left: f32) -> [f32; 3] {
    let k = (left / SQUASH_TIME).clamp(0.0, 1.0);
    [1.0 + 0.18 * k, 1.0 - 0.18 * k, 1.0 + 0.18 * k]
}

/// Trauma from one blow: heavier moves shake more, and a finishing blow most of all.
pub fn trauma(mv: Move, finisher: bool) -> f32 {
    let base = match mv {
        Move::Punch => 0.18,
        Move::Slash => 0.2,
        Move::Kick => 0.3,
        Move::DropKick => 0.38,
        Move::Smash => 0.42,
    };
    base + if finisher { 0.26 } else { 0.0 }
}

/// Screen shake as trauma (0 to 1): impacts add to it, it drains over time, and the shake grows with its square, so a
/// jab barely moves the view while a kill or a blast jolts it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shake {
    pub trauma: f32,
}

impl Shake {
    pub fn add(&mut self, amount: f32) {
        self.trauma = (self.trauma + amount).min(1.0);
    }

    pub fn drain(&mut self, dt: f32) {
        self.trauma = (self.trauma - DRAIN * dt).max(0.0);
    }

    /// The view's offset at time `t` (seconds), as a fraction of the full shake (each game scales it): two unrelated
    /// sines per axis read as noise without a noise crate.
    pub fn offset(&self, t: f32) -> [f32; 2] {
        let a = self.trauma * self.trauma;
        [a * (0.6 * (t * 47.0).sin() + 0.4 * (t * 71.0).sin()), a * (0.6 * (t * 53.0).cos() + 0.4 * (t * 89.0).sin())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heavier_blows_and_kills_burst_bigger() {
        assert!(burst_size(Move::Kick, false) > burst_size(Move::Punch, false));
        assert!(burst_size(Move::Punch, true) > burst_size(Move::Smash, false), "a kill outshines any plain hit");
    }

    #[test]
    fn a_burst_snaps_open_then_shrinks_away() {
        assert_eq!(pop(0.0), 0.0);
        assert_eq!(pop(0.3), 1.0, "fully open at 30% of its life");
        assert!(pop(0.6) < 1.0 && pop(0.6) > pop(0.9));
        assert_eq!(pop(1.0), 0.0);
    }

    #[test]
    fn the_squash_flattens_at_the_blow_and_recovers() {
        let hit = squash(SQUASH_TIME);
        assert!(hit[1] < 1.0 && hit[0] > 1.0 && hit[2] > 1.0, "flattened and widened");
        assert_eq!(squash(0.0), [1.0; 3]);
    }

    #[test]
    fn shake_grows_with_the_square_of_trauma_and_drains_away() {
        let peak = |trauma: f32| {
            let s = Shake { trauma };
            (0..200).map(|i| s.offset(i as f32 * 0.01)).map(|[x, y]| x.hypot(y)).fold(0.0, f32::max)
        };
        assert_eq!(peak(0.0), 0.0, "no trauma, no shake");
        assert!(peak(1.0) > 3.5 * peak(0.5), "quadratic: a light knock barely moves the view");
        let mut s = Shake::default();
        s.add(0.7);
        s.add(0.7);
        assert_eq!(s.trauma, 1.0, "capped");
        s.drain(1.0);
        s.drain(1.0);
        assert_eq!(s.trauma, 0.0, "gone within about half a second per unit");
    }

    #[test]
    fn heavier_blows_and_finishers_shake_harder() {
        assert!(trauma(Move::Kick, false) > trauma(Move::Punch, false));
        assert!(trauma(Move::Punch, true) > trauma(Move::Smash, false), "any kill outweighs any plain hit");
    }
}
