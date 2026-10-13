//! Enemy decision-making, kept free of physics: given where the player is, what does the enemy want to do?

use crate::combat::Move;

/// What an enemy wants to do this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Intent {
    /// Walk with this input axis (-1..1).
    Walk(f32),
    /// Stand still (winding up, cooling down, or stunned).
    Hold,
    Attack(Move),
}

/// "Fodder": walks straight at the player, telegraphs, then punches; waits between punches.
#[derive(Clone, Copy, Debug, Default)]
pub struct FodderBrain {
    pub windup: f32,
    pub cooldown: f32,
}

impl FodderBrain {
    /// Horizontal centre distance at which fodder stops and starts its windup.
    pub const RANGE: f32 = 52.0;
    /// Telegraph time before the punch comes out.
    pub const WINDUP: f32 = 0.35;
    pub const COOLDOWN: f32 = 0.65;
    /// Fraction of the player's run speed.
    pub const WALK: f32 = 0.5;

    /// `dx` is player.x - self.x. `busy` means a swing is already in progress.
    pub fn decide(&mut self, dx: f32, dt: f32, stunned: bool, busy: bool) -> Intent {
        if stunned || busy {
            self.windup = 0.0; // a hit (or its own swing) interrupts the telegraph
            return Intent::Hold;
        }
        self.cooldown = (self.cooldown - dt).max(0.0);
        if dx.abs() > Self::RANGE {
            self.windup = 0.0;
            return Intent::Walk(dx.signum() * Self::WALK);
        }
        if self.cooldown > 0.0 {
            return Intent::Hold;
        }
        self.windup += dt;
        if self.windup >= Self::WINDUP {
            self.windup = 0.0;
            self.cooldown = Self::COOLDOWN;
            Intent::Attack(Move::Punch)
        } else {
            Intent::Hold
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    #[test]
    fn walks_toward_a_distant_player() {
        let mut b = FodderBrain::default();
        assert_eq!(b.decide(300.0, DT, false, false), Intent::Walk(FodderBrain::WALK));
        assert_eq!(b.decide(-300.0, DT, false, false), Intent::Walk(-FodderBrain::WALK));
    }

    #[test]
    fn telegraphs_before_punching_a_nearby_player() {
        let mut b = FodderBrain::default();
        let frames_before_punch = (0..60).take_while(|_| b.decide(30.0, DT, false, false) == Intent::Hold).count();
        let expected = (FodderBrain::WINDUP / DT).ceil() as usize - 1;
        assert!(frames_before_punch.abs_diff(expected) <= 1, "punched after {frames_before_punch} frames");
    }

    #[test]
    fn waits_out_its_cooldown_between_punches() {
        let mut b = FodderBrain::default();
        let mut punches = 0;
        for _ in 0..(2.0 / DT) as usize {
            if b.decide(30.0, DT, false, false) == Intent::Attack(Move::Punch) {
                punches += 1;
            }
        }
        // one cycle = windup + cooldown = 1.0 s, so two seconds fit exactly two punches
        assert_eq!(punches, 2);
    }

    #[test]
    fn a_stunned_or_busy_enemy_does_nothing_and_loses_its_windup() {
        let mut b = FodderBrain::default();
        for _ in 0..10 {
            b.decide(30.0, DT, false, false);
        }
        assert_eq!(b.decide(30.0, DT, true, false), Intent::Hold);
        assert_eq!(b.windup, 0.0, "getting hit should interrupt the telegraph");
        assert_eq!(b.decide(30.0, DT, false, true), Intent::Hold);
    }
}
