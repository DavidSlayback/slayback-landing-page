//! Times the solver on random settled levels of each size up to 20 × 12, solvable and not.
//!
//! `bash scripts/native.sh run -p puzzle-core --release --example solve [levels per size, default 50]`. At 20 × 12 an
//! unsolvable level can take a minute and several GB (docs/planning/puzzle-spec.md, "Measured"), so keep the count low.
//!
//! Proving a level unsolvable means expanding every reachable state, which is what the generator pays for each level
//! it throws away, so the two cases are reported apart.

use std::time::Instant;

use puzzle_core::{Level, solve};

/// xorshift64*, as in the tests.
struct Rng(u64);
impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 33) as usize % n
    }
}

/// A walled pit of exactly `w` × `h` with scattered wall cells and about `density` of its cells in blocks, dropped
/// down their columns, then the exit and the player dropped in. `None` when a full column swallowed one of them.
fn level(rng: &mut Rng, w: usize, h: usize, density: f32) -> Option<Level> {
    let mut g = vec![vec!['.'; w]; h];
    for (y, row) in g.iter_mut().enumerate() {
        for (x, c) in row.iter_mut().enumerate() {
            if x == 0 || x == w - 1 || y == h - 1 || rng.below(7) == 0 {
                *c = '#';
            }
        }
    }
    let mut drop = |g: &mut Vec<Vec<char>>, c: char| {
        let x = 1 + rng.below(w - 2);
        let Some(mut y) = (0..h).find(|&y| g[y][x] == '.') else { return };
        while y + 1 < h && g[y + 1][x] == '.' {
            y += 1;
        }
        g[y][x] = c;
    };
    for _ in 0..(density * (w * h) as f32) as usize {
        drop(&mut g, 'B');
    }
    drop(&mut g, 'E');
    drop(&mut g, '>');
    Level::parse(&g.iter().map(|r| r.iter().collect::<String>()).collect::<Vec<_>>().join("\n")).ok()
}

#[derive(Default)]
struct Tally {
    ms: Vec<f64>,
    expanded: Vec<usize>,
    longest: usize,
}

impl Tally {
    fn line(&mut self, what: &str) -> String {
        if self.ms.is_empty() {
            return format!("{what:>10}: none");
        }
        self.ms.sort_by(f64::total_cmp);
        self.expanded.sort();
        let (n, mid) = (self.ms.len(), self.ms.len() / 2);
        format!(
            "{what:>10}: {n:>4}  ms median {:>8.3} max {:>9.3}  expanded median {:>8} max {:>9}{}",
            self.ms[mid],
            self.ms[n - 1],
            self.expanded[mid],
            self.expanded[n - 1],
            if self.longest > 0 { format!("  longest {}", self.longest) } else { String::new() }
        )
    }
}

fn main() {
    let per: usize = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(50);
    let mut rng = Rng(0x501e_bea7);
    for (w, h) in [(8, 5), (12, 8), (16, 10), (20, 12)] {
        for density in [0.05, 0.15] {
            let (mut solved, mut unsolved) = (Tally::default(), Tally::default());
            for _ in 0..per {
                let Some(l) = level(&mut rng, w, h, density) else { continue };
                let t = Instant::now();
                let found = solve(&l);
                let ms = t.elapsed().as_secs_f64() * 1e3;
                let tally = if found.solution.is_some() { &mut solved } else { &mut unsolved };
                tally.ms.push(ms);
                tally.expanded.push(found.expanded);
                tally.longest = tally.longest.max(found.solution.map_or(0, |s| s.len()));
            }
            println!("{w}x{h}, blocks {:.0}% of cells:", density * 100.0);
            println!("{}", solved.line("solvable"));
            println!("{}", unsolved.line("unsolvable"));
        }
    }
}
