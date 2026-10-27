//! A move that plays out over several rounds (ice sliding, M3): the rules' state after each round
//! (`puzzle_core::step_traced`), and which block went where between rounds, so blocks glide instead of snapping.

use puzzle_core::{Bits, State};

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
    /// `rounds`: each state from `step_traced` with the cells of the blocks that fell a row to reach it.
    pub fn new(before: State, rounds: Vec<(State, Bits)>, w: usize, h: usize) -> Trace {
        let mut ticks = vec![before];
        let mut moves = vec![];
        for (s, fell) in rounds {
            moves.push(between(ticks.last().unwrap(), &s, fell, w, h));
            ticks.push(s);
        }
        Trace { ticks, moves, t: 0.0 }
    }

    /// Whether any block slides after the key's own action: then the move needs this trace to be drawn right.
    pub fn slides(&self, w: usize) -> bool {
        self.moves.iter().skip(1).flatten().any(|&(from, to, _)| from / w == to / w && from != to)
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

/// The blocks of `s` by cell, the carried one in the cell above you: (cell, ice).
fn blocks(s: &State, w: usize, h: usize) -> Vec<(usize, bool)> {
    let mut out: Vec<(usize, bool)> = (0..w * h).filter(|&i| s.blocks.get(i)).map(|i| (i, s.ice.get(i))).collect();
    if s.carrying {
        out.push((s.y as usize * w + s.x as usize - w, s.carrying_ice));
    }
    out
}

/// Matches the blocks of `a` to those of `b`: those in `fell` came from the cell above, those in the same cell stayed,
/// and the rest moved to a neighbouring cell of their own kind (a slide, a lift, a set-down, a climb).
fn between(a: &State, b: &State, fell: Bits, w: usize, h: usize) -> Vec<Move> {
    let (from, to) = (blocks(a, w, h), blocks(b, w, h));
    let mut sources: Vec<(usize, bool)> = vec![];
    let mut targets: Vec<(usize, bool)> = to.iter().copied().filter(|&(c, _)| !fell.get(c)).collect();
    let mut out: Vec<Move> = to.iter().filter(|&&(c, _)| fell.get(c)).map(|&(c, ice)| (c - w, c, ice)).collect();
    for &(c, ice) in &from {
        if out.iter().any(|m| m.0 == c) {
            continue;
        }
        match targets.iter().position(|&t| t == (c, ice)) {
            Some(k) => {
                targets.remove(k);
                out.push((c, c, ice));
            }
            None => sources.push((c, ice)),
        }
    }
    let (w, ci) = (w as i32, |c: usize| c as i32);
    for (c, ice) in sources {
        let near = [-1, 1, -w - 1, -w + 1, -w, w - 1, w + 1, w].map(|d| ci(c) + d);
        let k = near
            .iter()
            .find_map(|&n| targets.iter().position(|&t| ci(t.0) == n && t.1 == ice))
            .or_else(|| targets.iter().position(|t| t.1 == ice));
        if let Some(k) = k {
            out.push((c, targets.remove(k).0, ice));
        }
    }
    out.extend(targets.into_iter().map(|(c, ice)| (c, c, ice)));
    out
}
