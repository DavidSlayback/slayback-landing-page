//! A move that plays out over several rounds (ice sliding, M3): the rules' state after each round and every block's
//! way there (`puzzle_core::Play::key`), so blocks glide instead of snapping.

use puzzle_core::{Round, State};

/// Seconds for the key's own action (a walk, a lift), then for each round after it.
pub const FIRST: f32 = 0.09;
pub const ROUND: f32 = 0.055;

/// A block between two rounds: from cell, to cell, and whether it is ice.
pub type Move = (usize, usize, bool);

pub struct Trace {
    /// The state before the key, after it acts, then after each round.
    pub ticks: Vec<State>,
    /// What moved between each tick and the next.
    pub moves: Vec<Vec<Move>>,
    pub t: f32,
}

impl Trace {
    /// `rounds`: how the key played out, from `Play::key`.
    pub fn new(before: State, rounds: &[Round]) -> Trace {
        let ticks = std::iter::once(before).chain(rounds.iter().map(|r| r.state)).collect();
        let moves = rounds.iter().map(|r| r.moved.iter().map(|m| (m.from, m.to, m.ice)).collect()).collect();
        Trace { ticks, moves, t: 0.0 }
    }

    /// Whether anything moves after the key's own action in a way the hand-made tweens can't draw: a block sliding or
    /// bouncing, or you bouncing. Then the move is drawn from this trace.
    pub fn needed(&self, w: usize) -> bool {
        let block = self.moves.iter().skip(1).flatten().any(|&(from, to, _)| from != to && to != from + w);
        let me = self.ticks.windows(2).skip(1).any(|p| p[1].y < p[0].y || p[1].x != p[0].x);
        block || me
    }

    /// Whether you go up in step `k` without having gone up in the step before: a bounce leaving its spring.
    pub fn launches(&self, k: usize) -> bool {
        let up = |k: usize| self.ticks[k + 1].y < self.ticks[k].y;
        k > 0 && up(k) && !up(k - 1)
    }

    /// Whether you go up in step `k`.
    pub fn rising(&self, k: usize) -> bool {
        self.ticks[k + 1].y < self.ticks[k].y
    }

    pub fn end(&self) -> f32 {
        FIRST + ROUND * (self.ticks.len() as f32 - 2.0).max(0.0)
    }

    /// Which step of the trace `t` falls in, and how far through it (0 to 1).
    pub fn at(&self, t: f32) -> (usize, f32) {
        if t < FIRST {
            return (0, t / FIRST);
        }
        let k = 1 + ((t - FIRST) / ROUND) as usize;
        if k >= self.moves.len() {
            return (self.moves.len() - 1, 1.0);
        }
        (k, (t - FIRST - ROUND * (k - 1) as f32) / ROUND)
    }

    /// The step at which the block that is at `cell` after step `k` stops moving down, for landing sparks: whether it
    /// fell in step `k` and stays put in step `k + 1`.
    pub fn lands(&self, k: usize, w: usize) -> Vec<usize> {
        let fell = |m: &Move| m.1 == m.0 + w;
        self.moves[k]
            .iter()
            .filter(|m| fell(m))
            .filter(|m| self.moves.get(k + 1).is_none_or(|next| next.iter().any(|n| n.0 == m.1 && n.1 == m.1)))
            .map(|m| m.1)
            .collect()
    }
}
