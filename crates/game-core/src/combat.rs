//! Attacks, hits, health and hit-stop: the rules of a single exchange of blows.
//!
//! Coordinates are world units with +y up (Bevy's convention); `facing` is +1 (right) or -1 (left).

/// An attack the player or an enemy can throw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Punch,
    Kick,
}

/// Timing (seconds), reach and effect of a move.
#[derive(Clone, Copy, Debug)]
pub struct MoveSpec {
    pub startup: f32,
    pub active: f32,
    pub recovery: f32,
    /// How far the hitbox extends past the attacker's edge.
    pub reach: f32,
    pub height: f32,
    /// Hitbox centre relative to the attacker's centre (punches high, kicks low).
    pub offset_y: f32,
    pub damage: f32,
    /// Launch velocity given to the target, before applying `facing` to x.
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
                startup: 0.06, active: 0.08, recovery: 0.14, reach: 34.0, height: 24.0, offset_y: 10.0,
                damage: 8.0, knockback: (520.0, 260.0), hitstun: 0.25, hitstop: 0.05,
            },
            // Slow and far: launches bodies across the box.
            Move::Kick => MoveSpec {
                startup: 0.12, active: 0.10, recovery: 0.24, reach: 46.0, height: 22.0, offset_y: -14.0,
                damage: 14.0, knockback: (900.0, 420.0), hitstun: 0.40, hitstop: 0.09,
            },
        }
    }
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
}

impl Swing {
    pub fn new(mv: Move) -> Swing {
        Swing { mv, elapsed: 0.0 }
    }

    /// Advances time; returns false once the swing has fully recovered.
    pub fn advance(&mut self, dt: f32) -> bool {
        self.elapsed += dt;
        self.phase().is_some()
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

/// Axis-aligned box in world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: (f32, f32),
    pub max: (f32, f32),
}

impl Aabb {
    /// A box of half-size `half` around `centre`.
    pub fn around(centre: (f32, f32), half: (f32, f32)) -> Aabb {
        Aabb { min: (centre.0 - half.0, centre.1 - half.1), max: (centre.0 + half.0, centre.1 + half.1) }
    }

    pub fn overlaps(&self, other: &Aabb) -> bool {
        self.min.0 < other.max.0 && self.max.0 > other.min.0 && self.min.1 < other.max.1 && self.max.1 > other.min.1
    }
}

/// The hitbox of `mv` for an attacker centred at `pos` with half-size `half`, facing `facing`.
pub fn hitbox(mv: Move, pos: (f32, f32), half: (f32, f32), facing: f32) -> Aabb {
    let s = mv.spec();
    let edge = pos.0 + facing * half.0;
    let tip = edge + facing * s.reach;
    let y = pos.1 + s.offset_y;
    Aabb { min: (edge.min(tip), y - s.height / 2.0), max: (edge.max(tip), y + s.height / 2.0) }
}

/// Launch velocity for a target hit by `mv` from an attacker facing `facing`.
pub fn knockback(mv: Move, facing: f32) -> (f32, f32) {
    let (x, y) = mv.spec().knockback;
    (x * facing, y)
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

    pub fn take(&mut self, mv: Move) -> HitOutcome {
        let s = mv.spec();
        self.hp -= s.damage;
        self.stun = self.stun.max(s.hitstun);
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

    const HALF: (f32, f32) = (14.0, 28.0);

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
    fn a_kick_is_slower_and_heavier_than_a_punch() {
        let (p, k) = (Move::Punch.spec(), Move::Kick.spec());
        assert!(k.startup > p.startup && k.damage > p.damage && k.knockback.0 > p.knockback.0);
    }

    #[test]
    fn hitboxes_point_the_way_the_attacker_faces() {
        let front = Aabb::around((40.0, 0.0), HALF);
        let behind = Aabb::around((-40.0, 0.0), HALF);
        let right = hitbox(Move::Punch, (0.0, 0.0), HALF, 1.0);
        assert!(right.overlaps(&front) && !right.overlaps(&behind));
        let left = hitbox(Move::Punch, (0.0, 0.0), HALF, -1.0);
        assert!(left.overlaps(&behind) && !left.overlaps(&front));
    }

    #[test]
    fn a_punch_does_not_reach_across_the_arena() {
        let far = Aabb::around((200.0, 0.0), HALF);
        assert!(!hitbox(Move::Punch, (0.0, 0.0), HALF, 1.0).overlaps(&far));
    }

    #[test]
    fn knockback_launches_away_from_the_attacker_and_upward() {
        let (x, y) = knockback(Move::Kick, -1.0);
        assert!(x < 0.0 && y > 0.0);
    }

    #[test]
    fn hits_hurt_and_stun_until_hit_points_run_out() {
        let mut h = Health::new(20.0);
        assert_eq!(h.take(Move::Punch), HitOutcome::Hurt);
        assert!(h.stunned() && h.hp < 20.0);
        h.tick(1.0);
        assert!(!h.stunned());
        assert_eq!(h.take(Move::Kick), HitOutcome::Defeated);
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
