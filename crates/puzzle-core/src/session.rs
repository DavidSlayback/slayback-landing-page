//! A benchmark session on one level (P10): what an agent may do and when it is over. Moves count only while they are
//! on the board, so undo and restart are free; the board may hold at most three times the level's shortest solution,
//! and filling it unsolved ends the session. Every action is kept, so a session replays from its actions alone. The
//! call budget (P21) belongs to the server, not here.

use crate::{Applied, Level, Play};

/// The move budget: this many times the shortest solution (P10).
pub const BUDGET: usize = 3;

/// One thing an agent did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Act {
    /// Played a move string.
    Keys(String),
    /// Took back up to this many moves.
    Undo(usize),
    Restart,
}

/// What a move string did, within the budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Moved {
    /// Keys played, as [`Play::apply`] reports them; `stop` is the refused key, if one was.
    pub applied: Applied,
    /// The board filled before the keys ran out: the keys past it were not played, and the session is over.
    pub out_of_moves: bool,
}

/// How a session ended, or stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub solved: bool,
    /// Moves on the board.
    pub moves: usize,
}

#[derive(Clone, Debug)]
pub struct Session {
    play: Play,
    budget: usize,
    acts: Vec<Act>,
}

impl Session {
    /// A session on `level`, whose shortest solution is `shortest` moves long.
    pub fn new(level: Level, shortest: usize) -> Session {
        Session { play: Play::new(level), budget: BUDGET * shortest, acts: vec![] }
    }

    /// Rebuilds a session from its actions.
    pub fn replay(level: Level, shortest: usize, acts: &[Act]) -> Session {
        let mut s = Session::new(level, shortest);
        for act in acts {
            match act {
                Act::Keys(keys) => {
                    s.keys(keys);
                }
                Act::Undo(n) => {
                    s.undo(*n);
                }
                Act::Restart => s.restart(),
            }
        }
        s
    }

    pub fn play(&self) -> &Play {
        &self.play
    }

    /// Moves the board may hold.
    pub fn budget(&self) -> usize {
        self.budget
    }

    pub fn acts(&self) -> &[Act] {
        &self.acts
    }

    /// Solved, or the board full: nothing more can be done.
    pub fn over(&self) -> bool {
        self.play.solved() || self.play.moves() >= self.budget
    }

    pub fn outcome(&self) -> Outcome {
        Outcome { solved: self.play.solved(), moves: self.play.moves() }
    }

    /// Plays `keys` until they run out, the level is solved, a key is refused or the board is full. Over, it plays
    /// nothing.
    pub fn keys(&mut self, keys: &str) -> Moved {
        let room = self.budget.saturating_sub(self.play.moves());
        let fits: String = keys.chars().take(if self.over() { 0 } else { room }).collect();
        self.acts.push(Act::Keys(keys.to_owned()));
        let applied = self.play.apply(&fits);
        let cut = fits.chars().count() < keys.chars().count();
        Moved { applied, out_of_moves: cut && applied.stop.is_none() && !applied.solved }
    }

    /// Takes back up to `n` moves (none once over); how many.
    pub fn undo(&mut self, n: usize) -> usize {
        self.acts.push(Act::Undo(n));
        if self.over() { 0 } else { self.play.undo(n) }
    }

    /// Back to the start (not once over).
    pub fn restart(&mut self) {
        self.acts.push(Act::Restart);
        if !self.over() {
            self.play.restart();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two moves right to the exit: `>.E` on a floor.
    fn level() -> Level {
        Level::parse("#....#\n#>..E#\n######").unwrap()
    }

    #[test]
    fn undo_and_restart_are_free_and_only_the_board_counts() {
        let mut s = Session::new(level(), 3);
        assert_eq!(s.budget(), 9);
        assert_eq!(s.keys("RL").applied.played, 2);
        assert_eq!(s.undo(1), 1);
        s.restart();
        let m = s.keys("RRR");
        assert!(m.applied.solved && !m.out_of_moves);
        assert_eq!(s.outcome(), Outcome { solved: true, moves: 3 });
        assert!(s.over());
        assert_eq!(s.keys("L").applied.played, 0, "nothing plays once solved");
    }

    #[test]
    fn filling_the_board_unsolved_ends_the_session() {
        let mut s = Session::new(level(), 1);
        // Right, then two turns fill the board one short of the exit.
        let m = s.keys("RLRRR");
        assert_eq!(m.applied.played, 3);
        assert!(m.out_of_moves && s.over() && !s.outcome().solved);
        assert_eq!(s.undo(3), 0, "no taking it back once over");
    }

    #[test]
    fn a_session_replays_from_its_actions() {
        let mut s = Session::new(level(), 3);
        s.keys("RX");
        s.undo(1);
        s.keys("RRR");
        let again = Session::replay(level(), 3, s.acts());
        assert_eq!(again.outcome(), s.outcome());
        assert_eq!(again.play().state(), s.play().state());
    }
}
