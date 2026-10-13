//! Automatic quality scaling (D12): hold 60 fps on integrated graphics by shedding the costliest effects first, in the
//! order the B2 spike measured them (bloom, then MSAA, then shadows), when frames run long for a while.
//!
//! It only ever steps down: stepping back up on a good stretch would flicker between levels on a borderline machine.

/// Effects still on, from most to least.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// No bloom, no MSAA, no shadows.
    Minimal,
    /// No bloom, no MSAA.
    NoMsaa,
    /// No bloom.
    NoBloom,
    Full,
}

impl Level {
    fn lower(self) -> Option<Level> {
        match self {
            Level::Full => Some(Level::NoBloom),
            Level::NoBloom => Some(Level::NoMsaa),
            Level::NoMsaa => Some(Level::Minimal),
            Level::Minimal => None,
        }
    }

    pub fn bloom(self) -> bool {
        self == Level::Full
    }

    pub fn msaa(self) -> bool {
        self >= Level::NoBloom
    }

    pub fn shadows(self) -> bool {
        self >= Level::NoMsaa
    }
}

/// A frame budget a little over 60 fps's 16.7 ms, so vsync jitter alone never trips it.
pub const BUDGET_MS: f32 = 18.0;
/// How long frames must run over budget before a step down, seconds.
pub const SUSTAIN: f32 = 1.5;
/// The smoothing weight of each new frame time.
const SMOOTHING: f32 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scaler {
    pub level: Level,
    /// Smoothed frame time, ms.
    pub average_ms: f32,
    over: f32,
}

impl Default for Scaler {
    fn default() -> Self {
        Self { level: Level::Full, average_ms: 1000.0 / 60.0, over: 0.0 }
    }
}

impl Scaler {
    /// Feeds one frame's duration (seconds). Returns the new level when it steps down.
    pub fn observe(&mut self, frame: f32) -> Option<Level> {
        // Ignore hitches longer than a quarter second (tab switches, loading): they say nothing about steady load.
        if frame > 0.25 {
            return None;
        }
        self.average_ms += SMOOTHING * (frame * 1000.0 - self.average_ms);
        if self.average_ms <= BUDGET_MS {
            self.over = 0.0;
            return None;
        }
        self.over += frame;
        if self.over < SUSTAIN {
            return None;
        }
        self.over = 0.0;
        let next = self.level.lower()?;
        self.level = next;
        Some(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(s: &mut Scaler, frame: f32, seconds: f32) -> Vec<Level> {
        (0..(seconds / frame) as usize).filter_map(|_| s.observe(frame)).collect()
    }

    #[test]
    fn a_machine_that_holds_60_fps_keeps_everything() {
        let mut s = Scaler::default();
        assert!(run(&mut s, 1.0 / 60.0, 10.0).is_empty());
        assert_eq!(s.level, Level::Full);
    }

    #[test]
    fn sustained_slow_frames_shed_bloom_then_msaa_then_shadows() {
        let mut s = Scaler::default();
        let steps = run(&mut s, 1.0 / 30.0, 10.0);
        assert_eq!(steps, vec![Level::NoBloom, Level::NoMsaa, Level::Minimal]);
        assert!(!s.level.bloom() && !s.level.msaa() && !s.level.shadows());
    }

    #[test]
    fn a_brief_slowdown_or_a_single_hitch_changes_nothing() {
        let mut s = Scaler::default();
        run(&mut s, 1.0 / 60.0, 2.0);
        assert!(run(&mut s, 1.0 / 30.0, 0.5).is_empty(), "half a second of slow frames isn't enough");
        run(&mut s, 1.0 / 60.0, 2.0);
        assert_eq!(s.observe(0.5), None, "a long hitch is ignored");
        assert_eq!(s.level, Level::Full);
    }

    #[test]
    fn it_never_steps_back_up() {
        let mut s = Scaler::default();
        run(&mut s, 1.0 / 30.0, 2.0);
        assert_eq!(s.level, Level::NoBloom);
        run(&mut s, 1.0 / 120.0, 10.0);
        assert_eq!(s.level, Level::NoBloom);
    }

    #[test]
    fn the_levels_shed_in_the_measured_cost_order() {
        assert!(Level::Full.bloom() && Level::Full.msaa() && Level::Full.shadows());
        assert!(!Level::NoBloom.bloom() && Level::NoBloom.msaa());
        assert!(!Level::NoMsaa.msaa() && Level::NoMsaa.shadows());
    }
}
