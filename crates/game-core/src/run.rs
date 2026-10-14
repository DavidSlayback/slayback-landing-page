//! The run: an endless series of boxes. Pile debris under yourself until you reach the rim, escape, and the next
//! box is a little taller, faster and busier. One life; health carries over with a partial refill; each box has a
//! timer, and running out costs health and restarts the clock.
//!
//! The numbers are first guesses aimed at "a first-timer dies around box 3-4 (3-5 min)" (decision Q29) and are meant
//! to be tuned at the playtest; everything that varies by box is in [`box_spec`]. The pace (how many enemies, how
//! fast they come) follows Rumble Box's gameplay video (`docs/research/rumble-box-reference.md`, "Pace, counted"):
//! box 1 holds 3-5 for its first half-minute and 6-8 from about 45 s in, with doors often opening in pairs, and
//! standing still is lethal (Q68).

use crate::enemy::Kind;

/// Health lost when a box's timer runs out (the timer then restarts).
pub const TIMEOUT_PENALTY: f32 = 25.0;
/// Fraction of max health restored on escaping a box.
pub const REFILL: f32 = 0.3;
/// Colour schemes the boxes rotate through (decision Q31: starting from the site's scheme).
pub const SCHEMES: usize = 3;
/// Seconds over which a box's crowd rises from its starting [`BoxSpec::crowd`] to its [`BoxSpec::peak`].
pub const RAMP: f32 = 40.0;

/// Everything that changes from box to box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxSpec {
    /// 1-based box number.
    pub number: usize,
    /// Height of the rim above the floor, world units.
    pub rim: f32,
    /// Seconds on the clock.
    pub timer: f32,
    /// Enemies kept in the box at once when it starts, rising to `peak` over [`RAMP`] seconds ([`BoxSpec::crowd_at`]).
    pub crowd: usize,
    pub peak: usize,
    /// Seconds between calls at the doors while the box holds fewer than its crowd.
    pub interval: f32,
    /// How many doors one call opens at most (each warns, then lets one enemy in).
    pub burst: usize,
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
        // Box 1 ramps 3 -> 7 (Rumble Box: 3-5, then 6-8 about 45 s in); later boxes start busier (box 2: 4-7 from the
        // start) and peak higher, to 10.
        crowd: (3 + 2 * n).min(7),
        peak: (7 + n).min(10),
        interval: (1.0 - 0.1 * n as f32).max(0.6),
        burst: if n < 2 { 2 } else { 3 },
        scheme: n % SCHEMES,
        // Thugs from box 2 (decision Q26 roster; "thugs from box 2" in the difficulty notes).
        thugs: if n == 0 { 0.0 } else { (0.25 + 0.05 * (n - 1) as f32).min(0.4) },
        // Ninjas from box 3.
        ninjas: if n < 2 { 0.0 } else { (0.2 + 0.05 * (n - 2) as f32).min(0.3) },
        // Bomb-heads from box 2, on any kind of enemy.
        bombs: if n == 0 { 0.0 } else { (0.15 + 0.05 * (n - 1) as f32).min(0.3) },
    }
}

impl BoxSpec {
    /// Enemies kept in the box `elapsed` seconds after it began: `crowd`, plus one at each step of the way to `peak`.
    pub fn crowd_at(&self, elapsed: f32) -> usize {
        let t = (elapsed / RAMP).clamp(0.0, 1.0);
        self.crowd + (self.peak.saturating_sub(self.crowd) as f32 * t).floor() as usize
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

/// The 3D game's box 1 rim, metres. A heap of about 20 fodder killed in one spot reaches it (25 reach ~1.9 m; the G3
/// measurements in the README); spread over the floor, the same kills stay under ~1.2 m.
pub const FIRST_RIM_METRES: f32 = 1.5;
/// The 3D game's highest rim, metres: about what 50 kills heaped in a corner reach, well inside the frozen-debris cap.
pub const MAX_RIM_METRES: f32 = 3.0;

/// The 3D game's rim for a box, metres: the 2.5D curve scaled so box 1's is [`FIRST_RIM_METRES`] (about +0.48 m a
/// box), levelling off at [`MAX_RIM_METRES`]. A 3D pile grows with the cube root of its volume, not the square root of
/// an area as in the side view, so the 2.5D rims (4.4 m and up at 32 units per metre) would be out of reach.
pub fn rim_metres(spec: &BoxSpec) -> f32 {
    (FIRST_RIM_METRES * spec.rim / box_spec(1).rim).min(MAX_RIM_METRES)
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

/// The run's state between frames: which box, the clock, and how long the box has been going (which a timeout
/// doesn't reset, so the crowd keeps its ramp). The player's health lives on the player.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Run {
    pub spec: BoxSpec,
    pub time_left: f32,
    pub elapsed: f32,
}

impl Default for Run {
    fn default() -> Self {
        Self::box_number(1)
    }
}

impl Run {
    fn box_number(number: usize) -> Self {
        let spec = box_spec(number);
        Self { spec, time_left: spec.timer, elapsed: 0.0 }
    }

    /// Enemies the box keeps now.
    pub fn crowd(&self) -> usize {
        self.spec.crowd_at(self.elapsed)
    }

    /// Advances the clock. On timeout the clock restarts and the caller applies [`TIMEOUT_PENALTY`].
    pub fn tick(&mut self, dt: f32) -> Tick {
        self.elapsed += dt;
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
            assert!(b.interval < a.interval, "box {} calls at the doors more often", n + 1);
            assert!(b.crowd >= a.crowd && b.burst >= a.burst, "box {} no quieter", n + 1);
            assert_eq!(b.scheme, (a.scheme + 1) % SCHEMES);
        }
        for n in 1..4 {
            assert!(box_spec(n + 1).peak > box_spec(n).peak, "box {} more enemies at its peak", n + 1);
        }
        assert_eq!(box_spec(1).scheme, 0, "the first box uses the site's scheme");
    }

    #[test]
    fn box_one_ramps_to_rumble_boxs_crowd_and_later_boxes_start_busy() {
        let first = box_spec(1);
        assert_eq!(first.crowd_at(0.0), 3, "3-5 at first in Rumble Box");
        assert_eq!(first.crowd_at(RAMP / 2.0), 5);
        assert_eq!(first.crowd_at(RAMP), 7, "6-8 about 45 s in");
        assert_eq!(first.crowd_at(10.0 * RAMP), 7, "and no more");
        assert!(first.burst >= 2, "doors open in pairs");
        assert!(first.interval <= 1.0, "never a long lull at the doors");
        assert!(box_spec(2).crowd_at(0.0) >= 5, "box 2 starts busy (4-7 in Rumble Box)");
    }

    #[test]
    fn a_timeout_restarts_the_clock_but_not_the_crowds_ramp() {
        let mut run = Run::default();
        assert_eq!(run.crowd(), run.spec.crowd);
        while run.tick(1.0) == Tick::Running {}
        assert_eq!(run.crowd(), run.spec.peak, "{} s in", run.elapsed);
        run.escape(100.0, 100.0);
        assert_eq!((run.elapsed, run.crowd()), (0.0, box_spec(2).crowd), "a new box starts its ramp again");
    }

    #[test]
    fn a_whole_box_of_kills_at_the_busiest_pace_fits_under_the_frozen_debris_cap() {
        use crate::debris::solid::{MAX_FROZEN, figure};
        // Rumble Box's busiest stretches kill about one enemy a second (reference doc, "Pace, counted"); the largest
        // figure bursts into the most pieces. Past one clock (a timeout restarts it) the oldest frozen pieces go (Q52).
        let most = [Kind::Fodder, Kind::Thug, Kind::Ninja].map(|k| figure(k).len()).into_iter().max().unwrap();
        for n in 1..=12 {
            let pieces = box_spec(n).timer * most as f32;
            assert!(pieces <= MAX_FROZEN as f32, "box {n}: {pieces} pieces in one clock");
        }
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
        assert!(late.rim <= 600.0 && late.timer >= 45.0);
        assert!(late.crowd <= late.peak && late.peak <= 10 && late.interval >= 0.6 && late.burst <= 3);
    }

    #[test]
    fn the_exit_opens_when_the_pile_under_the_player_reaches_the_rim() {
        let rim = box_spec(1).rim;
        assert!(!escaped(rim - 1.0, rim));
        assert!(escaped(rim, rim));
    }

    #[test]
    fn the_3d_rim_starts_at_a_metre_and_a_half_rises_each_box_and_levels_off() {
        assert_eq!(rim_metres(&box_spec(1)), FIRST_RIM_METRES);
        for n in 1..4 {
            assert!(rim_metres(&box_spec(n + 1)) > rim_metres(&box_spec(n)), "box {} taller", n + 1);
        }
        assert!((rim_metres(&box_spec(2)) - rim_metres(&box_spec(1)) - 0.48).abs() < 0.01);
        assert_eq!(rim_metres(&box_spec(40)), MAX_RIM_METRES);
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
