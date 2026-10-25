//! Rules of *Out of the Box* (`docs/planning/puzzle-spec.md`, "Rules").
//!
//! A level is a grid of walls, loose blocks and one exit. The player walks, turns, climbs one cell and carries one
//! block on their head. Everything is plain data with no dependencies, so the browser, the MCP server, the RL env and
//! the bench all run the same rules.

use std::fmt;

mod generate;
mod solve;
pub use generate::{CAP, Generated, TIERS, Tier, generate};
pub use solve::{Search, solve, solve_within};

/// Widest and tallest grid a level may have; 20 × 12 = 240 cells fit a [`Bits`].
pub const MAX_W: usize = 20;
pub const MAX_H: usize = 12;

/// One set bit per grid cell, indexed `y * w + x`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Bits([u64; 4]);

impl Bits {
    pub fn get(&self, i: usize) -> bool {
        self.0[i / 64] >> (i % 64) & 1 == 1
    }
    pub fn set(&mut self, i: usize, on: bool) {
        let bit = 1u64 << (i % 64);
        if on { self.0[i / 64] |= bit } else { self.0[i / 64] &= !bit }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Facing {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    L,
    R,
    U,
    D,
}

impl Key {
    pub fn from_char(c: char) -> Option<Key> {
        Some(match c {
            'L' => Key::L,
            'R' => Key::R,
            'U' => Key::U,
            'D' => Key::D,
            _ => return None,
        })
    }
}

/// Everything that changes during play. Blocks are identical, so their cells are a set; the carried block is not in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct State {
    pub x: u8,
    pub y: u8,
    pub facing: Facing,
    pub carrying: bool,
    pub blocks: Bits,
}

/// The fixed part of a level, plus where play starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Level {
    pub w: u8,
    pub h: u8,
    pub walls: Bits,
    pub exit: (u8, u8),
    pub start: State,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    TooBig {
        w: usize,
        h: usize,
    },
    Ragged {
        row: usize,
    },
    Unknown {
        ch: char,
        x: usize,
        y: usize,
    },
    Players(usize),
    Exits(usize),
    /// A block or the player with nothing under them; the rules never leave anything there.
    Floating {
        x: usize,
        y: usize,
    },
}

impl Level {
    /// Reads the text form: `#` wall, `B` block, `E` exit, `.` empty, `>`/`<` the player facing that way, `b` a block
    /// resting on the exit. A `B` directly above the player is the block they carry.
    pub fn parse(text: &str) -> Result<Level, ParseError> {
        let rows: Vec<Vec<char>> = text.lines().map(|l| l.chars().collect()).collect();
        let (h, w) = (rows.len(), rows.first().map_or(0, Vec::len));
        if w == 0 {
            return Err(ParseError::Empty);
        }
        if w > MAX_W || h > MAX_H {
            return Err(ParseError::TooBig { w, h });
        }
        let (mut walls, mut blocks) = (Bits::default(), Bits::default());
        let (mut players, mut exits) = (vec![], vec![]);
        for (y, row) in rows.iter().enumerate() {
            if row.len() != w {
                return Err(ParseError::Ragged { row: y });
            }
            for (x, &ch) in row.iter().enumerate() {
                let i = y * w + x;
                match ch {
                    '.' => {}
                    '#' => walls.set(i, true),
                    'B' => blocks.set(i, true),
                    'E' => exits.push((x, y)),
                    'b' => {
                        blocks.set(i, true);
                        exits.push((x, y));
                    }
                    '>' => players.push((x, y, Facing::Right)),
                    '<' => players.push((x, y, Facing::Left)),
                    _ => return Err(ParseError::Unknown { ch, x, y }),
                }
            }
        }
        let &[(px, py, facing)] = players.as_slice() else { return Err(ParseError::Players(players.len())) };
        let &[(ex, ey)] = exits.as_slice() else { return Err(ParseError::Exits(exits.len())) };
        let carrying = py > 0 && blocks.get((py - 1) * w + px);
        if carrying {
            blocks.set((py - 1) * w + px, false);
        }
        let start = State { x: px as u8, y: py as u8, facing, carrying, blocks };
        let level = Level { w: w as u8, h: h as u8, walls, exit: (ex as u8, ey as u8), start };
        let (x, y) = (px as i32, py as i32);
        if !level.solid(&blocks, x, y + 1) {
            return Err(ParseError::Floating { x: px, y: py });
        }
        for i in (0..w * h).filter(|&i| blocks.get(i)) {
            let (x, y) = ((i % w) as i32, (i / w) as i32);
            if !level.solid(&blocks, x, y + 1) {
                return Err(ParseError::Floating { x: x as usize, y: y as usize });
            }
        }
        Ok(level)
    }

    /// The text form of `state` in this level.
    pub fn show(&self, state: &State) -> String {
        let (w, h) = (self.w as usize, self.h as usize);
        let mut out = String::with_capacity((w + 1) * h);
        for y in 0..h {
            if y > 0 {
                out.push('\n');
            }
            for x in 0..w {
                let i = y * w + x;
                let exit = (x, y) == (self.exit.0 as usize, self.exit.1 as usize);
                let player = (x, y) == (state.x as usize, state.y as usize);
                let carried = state.carrying && (x, y + 1) == (state.x as usize, state.y as usize);
                let block = carried || state.blocks.get(i);
                out.push(match () {
                    _ if player && state.facing == Facing::Right => '>',
                    _ if player => '<',
                    _ if self.walls.get(i) => '#',
                    _ if block && exit => 'b',
                    _ if block => 'B',
                    _ if exit => 'E',
                    _ => '.',
                });
            }
        }
        out
    }

    /// FNV-1a 64 over the text form of the start: what the bench suite pins each seed's level to.
    pub fn fingerprint(&self) -> u64 {
        self.to_string().bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3))
    }

    /// Whether the player stands on the exit.
    pub fn solved(&self, s: &State) -> bool {
        (s.x, s.y) == self.exit
    }

    /// Whether the cell at (x, y) is a wall or a block in `blocks`. Outside the grid counts as wall.
    pub fn solid(&self, blocks: &Bits, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return true;
        }
        let i = y as usize * self.w as usize + x as usize;
        self.walls.get(i) || blocks.get(i)
    }
}

/// Why a key changed nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The cell you would walk into is filled.
    Blocked,
    /// The block you carry would hit something.
    CarryBlocked,
    /// Climbing: the cell in front is empty.
    NothingToClimb,
    /// Climbing: the cell above you, or above the step, is filled. Lifting: the cell above you is filled.
    NoHeadroom,
    /// Lifting: there is no block in front of you at your level.
    NoBlock,
    /// Lifting: something sits on the block in front.
    Covered,
    /// Putting down: the cell in front at head height is filled.
    NoRoom,
    /// A letter in a move string that is not `L`, `R`, `U` or `D`.
    NotAKey,
}

/// Where and why a move string stopped early.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stop {
    pub index: usize,
    pub key: char,
    pub why: Refusal,
}

/// What [`Play::apply`] did with a move string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Applied {
    pub played: usize,
    pub stop: Option<Stop>,
    pub solved: bool,
}

/// A level being played: its states from the start, so moves can be undone. The moves that count are the ones on the
/// board, so undo and restart cost nothing (P10).
#[derive(Clone, Debug)]
pub struct Play {
    pub level: Level,
    history: Vec<State>,
}

impl Play {
    pub fn new(level: Level) -> Play {
        Play { history: vec![level.start], level }
    }
    pub fn state(&self) -> &State {
        self.history.last().expect("history holds the start")
    }
    pub fn moves(&self) -> usize {
        self.history.len() - 1
    }
    pub fn solved(&self) -> bool {
        self.level.solved(self.state())
    }
    pub fn show(&self) -> String {
        self.level.show(self.state())
    }
    /// Plays `keys` until they run out, the level is solved or a key is refused.
    pub fn apply(&mut self, keys: &str) -> Applied {
        let mut played = 0;
        for (index, key) in keys.chars().enumerate() {
            if self.solved() {
                break;
            }
            let next = Key::from_char(key).ok_or(Refusal::NotAKey).and_then(|k| step(&self.level, self.state(), k));
            match next {
                Ok(s) => self.history.push(s),
                Err(why) => return Applied { played, stop: Some(Stop { index, key, why }), solved: false },
            }
            played += 1;
        }
        Applied { played, stop: None, solved: self.solved() }
    }
    /// Takes back up to `n` moves; returns how many it took back.
    pub fn undo(&mut self, n: usize) -> usize {
        let n = n.min(self.moves());
        self.history.truncate(self.history.len() - n);
        n
    }
    pub fn restart(&mut self) {
        self.history.truncate(1);
    }
}

/// Plays one key. A refused key changes nothing and is not a move.
pub fn step(level: &Level, s: &State, key: Key) -> Result<State, Refusal> {
    let mut n = *s;
    let (x, y) = (s.x as i32, s.y as i32);
    let solid = |x, y| level.solid(&s.blocks, x, y);
    match key {
        Key::L | Key::R => {
            let way = if key == Key::L { Facing::Left } else { Facing::Right };
            if s.facing != way {
                n.facing = way;
                return Ok(n);
            }
            let d = dx(way);
            if solid(x + d, y) {
                return Err(Refusal::Blocked);
            }
            if s.carrying && solid(x + d, y - 1) {
                return Err(Refusal::CarryBlocked);
            }
            n.x = (x + d) as u8;
        }
        Key::U => {
            let d = dx(s.facing);
            if !solid(x + d, y) {
                return Err(Refusal::NothingToClimb);
            }
            // The carried block is not in `blocks`, so the cell above you reads empty while you carry.
            if solid(x + d, y - 1) || solid(x, y - 1) {
                return Err(Refusal::NoHeadroom);
            }
            if s.carrying && solid(x + d, y - 2) {
                return Err(Refusal::CarryBlocked);
            }
            n.x = (x + d) as u8;
            n.y = (y - 1) as u8;
        }
        Key::D if !s.carrying => {
            let (fx, w) = (x + dx(s.facing), level.w as i32);
            if !(0..w).contains(&fx) || !s.blocks.get((y * w + fx) as usize) {
                return Err(Refusal::NoBlock);
            }
            if solid(fx, y - 1) {
                return Err(Refusal::Covered);
            }
            if solid(x, y - 1) {
                return Err(Refusal::NoHeadroom);
            }
            n.blocks.set((y * w + fx) as usize, false);
            n.carrying = true;
        }
        Key::D => {
            let (fx, mut by) = (x + dx(s.facing), y - 1);
            if solid(fx, by) {
                return Err(Refusal::NoRoom);
            }
            while !solid(fx, by + 1) {
                by += 1;
            }
            n.blocks.set((by * level.w as i32 + fx) as usize, true);
            n.carrying = false;
        }
    }
    fall(level, &mut n);
    Ok(n)
}

fn dx(facing: Facing) -> i32 {
    if facing == Facing::Left { -1 } else { 1 }
}

/// Drops the player (and any carried block) until something is under them.
fn fall(level: &Level, s: &mut State) {
    while !level.solid(&s.blocks, s.x as i32, s.y as i32 + 1) {
        s.y += 1;
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.show(&self.start))
    }
}
