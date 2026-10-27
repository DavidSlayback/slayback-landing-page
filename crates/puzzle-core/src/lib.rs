//! Rules of *Out of the Box* (`docs/planning/puzzle-spec.md`, "Rules").
//!
//! A level is a grid of walls, loose blocks and one exit. The player walks, turns, climbs one cell and carries one
//! block on their head. Everything is plain data with no dependencies, so the browser, the MCP server, the RL env and
//! the bench all run the same rules.

use std::fmt;

mod generate;
mod solve;
pub use generate::{CAP, Generated, TIERS, Tier, World, generate};
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
    fn is_empty(&self) -> bool {
        self.0 == [0; 4]
    }
    pub(crate) fn zip(self, o: Bits, f: impl Fn(u64, u64) -> u64) -> Bits {
        Bits(std::array::from_fn(|k| f(self.0[k], o.0[k])))
    }
    /// The cells from `lo` up to, not including, `hi`.
    fn range(lo: usize, hi: usize) -> Bits {
        let below = |n: usize| {
            Bits(std::array::from_fn(|k| match n.saturating_sub(64 * k) {
                0 => 0,
                m if m >= 64 => !0,
                m => (1 << m) - 1,
            }))
        };
        below(hi).zip(below(lo), |a, b| a & !b)
    }
    /// Every cell moved `n` (1 to 63) cells later in the grid's order: with `n = w`, one row down.
    fn down(self, n: usize) -> Bits {
        Bits(std::array::from_fn(|k| self.0[k] << n | if k > 0 { self.0[k - 1] >> (64 - n) } else { 0 }))
    }
    /// Every cell moved `n` (1 to 63) cells earlier: with `n = w`, one row up.
    fn up(self, n: usize) -> Bits {
        Bits(std::array::from_fn(|k| self.0[k] >> n | if k < 3 { self.0[k + 1] << (64 - n) } else { 0 }))
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
    /// Cracked floor tiles still standing (M2): solid until whatever stands directly on one leaves it.
    pub cracked: Bits,
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
    /// Reads the text form: `#` wall, `~` cracked floor, `B` block, `E` exit, `.` empty, `>`/`<` the player facing that
    /// way, `b` a block resting on the exit. A `B` directly above the player is the block they carry.
    pub fn parse(text: &str) -> Result<Level, ParseError> {
        let rows: Vec<Vec<char>> = text.lines().map(|l| l.chars().collect()).collect();
        let (h, w) = (rows.len(), rows.first().map_or(0, Vec::len));
        if w == 0 {
            return Err(ParseError::Empty);
        }
        if w > MAX_W || h > MAX_H {
            return Err(ParseError::TooBig { w, h });
        }
        let (mut walls, mut blocks, mut cracked) = (Bits::default(), Bits::default(), Bits::default());
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
                    '~' => cracked.set(i, true),
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
        let start = State { x: px as u8, y: py as u8, facing, carrying, blocks, cracked };
        let level = Level { w: w as u8, h: h as u8, walls, exit: (ex as u8, ey as u8), start };
        let (x, y) = (px as i32, py as i32);
        if !level.solid(&start, x, y + 1) {
            return Err(ParseError::Floating { x: px, y: py });
        }
        for i in (0..w * h).filter(|&i| blocks.get(i)) {
            let (x, y) = ((i % w) as i32, (i / w) as i32);
            if !level.solid(&start, x, y + 1) {
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
                    _ if state.cracked.get(i) => '~',
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

    /// Whether the cell at (x, y) is a wall, or a block or a cracked tile in `s`. Outside the grid counts as wall.
    pub fn solid(&self, s: &State, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return true;
        }
        let i = y as usize * self.w as usize + x as usize;
        self.walls.get(i) || s.blocks.get(i) || s.cracked.get(i)
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
    let solid = |x, y| level.solid(s, x, y);
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
            // Set down in front of your head; settling drops it the rest of the way.
            let fx = x + dx(s.facing);
            if solid(fx, y - 1) {
                return Err(Refusal::NoRoom);
            }
            n.blocks.set(((y - 1) * level.w as i32 + fx) as usize, true);
            n.carrying = false;
        }
    }
    settle(level, &mut n);
    crumble(level, s, &mut n);
    Ok(n)
}

/// Breaks every cracked tile that something stood directly on before the move and nothing stands on after it (P28,
/// P29). Whatever left was the only thing on the tile, so nothing is left hanging.
fn crumble(level: &Level, before: &State, after: &mut State) {
    if after.cracked.is_empty() {
        return;
    }
    let w = level.w as usize;
    let on = |s: &State| {
        let mut on = s.blocks;
        on.set(s.y as usize * w + s.x as usize, true);
        on
    };
    let left = on(before).zip(on(after), |a, b| a & !b);
    after.cracked = after.cracked.zip(left.down(w), |a, b| a & !b);
}

fn dx(facing: Facing) -> i32 {
    if facing == Facing::Left { -1 } else { 1 }
}

/// Lets everything come to rest after a move (`docs/planning/puzzle-mechanics.md`, "settle"): each round, every
/// unsupported block falls a row, all at once, and so do you with what you carry. You and your carried block are
/// solid to blocks landing on you. Blocks are identical and each column just packs down, so the resting state doesn't
/// depend on the order.
fn settle(level: &Level, s: &mut State) {
    let (w, h) = (level.w as usize, level.h as usize);
    let floor = Bits::range(w * (h - 1), w * h);
    loop {
        let mut solid = level.walls.zip(s.blocks, |a, b| a | b).zip(s.cracked, |a, b| a | b);
        let me = s.y as usize * w + s.x as usize;
        let falls = me + w < w * h && !solid.get(me + w);
        solid.set(me, true);
        if s.carrying {
            solid.set(me - w, true);
        }
        let held = solid.up(w).zip(floor, |a, b| a | b);
        let dropping = s.blocks.zip(held, |a, b| a & !b);
        if dropping.is_empty() && !falls {
            return;
        }
        s.blocks = s.blocks.zip(dropping, |a, b| a & !b).zip(dropping.down(w), |a, b| a | b);
        if falls {
            s.y += 1;
        }
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.show(&self.start))
    }
}
