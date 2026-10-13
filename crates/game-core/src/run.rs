//! The run: an endless series of boxes. Pile debris under yourself until you reach the rim, escape, and the next
//! box is a little taller, faster and busier. One life; health carries over with a partial refill; each box has a
//! timer, and running out costs health and restarts the clock.
//!
//! The numbers are first guesses aimed at "a first-timer dies around box 3-4 (3-5 min)" (decision Q29) and are meant
//! to be tuned at the playtest; everything that varies by box is in [`box_spec`].

use crate::enemy::Kind;

/// Health lost when a box's timer runs out (the timer then restarts).
pub const TIMEOUT_PENALTY: f32 = 25.0;
/// Fraction of max health restored on escaping a box.
pub const REFILL: f32 = 0.3;
/// Colour schemes the boxes rotate through (decision Q31: starting from the site's scheme).
pub const SCHEMES: usize = 3;

/// Everything that changes from box to box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxSpec {
    /// 1-based box number.
    pub number: usize,
    /// Height of the rim above the floor, world units.
    pub rim: f32,
    /// Seconds on the clock.
    pub timer: f32,
    /// Enemies kept in the box at once.
    pub crowd: usize,
    /// Offset from the starting colour scheme.
    pub scheme: usize,
    /// Shares of entering enemies that are thugs and ninjas (the rest are fodder).
    pub thugs: f32,
    pub ninjas: f32,
    /// Share of entering enemies (of any kind) that carry a bomb for a head.
    pub bombs: f32,
}

/// Box `number` (1-based) of a run.
pub fn box_spec(number: usize) -> BoxSpec {
    let n = number.max(1) - 1;
    BoxSpec {
        number: number.max(1),
        rim: (140.0 + 45.0 * n as f32).min(600.0),
        timer: (90.0 - 10.0 * n as f32).max(45.0),
        crowd: (3 + n).min(8),
        scheme: n % SCHEMES,
        // Thugs from box 2 (decision Q26 roster; "thugs from box 2" in the difficulty notes).
        thugs: if n == 0 { 0.0 } else { (0.25 + 0.05 * (n - 1) as f32).min(0.4) },
        // Ninjas from box 3.
        ninjas: if n < 2 { 0.0 } else { (0.2 + 0.05 * (n - 2) as f32).min(0.3) },
        // Bomb-heads from box 2, on any kind of enemy.
        bombs: if n == 0 { 0.0 } else { (0.15 + 0.05 * (n - 1) as f32).min(0.3) },
    }
}

/// Which kind of enemy enters next, from a uniform `roll` in 0..1.
pub fn pick_kind(spec: &BoxSpec, roll: f32) -> Kind {
    if roll < spec.thugs {
        Kind::Thug
    } else if roll < spec.thugs + spec.ninjas {
        Kind::Ninja
    } else {
        Kind::Fodder
    }
}

/// The escape test: the pile under the player (how high the player stands above the floor) has reached the rim.
pub fn escaped(standing_height: f32, rim: f32) -> bool {
    standing_height >= rim
}

/// What the clock did this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tick {
    Running,
    /// Time ran out: the penalty was applied and the clock restarted.
    TimedOut,
}

/// The run's state between frames: which box, and the clock. The player's health lives on the player.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Run {
    pub spec: BoxSpec,
    pub time_left: f32,
}

impl Default for Run {
    fn default() -> Self {
        Self::box_number(1)
    }
}

impl Run {
    fn box_number(number: usize) -> Self {
        let spec = box_spec(number);
        Self { spec, time_left: spec.timer }
    }

    /// Advances the clock. On timeout the clock restarts and the caller applies [`TIMEOUT_PENALTY`].
    pub fn tick(&mut self, dt: f32) -> Tick {
        self.time_left -= dt;
        if self.time_left > 0.0 {
            return Tick::Running;
        }
        self.time_left = self.spec.timer;
        Tick::TimedOut
    }

    /// Escaping: on to the next box. Returns the player's health after the refill.
    pub fn escape(&mut self, hp: f32, max: f32) -> f32 {
        *self = Self::box_number(self.spec.number + 1);
        (hp + REFILL * max).min(max)
    }

    /// Death: back to box 1 (the caller restores full health).
    pub fn restart(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_box_is_taller_faster_and_busier_and_rotates_the_scheme() {
        for n in 1..5 {
            let (a, b) = (box_spec(n), box_spec(n + 1));
            assert!(b.rim > a.rim, "box {} taller", n + 1);
            assert!(b.timer < a.timer, "box {} shorter timer", n + 1);
            assert!(b.crowd > a.crowd, "box {} more enemies", n + 1);
            assert_eq!(b.scheme, (a.scheme + 1) % SCHEMES);
        }
        assert_eq!(box_spec(1).scheme, 0, "the first box uses the site's scheme");
    }

    #[test]
    fn thugs_join_from_box_two_and_never_crowd_out_fodder() {
        assert_eq!(pick_kind(&box_spec(1), 0.0), Kind::Fodder, "box 1 is all fodder");
        assert_eq!(pick_kind(&box_spec(2), 0.1), Kind::Thug);
        assert_eq!(pick_kind(&box_spec(2), 0.9), Kind::Fodder);
        assert!(box_spec(40).thugs <= 0.4);
    }

    #[test]
    fn bomb_heads_appear_from_box_two() {
        assert_eq!(box_spec(1).bombs, 0.0);
        assert!(box_spec(2).bombs > 0.0 && box_spec(40).bombs <= 0.3);
    }

    #[test]
    fn ninjas_join_from_box_three_and_fodder_always_remain() {
        assert_eq!(box_spec(2).ninjas, 0.0);
        assert_eq!(pick_kind(&box_spec(3), box_spec(3).thugs + 0.01), Kind::Ninja);
        let late = box_spec(40);
        assert!(late.thugs + late.ninjas < 0.8, "fodder remain part of every crowd");
    }

    #[test]
    fn the_curve_levels_off_instead_of_becoming_impossible() {
        let late = box_spec(40);
        assert!(late.rim <= 600.0 && late.timer >= 45.0 && late.crowd <= 8);
    }

    #[test]
    fn the_exit_opens_when_the_pile_under_the_player_reaches_the_rim() {
        let rim = box_spec(1).rim;
        assert!(!escaped(rim - 1.0, rim));
        assert!(escaped(rim, rim));
    }

    #[test]
    fn running_out_of_time_restarts_the_clock_for_a_penalty() {
        let mut run = Run::default();
        assert_eq!(run.tick(run.spec.timer - 1.0), Tick::Running);
        assert_eq!(run.tick(1.5), Tick::TimedOut);
        assert_eq!(run.time_left, run.spec.timer);
    }

    #[test]
    fn escaping_moves_on_and_refills_part_of_the_health() {
        let mut run = Run::default();
        assert_eq!(run.escape(50.0, 100.0), 80.0);
        assert_eq!(run.spec.number, 2);
        assert_eq!(run.time_left, box_spec(2).timer);
        assert_eq!(run.escape(95.0, 100.0), 100.0, "never above max");
    }

    #[test]
    fn dying_restarts_the_run_at_box_one() {
        let mut run = Run::default();
        run.escape(100.0, 100.0);
        run.restart();
        assert_eq!(run, Run::default());
    }
}
