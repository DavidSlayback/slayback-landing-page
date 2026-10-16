//! Enemy decision-making, kept free of physics: given where the player is, what does the enemy want to do?
//!
//! Fodder and thugs fight the same way (walk up, telegraph, strike, cool down) with different numbers, so they share
//! one [`MeleeBrain`] driven by a [`Style`]. Ninjas fight differently (hit and run, and never from in front of the
//! player), so they get a [`NinjaBrain`]; [`Mind`] is either.

use crate::combat::Move;

/// The enemy types (decision Q26; bomb-heads are a modifier on any of them).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Fodder,
    Thug,
    Ninja,
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

/// Everything a kind of enemy is, in one place: its body, toughness, pace and blow. (Its figure's pieces are
/// `debris::solid::figure`; its colours, the palette's.)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    pub kind: Kind,
    /// What it's called (entity names, logs).
    pub name: &'static str,
    /// Its capsule's radius and full height, metres (the height is its figure's).
    pub radius: f32,
    pub height: f32,
    pub max_hp: f32,
    /// Distance to the player, centre to centre across the floor (metres), at which it stops and starts its telegraph.
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
    name: "fodder",
    radius: 0.35,
    height: crate::debris::solid::FODDER_HEIGHT,
    max_hp: 30.0,
    range: 1.625,
    windup: 0.35,
    cooldown: 0.65,
    walk: 0.5,
    attack: Move::Jab,
    armour: Armour::None,
};

/// Slow, tough and wide: a long, obvious wind-up into an overhead smash that punches can't interrupt.
pub const THUG: Style = Style {
    kind: Kind::Thug,
    name: "thug",
    // The 2.5D bodies' widths converted as the fodder's was, so its arms just fit inside.
    radius: 0.55,
    height: crate::debris::solid::THUG_HEIGHT,
    max_hp: 60.0,
    range: 2.0625,
    windup: 0.6,
    cooldown: 1.0,
    walk: 0.3,
    attack: Move::Smash,
    armour: Armour::Punches,
};

/// Fast and fragile: closes in quicker than the player runs, strikes after a brief tell, then backs off.
pub const NINJA: Style = Style {
    kind: Kind::Ninja,
    name: "ninja",
    radius: 0.3,
    height: crate::debris::solid::NINJA_HEIGHT,
    max_hp: 18.0,
    range: 1.375,
    windup: 0.2,
    cooldown: 0.0,
    walk: 1.25,
    attack: Move::Slash,
    armour: Armour::None,
};

impl Kind {
    pub fn style(self) -> Style {
        match self {
            Kind::Fodder => FODDER,
            Kind::Thug => THUG,
            Kind::Ninja => NINJA,
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

    /// `dx` is how far away the player is, metres; a walk takes its sign. `stunned` means a blow staggered it; `busy` means its own swing is under way.
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

/// What a ninja is doing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NinjaPhase {
    /// Closing in on the player's back.
    Approach,
    /// The player faces it at close range: run through the player (fighters pass through each other) to get behind.
    Flank,
    /// The brief tell before the slash, seconds in.
    Windup(f32),
    /// Backing off after a strike, seconds left.
    Retreat(f32),
}

/// Hit and run: approach, strike, retreat, and avoid standing in front of the player.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NinjaBrain {
    pub phase: NinjaPhase,
}

impl Default for NinjaBrain {
    fn default() -> Self {
        Self { phase: NinjaPhase::Approach }
    }
}

impl NinjaBrain {
    /// How close the player may be while facing it before it flanks, metres.
    pub const AVOID: f32 = 5.3125;
    /// Seconds it backs off after a strike.
    pub const RETREAT: f32 = 0.8;

    /// `dx` as in [`MeleeBrain::decide`]; `in_front` means the player is facing it. Otherwise as [`MeleeBrain::decide`].
    pub fn decide(&mut self, dx: f32, in_front: bool, dt: f32, stunned: bool, busy: bool) -> Intent {
        let s = NINJA;
        if stunned || busy {
            if matches!(self.phase, NinjaPhase::Windup(_)) {
                self.phase = NinjaPhase::Approach;
            }
            return Intent::Hold;
        }
        let toward = dx.signum();
        match self.phase {
            NinjaPhase::Retreat(t) => {
                self.phase = if t > dt { NinjaPhase::Retreat(t - dt) } else { NinjaPhase::Approach };
                Intent::Walk(-toward * s.walk)
            }
            NinjaPhase::Windup(t) => {
                if t + dt >= s.windup {
                    self.phase = NinjaPhase::Retreat(Self::RETREAT);
                    Intent::Attack(s.attack)
                } else {
                    self.phase = NinjaPhase::Windup(t + dt);
                    Intent::Hold
                }
            }
            NinjaPhase::Flank | NinjaPhase::Approach => {
                // Faced at close range: dart through to the player's back instead of standing in front.
                if in_front && dx.abs() < Self::AVOID {
                    self.phase = NinjaPhase::Flank;
                    return Intent::Walk(toward * s.walk);
                }
                if dx.abs() > s.range {
                    self.phase = NinjaPhase::Approach;
                    return Intent::Walk(toward * s.walk);
                }
                self.phase = NinjaPhase::Windup(0.0);
                Intent::Hold
            }
        }
    }

    pub fn telegraph(&self) -> f32 {
        match self.phase {
            NinjaPhase::Windup(t) => (t / NINJA.windup).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }
}

/// An enemy's mind: the shared melee brain, or a ninja's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mind {
    Melee(MeleeBrain),
    Ninja(NinjaBrain),
}

impl Mind {
    pub fn new(kind: Kind) -> Self {
        match kind {
            Kind::Ninja => Mind::Ninja(NinjaBrain::default()),
            _ => Mind::Melee(MeleeBrain::new(kind.style())),
        }
    }

    pub fn style(&self) -> Style {
        match self {
            Mind::Melee(b) => b.style,
            Mind::Ninja(_) => NINJA,
        }
    }

    pub fn telegraph(&self) -> f32 {
        match self {
            Mind::Melee(b) => b.telegraph(),
            Mind::Ninja(b) => b.telegraph(),
        }
    }

    pub fn decide(&mut self, dx: f32, in_front: bool, dt: f32, stunned: bool, busy: bool) -> Intent {
        match self {
            Mind::Melee(b) => b.decide(dx, dt, stunned, busy),
            Mind::Ninja(b) => b.decide(dx, in_front, dt, stunned, busy),
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
        assert_eq!(b.decide(9.375, DT, false, false), Intent::Walk(FODDER.walk));
        assert_eq!(b.decide(-9.375, DT, false, false), Intent::Walk(-FODDER.walk));
    }

    #[test]
    fn telegraphs_before_punching_a_nearby_player() {
        let mut b = fodder();
        let frames_before_punch = (0..60).take_while(|_| b.decide(0.9375, DT, false, false) == Intent::Hold).count();
        let expected = (FODDER.windup / DT).ceil() as usize - 1;
        assert!(frames_before_punch.abs_diff(expected) <= 1, "punched after {frames_before_punch} frames");
    }

    #[test]
    fn waits_out_its_cooldown_between_punches() {
        let mut b = fodder();
        let mut punches = 0;
        for _ in 0..(2.0 / DT) as usize {
            if b.decide(0.9375, DT, false, false) == Intent::Attack(Move::Jab) {
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
            b.decide(0.9375, DT, false, false);
        }
        assert_eq!(b.decide(0.9375, DT, true, false), Intent::Hold);
        assert_eq!(b.windup, 0.0, "getting hit should interrupt the telegraph");
        assert_eq!(b.decide(0.9375, DT, false, true), Intent::Hold);
    }

    #[test]
    fn a_thug_is_slower_tougher_and_wider_than_fodder_with_a_longer_telegraph() {
        let (thug, fodder) = (Kind::Thug.style(), Kind::Fodder.style());
        assert!(thug.walk < fodder.walk && thug.max_hp >= 2.0 * fodder.max_hp);
        assert!(thug.windup > fodder.windup);
        let mut b = MeleeBrain::new(THUG);
        let frames = (0..120).take_while(|_| b.decide(1.25, DT, false, false) == Intent::Hold).count();
        assert!(frames as f32 * DT >= 0.55, "a big, readable wind-up: {frames} frames");
        assert_eq!(b.decide(1.25, DT, false, false), Intent::Hold, "then cools down");
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
            b.decide(1.25, DT, false, false);
        }
        // A punch lands but doesn't stagger: the game passes stunned = false, so the telegraph carries on.
        let before = b.telegraph();
        b.decide(1.25, DT, false, false);
        assert!(b.telegraph() > before);
    }

    #[test]
    fn a_ninja_is_fast_and_fragile() {
        let (ninja, fodder) = (Kind::Ninja.style(), Kind::Fodder.style());
        assert!(ninja.walk > 1.0, "outruns the player");
        assert!(ninja.max_hp < fodder.max_hp);
    }

    #[test]
    fn a_ninja_approaches_from_behind_then_strikes_and_retreats() {
        let mut b = NinjaBrain::default();
        // Behind the player (not in front), far away: closes in at full speed.
        assert_eq!(b.decide(6.25, false, DT, false, false), Intent::Walk(NINJA.walk));
        // In range: a brief tell, then the slash.
        let mut struck = false;
        for _ in 0..30 {
            if b.decide(0.9375, false, DT, false, false) == Intent::Attack(Move::Slash) {
                struck = true;
                break;
            }
        }
        assert!(struck, "struck within half a second");
        // Then backs off, away from the player.
        assert_eq!(b.decide(0.9375, false, DT, false, false), Intent::Walk(-NINJA.walk));
        assert!(matches!(b.phase, NinjaPhase::Retreat(_)));
    }

    #[test]
    fn a_ninja_never_stands_in_front_of_the_player() {
        let mut b = NinjaBrain::default();
        // Faced from about 3 m: it runs through toward the player's back rather than waiting there.
        assert_eq!(b.decide(3.125, true, DT, false, false), Intent::Walk(NINJA.walk));
        assert_eq!(b.phase, NinjaPhase::Flank);
        // Once behind (the player no longer faces it), it attacks as normal.
        for _ in 0..30 {
            b.decide(-0.9375, false, DT, false, false);
        }
        assert!(!matches!(b.phase, NinjaPhase::Flank));
    }

    #[test]
    fn minds_dispatch_by_kind() {
        assert!(matches!(Mind::new(Kind::Ninja), Mind::Ninja(_)));
        assert!(matches!(Mind::new(Kind::Thug), Mind::Melee(b) if b.style.kind == Kind::Thug));
        assert_eq!(Mind::new(Kind::Fodder).style().kind, Kind::Fodder);
    }
}
