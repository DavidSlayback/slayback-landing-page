//! Scoring: kills times a streak multiplier for fast kills, plus a time-left bonus on each escape; and the best-score
//! record, kept locally now and serialisable for a global board later (decision Q15).

/// Points for a kill before the multiplier.
pub const KILL: u64 = 100;
/// A kill within this many seconds of the last one extends the streak.
pub const STREAK_WINDOW: f32 = 2.5;
/// The multiplier tops out here.
pub const MAX_MULTIPLIER: f32 = 4.0;
/// Points per second left on the clock when escaping, per box number (later boxes pay more).
pub const TIME_BONUS: f32 = 10.0;

/// A run's score so far.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Score {
    pub points: u64,
    /// Kills in the current streak.
    pub streak: u32,
    /// Seconds left before the streak lapses.
    window: f32,
}

/// The multiplier for the `n`th kill of a streak: x1, x1.5, x2, ... up to [`MAX_MULTIPLIER`].
pub fn multiplier(streak: u32) -> f32 {
    (1.0 + 0.5 * streak.saturating_sub(1) as f32).min(MAX_MULTIPLIER)
}

/// The escape bonus for `time_left` seconds on box `number`.
pub fn time_bonus(time_left: f32, number: usize) -> u64 {
    (time_left.max(0.0) * TIME_BONUS * number as f32).round() as u64
}

impl Score {
    /// Scores a kill; returns the points it earned.
    pub fn kill(&mut self) -> u64 {
        self.streak = if self.window > 0.0 { self.streak + 1 } else { 1 };
        self.window = STREAK_WINDOW;
        let points = (KILL as f32 * multiplier(self.streak)).round() as u64;
        self.points += points;
        points
    }

    pub fn tick(&mut self, dt: f32) {
        self.window = (self.window - dt).max(0.0);
        if self.window == 0.0 {
            self.streak = 0;
        }
    }

    pub fn escape(&mut self, time_left: f32, number: usize) -> u64 {
        let bonus = time_bonus(time_left, number);
        self.points += bonus;
        bonus
    }

    pub fn multiplier(&self) -> f32 {
        multiplier(self.streak.max(1))
    }
}

/// The best run on record: its score, how far it got, and when (an ISO date, `YYYY-MM-DD`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Best {
    pub score: u64,
    pub reached: usize,
    pub date: String,
}

impl Best {
    /// Whether a finished run with `score` beats this record.
    pub fn beaten_by(&self, score: u64) -> bool {
        score > self.score
    }

    /// A compact, versioned text form: `v1;score;box;date`.
    pub fn to_record(&self) -> String {
        format!("v1;{};{};{}", self.score, self.reached, self.date)
    }

    pub fn from_record(s: &str) -> Option<Best> {
        let mut it = s.trim().split(';');
        (it.next()? == "v1").then_some(())?;
        let best = Best { score: it.next()?.parse().ok()?, reached: it.next()?.parse().ok()?, date: it.next()?.to_owned() };
        it.next().is_none().then_some(best)
    }
}

/// The calendar date (`YYYY-MM-DD`, UTC) of a Unix timestamp in seconds.
pub fn iso_date(unix_seconds: u64) -> String {
    // Howard Hinnant's days-to-civil algorithm.
    let z = (unix_seconds / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_kills_build_a_streak_multiplier_that_lapses() {
        let mut s = Score::default();
        assert_eq!(s.kill(), 100);
        s.tick(1.0);
        assert_eq!(s.kill(), 150);
        s.tick(1.0);
        assert_eq!(s.kill(), 200);
        s.tick(STREAK_WINDOW + 0.1);
        assert_eq!(s.streak, 0, "the streak lapsed");
        assert_eq!(s.kill(), 100);
        assert_eq!(s.points, 550);
    }

    #[test]
    fn the_multiplier_tops_out() {
        assert_eq!(multiplier(1), 1.0);
        assert_eq!(multiplier(100), MAX_MULTIPLIER);
    }

    #[test]
    fn escaping_with_time_left_pays_more_on_later_boxes() {
        assert_eq!(time_bonus(30.0, 1), 300);
        assert_eq!(time_bonus(30.0, 3), 900);
        assert_eq!(time_bonus(-5.0, 2), 0);
        let mut s = Score::default();
        s.escape(12.4, 1);
        assert_eq!(s.points, 124);
    }

    #[test]
    fn the_best_score_round_trips_and_rejects_garbage() {
        let best = Best { score: 4200, reached: 4, date: "2026-10-13".into() };
        assert_eq!(Best::from_record(&best.to_record()), Some(best.clone()));
        for bad in ["", "v2;1;1;2026-01-01", "v1;x;1;d", "v1;1;1", "v1;1;1;d;extra"] {
            assert_eq!(Best::from_record(bad), None, "{bad:?}");
        }
        assert!(best.beaten_by(4201) && !best.beaten_by(4200));
    }

    #[test]
    fn dates_come_out_in_iso_form() {
        assert_eq!(iso_date(0), "1970-01-01");
        assert_eq!(iso_date(951_782_400), "2000-02-29");
        assert_eq!(iso_date(1_791_849_600), "2026-10-13");
    }
}
