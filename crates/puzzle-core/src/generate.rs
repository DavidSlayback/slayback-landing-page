//! Seeded level generation and the difficulty tiers (P16; `docs/planning/puzzle-spec.md`, "Solver and generator").
//!
//! Random pits rarely need more than a few dozen moves, so a level isn't drawn and filtered: it is climbed. A random
//! pit is changed one cell at a time (a wall, a block, where the exit or the player goes), and a change is kept when
//! the level stays solvable within [`CAP`] and its shortest solution gets no shorter. It stops once the solution lands
//! in the tier's band. Everything runs on integers from one seeded generator, so a seed gives the same level
//! everywhere.

use crate::{Facing, Key, Level, solve_within};

/// Which mechanic a world's levels are built around (P38). Classic is suite v1's Block Dude rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum World {
    Classic,
    /// Cracked floor tiles (`~`, M2).
    Crumble,
    /// Ice blocks (`I`, M3).
    Ice,
}

/// A difficulty tier: the band its shortest solutions fall in, and the grid it is built on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tier {
    pub n: u8,
    /// Shortest-solution lengths, inclusive.
    pub moves: (usize, usize),
    pub w: u8,
    pub h: u8,
}

pub const TIERS: [Tier; 4] = [
    Tier { n: 1, moves: (8, 15), w: 8, h: 6 },
    Tier { n: 2, moves: (16, 40), w: 12, h: 8 },
    Tier { n: 3, moves: (41, 80), w: 16, h: 10 },
    Tier { n: 4, moves: (81, 150), w: 20, h: 12 },
];

/// The most states one candidate's search may expand. A candidate that hits it is thrown away as too costly to
/// verify, which bounds generation: proving one big level unsolvable once took 46 M expansions (OB2's measurement).
pub const CAP: usize = 200_000;

/// Candidates tried per climb before starting again from a fresh pit, and climbs per seed before giving up.
const STEPS: usize = 3_000;
const CLIMBS: usize = 8;

/// A generated level with its shortest solution, and what it cost to find.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Generated {
    pub level: Level,
    pub solution: Vec<Key>,
    /// Candidate levels the search tried, each solved within the cap.
    pub tries: usize,
}

/// xorshift64*: small, fast and the same on every platform.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        // splitmix64 spreads neighbouring seeds apart and keeps the state off zero.
        let mut z = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        Rng((z ^ (z >> 31)) | 1)
    }
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        ((self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 32) % n as u64) as usize
    }
}

/// A level under construction: walls, how many blocks are dropped down each column, and the columns the exit and the
/// player are dropped into.
#[derive(Clone)]
struct Pit {
    w: usize,
    h: usize,
    walls: Vec<bool>,
    blocks: Vec<u8>,
    exit: usize,
    player: usize,
    facing: Facing,
    /// Wall cells drawn as cracked floor instead.
    cracked: Vec<bool>,
    /// How many of each column's blocks are ice: the top ones.
    ice: Vec<u8>,
    world: World,
}

impl Pit {
    /// A pit with a rolling floor, a few blocks, and the exit and the player in different columns; in the Crumble
    /// world, some floor tiles cracked too, and in the Ice world, some ice blocks.
    fn new(rng: &mut Rng, w: usize, h: usize, world: World) -> Pit {
        let mut walls = vec![false; w * h];
        let mut floor = 1 + rng.below(h / 3);
        for x in 0..w {
            if x > 0 && x < w - 1 {
                floor = (floor + rng.below(3)).saturating_sub(1).clamp(1, h / 2);
            }
            for y in 0..h {
                walls[y * w + x] = x == 0 || x == w - 1 || y >= h - floor;
            }
        }
        let mut blocks = vec![0; w];
        for _ in 0..w / 3 {
            blocks[1 + rng.below(w - 2)] += 1;
        }
        let exit = 1 + rng.below(w - 2);
        let player = (exit + 1 + rng.below(w - 3)) % (w - 2) + 1;
        let facing = if rng.below(2) == 0 { Facing::Left } else { Facing::Right };
        let mut cracked = vec![false; w * h];
        if world == World::Crumble {
            for _ in 0..w / 3 {
                let x = 1 + rng.below(w - 2);
                if let Some(y) = (0..h).find(|&y| walls[y * w + x]) {
                    cracked[y * w + x] = true;
                }
            }
        }
        let mut ice = vec![0; w];
        if world == World::Ice {
            for _ in 0..w / 4 {
                let x = 1 + rng.below(w - 2);
                blocks[x] += u8::from(blocks[x] == ice[x]);
                ice[x] += 1;
            }
        }
        Pit { w, h, walls, blocks, exit, player, facing, cracked, ice, world }
    }

    /// One random change. The border walls stay. Classic levels draw from ten kinds of change; other worlds add their
    /// own after those, so a classic seed's draws never change.
    fn mutate(&mut self, rng: &mut Rng) {
        let (w, h) = (self.w, self.h);
        let col = 1 + rng.below(w - 2);
        match rng.below(if self.world == World::Classic { 10 } else { 12 }) {
            0..=4 => {
                let i = rng.below(h - 1) * w + col;
                self.walls[i] = !self.walls[i];
            }
            5 | 6 => self.blocks[col] += 1,
            7 => {
                self.blocks[col] = self.blocks[col].saturating_sub(1);
                self.ice[col] = self.ice[col].min(self.blocks[col]);
            }
            8 => self.exit = col,
            9 => {
                self.player = col;
                self.facing = if rng.below(2) == 0 { Facing::Left } else { Facing::Right };
            }
            _ if self.world == World::Crumble => {
                let i = rng.below(h) * w + col;
                self.cracked[i] = !self.cracked[i];
            }
            10 => {
                self.blocks[col] += 1;
                self.ice[col] += 1;
            }
            _ if self.ice[col] > 0 => {
                self.blocks[col] -= 1;
                self.ice[col] -= 1;
            }
            _ => {}
        }
    }

    /// The text form: blocks dropped down their columns, then the exit, then the player, each landing on whatever is
    /// under it. `None` when a column is too full to take what was dropped into it.
    fn level(&self) -> Option<Level> {
        let (w, h) = (self.w, self.h);
        let cell = |(&wall, &cracked)| match (wall, cracked) {
            (true, true) => b'~',
            (true, false) => b'#',
            _ => b'.',
        };
        let mut g: Vec<u8> = self.walls.iter().zip(&self.cracked).map(cell).collect();
        let drop = |g: &mut Vec<u8>, x: usize, c: u8| -> Option<()> {
            let mut y = (0..h).find(|&y| g[y * w + x] == b'.')?;
            while y + 1 < h && g[(y + 1) * w + x] == b'.' {
                y += 1;
            }
            g[y * w + x] = c;
            Some(())
        };
        for x in 1..w - 1 {
            for k in 0..self.blocks[x] {
                drop(&mut g, x, if k + self.ice[x] >= self.blocks[x] { b'I' } else { b'B' })?;
            }
        }
        drop(&mut g, self.exit, b'E')?;
        drop(&mut g, self.player, if self.facing == Facing::Left { b'<' } else { b'>' })?;
        let text: Vec<String> = g.chunks(w).map(|row| String::from_utf8_lossy(row).into_owned()).collect();
        Level::parse(&text.join("\n")).ok()
    }
}

/// How good a candidate is: being what its world and tier want comes first (from tier 2 up, needing a block; outside
/// Classic, its mechanic changing the shortest solution), then the length of the shortest solution. `None` for a
/// candidate that is unsolvable, too costly to verify, or longer than the tier allows.
fn score(level: &Level, world: World, tier: &Tier) -> Option<(bool, Vec<Key>)> {
    let found = solve_within(level, CAP).solution?;
    if found.len() > tier.moves.1 {
        return None;
    }
    // With the blocks fixed in place there are at most 2 × 240 states, so this check is cheap.
    let mut fixed = level.clone();
    fixed.walls = (0..fixed.w as usize * fixed.h as usize).fold(fixed.walls, |mut walls, i| {
        if fixed.start.blocks.get(i) {
            walls.set(i, true);
        }
        walls
    });
    (fixed.start.blocks, fixed.start.ice) = Default::default();
    let needs_block = tier.n < 2 || (!level.start.carrying && solve_within(&fixed, CAP).solution.is_none());
    Some((needs_block && matters(level, world, found.len()), found))
}

/// Whether the world's mechanic changes the level's shortest solution: with every cracked tile made wall (Crumble) or
/// every ice block made a plain one (Ice), the level must be unsolvable or solved in a different number of moves. So a
/// Crumble level is one where a breaking tile either
/// opens the way or costs you the way you'd otherwise take.
fn matters(level: &Level, world: World, moves: usize) -> bool {
    match world {
        World::Classic => true,
        World::Crumble => {
            let mut fixed = level.clone();
            fixed.walls = fixed.walls.zip(fixed.start.cracked, |a, b| a | b);
            fixed.start.cracked = Default::default();
            let search = solve_within(&fixed, CAP);
            level.start.cracked != fixed.start.cracked
                && !search.gave_up
                && search.solution.map(|s| s.len()) != Some(moves)
        }
        World::Ice => {
            let mut plain = level.clone();
            (plain.start.ice, plain.start.carrying_ice) = Default::default();
            let search = solve_within(&plain, CAP);
            level.start != plain.start && !search.gave_up && search.solution.map(|s| s.len()) != Some(moves)
        }
    }
}

/// The level for `seed` in `world` and `tier`, or `None` if the search ran out of tries.
pub fn generate(seed: u64, world: World, tier: &Tier) -> Option<Generated> {
    let mut rng = Rng::new(seed ^ (tier.n as u64) << 56 ^ (world as u64) << 48);
    let (w, h) = (tier.w as usize, tier.h as usize);
    let mut tries = 0;
    for _ in 0..CLIMBS {
        let mut pit = Pit::new(&mut rng, w, h, world);
        let mut best = pit.level().and_then(|l| score(&l, world, tier).map(|s| (l, s)));
        tries += 1;
        for _ in 0..STEPS {
            if let Some((l, (needs_block, solution))) = &best
                && *needs_block
                && solution.len() >= tier.moves.0
            {
                return Some(Generated { level: l.clone(), solution: solution.clone(), tries });
            }
            let mut next = pit.clone();
            next.mutate(&mut rng);
            let Some(l) = next.level() else { continue };
            tries += 1;
            let Some((needs_block, solution)) = score(&l, world, tier) else { continue };
            let better = match &best {
                None => true,
                Some((_, (had_block, had))) => (needs_block, solution.len()) >= (*had_block, had.len()),
            };
            if better {
                pit = next;
                best = Some((l, (needs_block, solution)));
            }
        }
    }
    None
}
