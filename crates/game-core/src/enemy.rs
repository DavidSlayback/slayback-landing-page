//! Enemy decision-making, kept free of physics: given where the player is, what does the enemy want to do?
//!
//! Fodder and thugs fight the same way (walk up, telegraph, strike, cool down) with different numbers, so they share
//! one [`MeleeBrain`] driven by a [`Style`].

use crate::combat::Move;

/// The enemy types (decision Q26; bomb-heads are a modifier on any of them).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Fodder,
    Thug,
}

/// What an enemy wants to do this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Intent {
    /// Walk with this input axis (-1..1).
    Walk(f32),
    /// Stand still (winding up, cooling down, or stunned).
    Hold,
    Attack(Move),
}

/// Which blows an enemy shrugs off: it still takes the damage, but isn't staggered (no stun, no knockback, and its
/// telegraph keeps going).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Armour {
    None,
    /// Punches don't interrupt it; kicks, drop-kicks and throws do.
    Punches,
}

/// Whether `mv` staggers an enemy with `armour`.
pub fn staggers(mv: Move, armour: Armour) -> bool {
    !(armour == Armour::Punches && mv == Move::Punch)
}

/// How a melee enemy looks for a fight: its body, toughness, pace and blow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    pub kind: Kind,
    /// Collider half-size, world units.
    pub half: (f32, f32),
    pub max_hp: f32,
    /// Horizontal centre distance at which it stops and starts its telegraph.
    pub range: f32,
    /// Telegraph time before the blow comes out.
    pub windup: f32,
    pub cooldown: f32,
    /// Fraction of the player's run speed.
    pub walk: f32,
    pub attack: Move,
    pub armour: Armour,
}

/// Walks straight at the player, telegraphs briefly, then punches.
pub const FODDER: Style = Style {
    kind: Kind::Fodder,
    half: (15.0, 26.0),
    max_hp: 30.0,
    range: 52.0,
    windup: 0.35,
    cooldown: 0.65,
    walk: 0.5,
    attack: Move::Punch,
    armour: Armour::None,
};

/// Slow, tough and wide: a long, obvious wind-up into an overhead smash that punches can't interrupt.
pub const THUG: Style = Style {
    kind: Kind::Thug,
    half: (24.0, 30.0),
    max_hp: 60.0,
    range: 66.0,
    windup: 0.6,
    cooldown: 1.0,
    walk: 0.3,
    attack: Move::Smash,
    armour: Armour::Punches,
};

impl Kind {
    pub fn style(self) -> Style {
        match self {
            Kind::Fodder => FODDER,
            Kind::Thug => THUG,
        }
    }
}

/// Walk up, telegraph, strike, cool down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeleeBrain {
    pub style: Style,
    pub windup: f32,
    pub cooldown: f32,
}

impl MeleeBrain {
    pub fn new(style: Style) -> Self {
        Self { style, windup: 0.0, cooldown: 0.0 }
    }

    /// How far into its telegraph it is, 0..=1 (for the windup glow).
    pub fn telegraph(&self) -> f32 {
        (self.windup / self.style.windup).clamp(0.0, 1.0)
    }

    /// `dx` is player.x - self.x. `stunned` means a blow staggered it; `busy` means its own swing is under way.
    pub fn decide(&mut self, dx: f32, dt: f32, stunned: bool, busy: bool) -> Intent {
        let s = self.style;
        if stunned || busy {
            self.windup = 0.0; // a staggering hit (or its own swing) interrupts the telegraph
            return Intent::Hold;
        }
        self.cooldown = (self.cooldown - dt).max(0.0);
        if dx.abs() > s.range {
            self.windup = 0.0;
            return Intent::Walk(dx.signum() * s.walk);
        }
        if self.cooldown > 0.0 {
            return Intent::Hold;
        }
        self.windup += dt;
        if self.windup >= s.windup {
            self.windup = 0.0;
            self.cooldown = s.cooldown;
            Intent::Attack(s.attack)
        } else {
            Intent::Hold
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn fodder() -> MeleeBrain {
        MeleeBrain::new(FODDER)
    }

    #[test]
    fn walks_toward_a_distant_player() {
        let mut b = fodder();
        assert_eq!(b.decide(300.0, DT, false, false), Intent::Walk(FODDER.walk));
        assert_eq!(b.decide(-300.0, DT, false, false), Intent::Walk(-FODDER.walk));
    }

    #[test]
    fn telegraphs_before_punching_a_nearby_player() {
        let mut b = fodder();
        let frames_before_punch = (0..60).take_while(|_| b.decide(30.0, DT, false, false) == Intent::Hold).count();
        let expected = (FODDER.windup / DT).ceil() as usize - 1;
        assert!(frames_before_punch.abs_diff(expected) <= 1, "punched after {frames_before_punch} frames");
    }

    #[test]
    fn waits_out_its_cooldown_between_punches() {
        let mut b = fodder();
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
        let mut b = fodder();
        for _ in 0..10 {
            b.decide(30.0, DT, false, false);
        }
        assert_eq!(b.decide(30.0, DT, true, false), Intent::Hold);
        assert_eq!(b.windup, 0.0, "getting hit should interrupt the telegraph");
        assert_eq!(b.decide(30.0, DT, false, true), Intent::Hold);
    }

    #[test]
    fn a_thug_is_slower_tougher_and_wider_than_fodder_with_a_longer_telegraph() {
        let (thug, fodder) = (Kind::Thug.style(), Kind::Fodder.style());
        assert!(thug.walk < fodder.walk && thug.max_hp >= 2.0 * fodder.max_hp && thug.half.0 > fodder.half.0);
        assert!(thug.windup > fodder.windup);
        let mut b = MeleeBrain::new(THUG);
        let frames = (0..120).take_while(|_| b.decide(40.0, DT, false, false) == Intent::Hold).count();
        assert!(frames as f32 * DT >= 0.55, "a big, readable wind-up: {frames} frames");
        assert_eq!(b.decide(40.0, DT, false, false), Intent::Hold, "then cools down");
    }

    #[test]
    fn punches_dont_stagger_a_thug_but_kicks_and_throws_do() {
        assert!(!staggers(Move::Punch, THUG.armour));
        assert!(staggers(Move::Kick, THUG.armour) && staggers(Move::DropKick, THUG.armour));
        assert!(staggers(Move::Punch, FODDER.armour));
    }

    #[test]
    fn an_unstaggered_thug_keeps_winding_up() {
        let mut b = MeleeBrain::new(THUG);
        for _ in 0..20 {
            b.decide(40.0, DT, false, false);
        }
        // A punch lands but doesn't stagger: the game passes stunned = false, so the telegraph carries on.
        let before = b.telegraph();
        b.decide(40.0, DT, false, false);
        assert!(b.telegraph() > before);
    }
}
