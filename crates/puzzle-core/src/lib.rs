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
    fn count(&self) -> usize {
        self.0.iter().map(|w| w.count_ones() as usize).sum()
    }
    /// The set cells, in order.
    fn ones(self) -> impl Iterator<Item = usize> {
        (0..4).flat_map(move |k| {
            let mut word = self.0[k];
            std::iter::from_fn(move || {
                (word != 0).then(|| {
                    let bit = word.trailing_zeros() as usize;
                    word &= word - 1;
                    64 * k + bit
                })
            })
        })
    }
    /// The cells set in both.
    pub fn zip_and(self, o: Bits) -> Bits {
        self.zip(o, |a, b| a & b)
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
    /// Which of `blocks` are ice (M3): set down or knocked, they slide.
    pub ice: Bits,
    /// Whether the block you carry is ice.
    pub carrying_ice: bool,
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
    /// Reads the text form: `#` wall, `~` cracked floor, `B` block, `I` ice block, `E` exit, `.` empty, `>`/`<` the
    /// player facing that way, `b`/`i` a block/ice block on the exit. A block directly above the player is the one they
    /// carry.
    pub fn parse(text: &str) -> Result<Level, ParseError> {
        let rows: Vec<Vec<char>> = text.lines().map(|l| l.chars().collect()).collect();
        let (h, w) = (rows.len(), rows.first().map_or(0, Vec::len));
        if w == 0 {
            return Err(ParseError::Empty);
        }
        if w > MAX_W || h > MAX_H {
            return Err(ParseError::TooBig { w, h });
        }
        let (mut walls, mut blocks, mut cracked, mut ice) =
            (Bits::default(), Bits::default(), Bits::default(), Bits::default());
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
                    'I' => {
                        blocks.set(i, true);
                        ice.set(i, true);
                    }
                    'i' => {
                        blocks.set(i, true);
                        ice.set(i, true);
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
        let carrying_ice = carrying && ice.get((py - 1) * w + px);
        if carrying {
            blocks.set((py - 1) * w + px, false);
            ice.set((py - 1) * w + px, false);
        }
        let start = State { x: px as u8, y: py as u8, facing, carrying, blocks, cracked, ice, carrying_ice };
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
                let ice = if carried { state.carrying_ice } else { state.ice.get(i) };
                out.push(match () {
                    _ if player && state.facing == Facing::Right => '>',
                    _ if player => '<',
                    _ if self.walls.get(i) => '#',
                    _ if state.cracked.get(i) => '~',
                    _ if block && exit && ice => 'i',
                    _ if block && exit => 'b',
                    _ if block && ice => 'I',
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
    step_traced(level, s, key, |_, _| {})
}

/// [`step`], calling `on_tick` with the state after the key acts and after each round of things falling and sliding,
/// so the browser can animate a move as it plays out. With each state come the cells of the blocks that fell a row in
/// that round (where they landed); any other block that moved slid a cell.
pub fn step_traced(
    level: &Level,
    s: &State,
    key: Key,
    mut on_tick: impl FnMut(&State, Bits),
) -> Result<State, Refusal> {
    let mut n = *s;
    let mut movers = vec![];
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
            n.carrying_ice = s.ice.get((y * w + fx) as usize);
            n.ice.set((y * w + fx) as usize, false);
        }
        Key::D => {
            // Set down in front of your head; settling drops it the rest of the way.
            let fx = x + dx(s.facing);
            if solid(fx, y - 1) {
                return Err(Refusal::NoRoom);
            }
            let at = ((y - 1) * level.w as i32 + fx) as usize;
            n.blocks.set(at, true);
            n.ice.set(at, s.carrying_ice);
            (n.carrying, n.carrying_ice) = (false, false);
            movers.push(Mover { at, dir: dx(s.facing), slides: s.carrying_ice, riders: false });
        }
    }
    settle(level, &mut n, movers, &mut on_tick);
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

/// Something moving sideways while a move plays out: ice sliding, or a block just set down, which slides only if it is
/// ice but carries your facing to knock any ice it lands on (P35). `riders`: ice passed motion by other ice carries
/// what sits on it (P37).
#[derive(Clone, Copy, Debug)]
struct Mover {
    at: usize,
    dir: i32,
    slides: bool,
    riders: bool,
}

/// Lets everything come to rest after a move (`docs/planning/puzzle-mechanics.md`, "settle"), in rounds. Each round:
///
/// - **Falling:** everything unsupported falls a row, stacks together, you with what you carry. You and your carried
///   block are solid to blocks landing on you.
/// - **Landing:** something that fell last round and has stopped on resting ice knocks it sliding the way it was going
///   (a block you set down: your facing; you: your facing; sliding ice: its way), and drops into the gap (P35).
///   Something falling with no way of its own (dropping because ice slid out from under it) just lands.
/// - **Sliding:** supported sliding ice moves a cell. Hitting resting ice, it stops and that ice slides on, carrying
///   what sits on it (P36, P37); hitting anything else, it stops (P34).
///
/// Everything you set moving moves away from your column, so you never ride ice. Every slide moves one way and every
/// fall moves down, so the rounds end.
fn settle(level: &Level, s: &mut State, mut movers: Vec<Mover>, on_tick: &mut impl FnMut(&State, Bits)) {
    let (w, h) = (level.w as usize, level.h as usize);
    let floor = Bits::range(w * (h - 1), w * h);
    let or = |a: Bits, b: Bits| a.zip(b, |a, b| a | b);
    let minus = |a: Bits, b: Bits| a.zip(b, |a, b| a & !b);
    // Every cell that blocks movement: walls, cracked tiles, blocks, you and what you carry.
    let solid = |s: &State| {
        let mut solid = or(or(level.walls, s.cracked), s.blocks);
        let me = s.y as usize * w + s.x as usize;
        solid.set(me, true);
        if s.carrying {
            solid.set(me - w, true);
        }
        solid
    };
    on_tick(s, Bits::default());
    let (mut landed, mut fell) = (Bits::default(), false);
    let rounds = 4 * w * h * (1 + s.ice.count() + usize::from(s.carrying_ice));
    for _ in 0..rounds {
        let mut moved = false;
        let me = s.y as usize * w + s.x as usize;
        // Falling. Fast path: nothing can fall when every block and you stand directly on something.
        let under = or(solid(s).up(w), floor);
        let (mut dropping, mut falls) = (minus(s.blocks, under), !under.get(me));
        if !dropping.is_empty() || falls {
            // Something hangs: find what rests on walls, cracked tiles or the bottom edge, through whatever stacks on
            // them, and let everything else fall together.
            let base = or(level.walls, s.cracked);
            let (mut held, mut me_held) = (Bits::default(), false);
            loop {
                let mut sup = or(base, held);
                if me_held {
                    sup.set(me, true);
                    if s.carrying {
                        sup.set(me - w, true);
                    }
                }
                let under = or(sup.up(w), floor);
                let (next, next_me) = (s.blocks.zip(under, |a, b| a & b), under.get(me));
                if next == held && next_me == me_held {
                    break;
                }
                (held, me_held) = (next, next_me);
            }
            (dropping, falls) = (minus(s.blocks, held), !me_held);
        }
        // Landing: what fell last round and now stays put, on resting ice.
        let sliding = |movers: &[Mover], i: usize| movers.iter().any(|m| m.slides && m.at == i);
        let mut knocks = vec![];
        for i in landed.zip(dropping, |a, b| a & !b).ones() {
            let dir = movers.iter().find(|m| m.at == i).map(|m| m.dir);
            if let Some(dir) = dir.filter(|_| i + w < w * h && s.ice.get(i + w) && !sliding(&movers, i + w)) {
                knocks.push(Mover { at: i + w, dir, slides: true, riders: false });
            }
        }
        if fell && !falls && me + w < w * h && s.ice.get(me + w) && !sliding(&movers, me + w) {
            knocks.push(Mover { at: me + w, dir: dx(s.facing), slides: true, riders: false });
        }
        // A block set down that has stopped has done its knocking.
        movers.retain(|m| m.slides || dropping.get(m.at));
        movers.extend(knocks);
        if !dropping.is_empty() {
            let ice = s.ice.zip(dropping, |a, b| a & b);
            s.blocks = or(minus(s.blocks, dropping), dropping.down(w));
            s.ice = or(minus(s.ice, ice), ice.down(w));
            for m in movers.iter_mut().filter(|m| dropping.get(m.at)) {
                m.at += w;
            }
            moved = true;
        }
        if falls {
            s.y += 1;
            moved = true;
        }
        (landed, fell) = (dropping.down(w), falls);
        // Sliding.
        let mut k = 0;
        while k < movers.len() {
            let m = movers[k];
            k += 1;
            let here = solid(s);
            if !m.slides || (m.at + w < w * h && !here.get(m.at + w)) {
                continue;
            }
            let x = (m.at % w) as i32 + m.dir;
            let to = (m.at as i32 + m.dir) as usize;
            if !(0..w as i32).contains(&x) || here.get(to) {
                movers.remove(k - 1);
                k -= 1;
                if (0..w as i32).contains(&x) && s.ice.get(to) && !sliding(&movers, to) {
                    movers.push(Mover { at: to, dir: m.dir, slides: true, riders: true });
                }
                continue;
            }
            let shift = |s: &mut State, from: usize, to: usize| {
                let ice = s.ice.get(from);
                s.blocks.set(from, false);
                s.ice.set(from, false);
                s.blocks.set(to, true);
                s.ice.set(to, ice);
            };
            shift(s, m.at, to);
            if m.riders {
                let mut c = m.at;
                while c >= w && s.blocks.get(c - w) && !solid(s).get(c - w + to - m.at) {
                    c -= w;
                    shift(s, c, c + to - m.at);
                }
            }
            movers[k - 1].at = to;
            moved = true;
        }
        if !moved {
            return;
        }
        on_tick(s, landed);
    }
    debug_assert!(false, "settling didn't end");
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.show(&self.start))
    }
}
