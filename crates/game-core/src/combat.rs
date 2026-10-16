//! Attacks, hits, health and hit-stop: the rules of a single exchange of blows. Lengths are metres and speeds m/s,
//! with +y up.
//!
//! A [`Blow`] is anything that lands on a fighter: a swing ([`Blow::swing`]), a thrown thing (`grab::blow`) or a bomb
//! blast (`bomb::blow`). [`land`] is the one rule for what any of them does to the fighter it reaches.

use crate::aim::launch;
use crate::enemy::{Armour, staggers};
use crate::impact::Feel;

/// An attack the player or an enemy can throw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Punch,
    Kick,
    /// Attacking during a dash: the dash becomes a flying kick.
    DropKick,
    /// The thug's overhead blow. Its telegraph is the thug's long wind-up (see `enemy::THUG`).
    Smash,
    /// The ninja's quick cut.
    Slash,
    /// The fodder's punch: the player's before Q70 lengthened it.
    Jab,
}

/// Timing (seconds), reach and effect of a move.
#[derive(Clone, Copy, Debug)]
pub struct MoveSpec {
    pub startup: f32,
    pub active: f32,
    pub recovery: f32,
    /// How far the blow reaches past the attacker's edge, metres.
    pub reach: f32,
    /// Where the blow lands, metres above the attacker's centre (punches high, kicks low).
    pub offset_y: f32,
    pub damage: f32,
    /// Launch given to the target, away and up, m/s: the side-view game's, which `aim::launch` scales for the floor.
    pub knockback: (f32, f32),
    /// How long the target loses control.
    pub hitstun: f32,
    /// Whole-game freeze on impact, sells the weight of the hit.
    pub hitstop: f32,
}

impl Move {
    pub fn spec(self) -> MoveSpec {
        match self {
            // Fast and close: a combo starter.
            Move::Punch => MoveSpec {
                startup: 0.06,
                active: 0.08,
                recovery: 0.14,
                // Q70, Q73: +70% over the original for more range (the fodder's `Jab` keeps the old reach).
                reach: 1.8,
                offset_y: 0.3125,
                damage: 8.0,
                knockback: (16.25, 8.125),
                hitstun: 0.25,
                hitstop: 0.05,
            },
            Move::Jab => MoveSpec { reach: 1.0625, ..Move::Punch.spec() },
            // Slow and far: launches bodies across the box.
            Move::Kick => MoveSpec {
                startup: 0.12,
                active: 0.10,
                recovery: 0.24,
                reach: 2.4, // Q70: +35% for more range
                offset_y: -0.4375,
                damage: 14.0,
                knockback: (28.125, 13.125),
                hitstun: 0.40,
                hitstop: 0.09,
            },
            // Rides the dash: out at once, lasts about as long as the dash, then a landing recovery.
            Move::DropKick => MoveSpec {
                startup: 0.0,
                active: 0.14,
                recovery: 0.22,
                reach: 1.55, // Q70: +35% for more range
                offset_y: -0.1875,
                damage: 12.0,
                knockback: (31.25, 11.25),
                hitstun: 0.45,
                hitstop: 0.08,
            },
            // Comes down from overhead in front of the thug: wide, heavy, and slow to recover from.
            Move::Smash => MoveSpec {
                startup: 0.1,
                active: 0.12,
                recovery: 0.4,
                reach: 1.375,
                offset_y: 0.5,
                damage: 22.0,
                knockback: (23.4375, 15.0),
                hitstun: 0.5,
                hitstop: 0.1,
            },
            Move::Slash => MoveSpec {
                startup: 0.05,
                active: 0.08,
                recovery: 0.2,
                reach: 0.9375,
                offset_y: 0.1875,
                damage: 7.0,
                knockback: (14.0625, 6.25),
                hitstun: 0.25,
                hitstop: 0.05,
            },
        }
    }

    /// The move an attack button throws: any attack during a dash is a drop-kick.
    pub fn for_button(button: Move, dashing: bool) -> Move {
        if dashing { Move::DropKick } else { button }
    }

    /// The player's moves (enemy moves are tuned for readability, not crispness).
    pub const PLAYER: [Move; 3] = [Move::Punch, Move::Kick, Move::DropKick];
}

/// Where a swing is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Startup,
    Active,
    Recovery,
}

/// One attack in progress.
#[derive(Clone, Copy, Debug)]
pub struct Swing {
    pub mv: Move,
    pub elapsed: f32,
    /// `elapsed` before the last `advance`: the frame covered `before..=elapsed`.
    before: f32,
}

impl Swing {
    pub fn new(mv: Move) -> Swing {
        Swing { mv, elapsed: 0.0, before: 0.0 }
    }

    /// Advances time; returns false once the swing has fully recovered.
    pub fn advance(&mut self, dt: f32) -> bool {
        self.before = self.elapsed;
        self.elapsed += dt;
        self.phase().is_some()
    }

    /// Whether the active window overlapped the last frame, however long that frame was: a slow frame must not let
    /// a blow skip straight from startup to recovery and whiff.
    pub fn struck_this_frame(&self) -> bool {
        let s = self.mv.spec();
        self.before < s.startup + s.active && self.elapsed >= s.startup
    }

    pub fn phase(&self) -> Option<Phase> {
        let s = self.mv.spec();
        let t = self.elapsed;
        if t < s.startup {
            Some(Phase::Startup)
        } else if t < s.startup + s.active {
            Some(Phase::Active)
        } else if t < s.startup + s.active + s.recovery {
            Some(Phase::Recovery)
        } else {
            None
        }
    }

    pub fn is_active(&self) -> bool {
        self.phase() == Some(Phase::Active)
    }
}

/// Freeze for a finishing blow: longer than any move's own, so a kill always lands as a beat.
pub const FINISHER_HITSTOP: f32 = 0.16;

/// A blow about to land: what it does to whatever fighter it reaches. Where it came from (a swing, a throw, a
/// blast) only decides these numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blow {
    pub damage: f32,
    /// Hit-stun it leaves, seconds (a fighter keeps the longer of this and any stun it already has).
    pub stun: f32,
    /// The launch it gives: across the floor (a plane vector) and up, m/s.
    pub launch: ([f32; 2], f32),
    /// The hit-stop it triggers, seconds, raised to [`FINISHER_HITSTOP`] if it defeats; `None` for a blow whose
    /// source freezes the game once for all it hits (a blast).
    pub freeze: Option<f32>,
    /// Armour that shrugs it off: the damage lands, but no stun or launch, so a telegraph keeps going.
    pub shrugged_by: Option<Armour>,
    /// Whether it reaches a fighter in the player's hands (only a blast does; even then it isn't launched).
    pub reaches_held: bool,
    /// How it reads on screen and in the speakers, or `None` if it raises no hit of its own (a blast's victims:
    /// the blast is the spectacle).
    pub feel: Option<Feel>,
}

impl Blow {
    /// A swing of `mv` landing along `yaw` (the direction it pushes).
    pub fn swing(mv: Move, yaw: f32) -> Blow {
        let s = mv.spec();
        Blow {
            damage: s.damage,
            stun: s.hitstun,
            launch: launch(mv, yaw),
            freeze: Some(s.hitstop),
            shrugged_by: [Armour::Punches].into_iter().find(|&a| !staggers(mv, a)),
            reaches_held: false,
            feel: Some(Feel::of(mv)),
        }
    }
}

/// What a blow did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Landed {
    pub outcome: HitOutcome,
    /// The launch to give the fighter, if it is launched at all (not when armour shrugs the blow off, nor in the
    /// player's hands).
    pub launch: Option<([f32; 2], f32)>,
    /// The hit-stop to trigger, seconds.
    pub freeze: Option<f32>,
}

impl Landed {
    pub fn finisher(&self) -> bool {
        self.outcome == HitOutcome::Defeated
    }
}

/// `blow` landing on a fighter with `health`, wearing `armour`, `held` or not in the player's hands. `None` if it
/// can't land: the fighter is already down, or held and the blow doesn't reach held things.
pub fn land(blow: &Blow, health: &mut Health, armour: Armour, held: bool) -> Option<Landed> {
    if health.defeated() || (held && !blow.reaches_held) {
        return None;
    }
    let shrugged = blow.shrugged_by == Some(armour);
    if !shrugged {
        health.stun = health.stun.max(blow.stun);
    }
    let outcome = health.hurt(blow.damage);
    let finished = outcome == HitOutcome::Defeated;
    Some(Landed {
        outcome,
        launch: (!shrugged && !held).then_some(blow.launch),
        freeze: blow.freeze.map(|f| if finished { f.max(FINISHER_HITSTOP) } else { f }),
    })
}

/// What a hit did to its target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitOutcome {
    Hurt,
    Defeated,
}

/// Hit points plus hit-stun.
#[derive(Clone, Copy, Debug)]
pub struct Health {
    pub hp: f32,
    pub max: f32,
    pub stun: f32,
}

impl Health {
    pub fn new(max: f32) -> Health {
        Health { hp: max, max, stun: 0.0 }
    }

    /// Damage with no stun (a timeout penalty, a thrown body's own knock); a blow lands through [`land`].
    pub fn hurt(&mut self, amount: f32) -> HitOutcome {
        self.hp -= amount;
        if self.defeated() { HitOutcome::Defeated } else { HitOutcome::Hurt }
    }

    pub fn tick(&mut self, dt: f32) {
        self.stun = (self.stun - dt).max(0.0);
    }

    pub fn stunned(&self) -> bool {
        self.stun > 0.0
    }

    pub fn defeated(&self) -> bool {
        self.hp <= 0.0
    }
}

/// A global freeze triggered by impacts; overlapping triggers keep the longest.
#[derive(Clone, Copy, Debug, Default)]
pub struct HitStop {
    pub remaining: f32,
}

impl HitStop {
    pub fn trigger(&mut self, duration: f32) {
        self.remaining = self.remaining.max(duration);
    }

    /// Consumes real time; returns true while the game should stay frozen.
    pub fn tick(&mut self, real_dt: f32) -> bool {
        self.remaining = (self.remaining - real_dt).max(0.0);
        self.remaining > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Move; 6] = [Move::Punch, Move::Kick, Move::DropKick, Move::Smash, Move::Slash, Move::Jab];

    #[test]
    fn a_finishing_blow_freezes_longer_than_any_ordinary_hit() {
        for mv in ALL {
            let blow = Blow::swing(mv, 0.0);
            let hurt = land(&blow, &mut Health::new(100.0), Armour::None, false).unwrap();
            assert_eq!(hurt.freeze, Some(mv.spec().hitstop));
            assert!(FINISHER_HITSTOP > mv.spec().hitstop, "{mv:?} must not out-freeze a kill");
            let killed = land(&blow, &mut Health::new(1.0), Armour::None, false).unwrap();
            assert!(killed.finisher());
            assert_eq!(killed.freeze, Some(FINISHER_HITSTOP));
        }
    }

    #[test]
    fn a_swing_lands_its_damage_stun_and_launch() {
        for mv in ALL {
            let s = mv.spec();
            let mut h = Health::new(100.0);
            let landed = land(&Blow::swing(mv, 0.0), &mut h, Armour::None, false).unwrap();
            assert_eq!(h.hp, 100.0 - s.damage, "{mv:?}");
            assert_eq!(h.stun, s.hitstun, "{mv:?}");
            assert_eq!(landed.launch, Some(launch(mv, 0.0)), "{mv:?}");
        }
    }

    #[test]
    fn armour_shrugs_off_a_punch_but_not_a_kick() {
        let mut h = Health::new(100.0);
        let punch = land(&Blow::swing(Move::Punch, 0.0), &mut h, Armour::Punches, false).unwrap();
        assert_eq!((h.hp, h.stun, punch.launch), (100.0 - Move::Punch.spec().damage, 0.0, None), "hurt, not staggered");
        let kick = land(&Blow::swing(Move::Kick, 0.0), &mut h, Armour::Punches, false).unwrap();
        assert!(h.stunned() && kick.launch.is_some(), "a kick staggers it");
    }

    #[test]
    fn only_a_blow_that_reaches_held_things_lands_in_the_players_hands_and_it_never_launches_them() {
        let mut h = Health::new(100.0);
        assert_eq!(land(&Blow::swing(Move::Kick, 0.0), &mut h, Armour::None, true), None);
        assert_eq!(h.hp, 100.0, "untouched");
        let blast = Blow { reaches_held: true, ..Blow::swing(Move::Kick, 0.0) };
        let landed = land(&blast, &mut h, Armour::None, true).unwrap();
        assert!(h.hp < 100.0 && h.stunned() && landed.launch.is_none());
    }

    #[test]
    fn nothing_lands_on_the_defeated() {
        let mut h = Health::new(1.0);
        assert!(land(&Blow::swing(Move::Punch, 0.0), &mut h, Armour::None, false).unwrap().finisher());
        assert_eq!(land(&Blow::swing(Move::Punch, 0.0), &mut h, Armour::None, false), None);
    }

    #[test]
    fn a_swing_runs_startup_then_active_then_recovery_then_ends() {
        let s = Move::Kick.spec();
        let mut swing = Swing::new(Move::Kick);
        assert_eq!(swing.phase(), Some(Phase::Startup));
        assert!(swing.advance(s.startup + 0.001));
        assert_eq!(swing.phase(), Some(Phase::Active));
        assert!(swing.advance(s.active));
        assert_eq!(swing.phase(), Some(Phase::Recovery));
        assert!(!swing.advance(s.recovery));
        assert_eq!(swing.phase(), None);
    }

    #[test]
    fn a_slow_frame_that_jumps_over_the_active_window_still_strikes() {
        let p = Move::Punch.spec();
        let mut swing = Swing::new(Move::Punch);
        swing.advance(0.02);
        assert!(!swing.struck_this_frame(), "still in startup");
        // One 250 ms frame: from startup to past the end of the whole swing.
        swing.advance(0.25);
        assert!(swing.phase().is_none() || swing.phase() == Some(Phase::Recovery));
        assert!(swing.struck_this_frame(), "the blow lands anyway");
        swing.advance(0.01);
        assert!(!swing.struck_this_frame(), "but only once");
        assert!(p.startup + p.active < 0.27);
    }

    #[test]
    fn a_kick_is_slower_and_heavier_than_a_punch() {
        let (p, k) = (Move::Punch.spec(), Move::Kick.spec());
        assert!(k.startup > p.startup && k.damage > p.damage && k.knockback.0 > p.knockback.0);
    }

    /// Crisp-feel targets: fast to come out, short freezes that still read as impact, stun outlasting the freeze.
    #[test]
    fn every_move_meets_the_crisp_feel_targets() {
        for mv in Move::PLAYER {
            let s = mv.spec();
            assert!(s.startup <= 0.12, "{mv:?} startup {}", s.startup);
            assert!((0.03..=0.10).contains(&s.hitstop), "{mv:?} hit-stop {}", s.hitstop);
            assert!(s.hitstun > s.hitstop + 0.1, "{mv:?} stun must outlast the freeze");
            assert!(s.startup + s.active + s.recovery <= 0.5, "{mv:?} too long to commit to");
        }
        let p = Move::Punch.spec();
        assert!(p.startup + p.active + p.recovery <= 0.3, "the punch is the quick one");
    }

    #[test]
    fn attacking_during_a_dash_is_a_drop_kick_that_comes_out_at_once() {
        assert_eq!(Move::for_button(Move::Punch, true), Move::DropKick);
        assert_eq!(Move::for_button(Move::Kick, true), Move::DropKick);
        assert_eq!(Move::for_button(Move::Kick, false), Move::Kick);
        let mut swing = Swing::new(Move::DropKick);
        assert!(swing.is_active());
        assert!(swing.advance(0.1) && swing.is_active());
        assert!(Move::DropKick.spec().knockback.0 > Move::Kick.spec().knockback.0, "the hardest launch");
    }

    #[test]
    fn hits_hurt_and_stun_until_hit_points_run_out() {
        let mut h = Health::new(20.0);
        let hit = |h: &mut Health, mv| land(&Blow::swing(mv, 0.0), h, Armour::None, false).unwrap().outcome;
        assert_eq!(hit(&mut h, Move::Punch), HitOutcome::Hurt);
        assert!(h.stunned() && h.hp < 20.0);
        h.tick(1.0);
        assert!(!h.stunned());
        assert_eq!(hit(&mut h, Move::Kick), HitOutcome::Defeated);
        assert!(h.defeated());
    }

    #[test]
    fn hitstop_keeps_the_longest_freeze_and_then_releases() {
        let mut stop = HitStop::default();
        stop.trigger(0.05);
        stop.trigger(0.09);
        stop.trigger(0.02);
        assert!(stop.tick(0.05));
        assert!(stop.tick(0.03));
        assert!(!stop.tick(0.02));
        assert!(!stop.tick(0.016));
    }
}
