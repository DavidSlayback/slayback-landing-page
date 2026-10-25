//! Helpers shared by the test files: grids written inline and random settled levels.

#![allow(dead_code)]

use puzzle_core::*;

/// Trims each line so grids can be indented in the source.
pub fn grid(text: &str) -> String {
    text.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")
}

pub fn level(text: &str) -> Level {
    Level::parse(&grid(text)).unwrap_or_else(|e| panic!("{e:?} in\n{}", grid(text)))
}

/// xorshift64*: enough randomness for property tests without a dependency.
pub struct Rng(pub u64);
impl Rng {
    pub fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 33) as usize % n
    }
}

/// A random settled level: a walled pit with scattered wall cells, blocks dropped down their columns, the player and
/// the exit dropped onto whatever is under them.
pub fn random_level(rng: &mut Rng) -> Level {
    random_level_within(rng, MAX_W, MAX_H)
}

/// A random settled level no bigger than `max_w` × `max_h`.
pub fn random_level_within(rng: &mut Rng, max_w: usize, max_h: usize) -> Level {
    let (w, h) = (6 + rng.below(max_w - 5), 4 + rng.below(max_h - 3));
    let mut g = vec![vec!['.'; w]; h];
    for (y, row) in g.iter_mut().enumerate() {
        for (x, c) in row.iter_mut().enumerate() {
            if x == 0 || x == w - 1 || y == h - 1 || rng.below(7) == 0 {
                *c = '#';
            }
        }
    }
    let drop = |g: &mut Vec<Vec<char>>, x: usize, c: char| {
        let Some(mut y) = (0..h).find(|&y| g[y][x] == '.') else { return };
        while y + 1 < h && g[y + 1][x] == '.' {
            y += 1;
        }
        g[y][x] = c;
    };
    for _ in 0..rng.below(w * h / 4) {
        drop(&mut g, 1 + rng.below(w - 2), 'B');
    }
    drop(&mut g, 1 + rng.below(w - 2), 'E');
    drop(&mut g, 1 + rng.below(w - 2), if rng.below(2) == 0 { '<' } else { '>' });
    let text = g.iter().map(|r| r.iter().collect::<String>()).collect::<Vec<_>>().join("\n");
    // A full column can swallow the player or the exit; such a level is skipped by the caller.
    Level::parse(&text).unwrap_or_else(|_| Level::parse("#>E#\n####").unwrap())
}
