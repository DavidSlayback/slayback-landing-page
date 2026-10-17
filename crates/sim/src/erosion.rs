//! The lettering erodes where sparks strike it, then re-welds (David, 2026-10-17: "what if we had the letters themselves
//! erode over time on being struck, and then occasionally reform when they reach some percentage eroded?"; chose
//! "Scorch, then flake").
//!
//! Works on the text mask's cells, only those the page marked erodible (255; other text is 200 and never erodes, so
//! the tagline and links stay readable). A strike scorches the cell it lands on and, less, its neighbours; a cell scorched
//! through flakes off (the mask loses it, so the SDF and the sparks treat it as open air), taking badly scorched
//! neighbours with it. Once the given share of the text is gone, the letters re-weld: the lost cells come back in a
//! random order over [`REWELD`] seconds, glowing as they return, while all scorch cools away.

/// How long a re-weld takes, seconds.
pub const REWELD: f32 = 1.5;
/// How long a re-welded cell glows, seconds.
const GLOW: f32 = 0.6;
/// Scorch at which a cell's neighbour flakes off with it.
const CRUMBLE: f32 = 0.7;

pub struct Erosion {
    w: usize,
    h: usize,
    /// Erodible at the last commit from the page: what re-welding restores.
    original: Vec<bool>,
    solid_total: usize,
    scorch: Vec<f32>,
    glow: Vec<f32>,
    /// Cells struck since the last tick (they're the only ones that can flake).
    hot: Vec<u32>,
    gone: usize,
    /// Seconds into a re-weld, while one runs.
    reweld: Option<f32>,
    /// Per cell for the renderer: scorch, gone, glow (0..255 each), 0. Repacked when anything changes.
    pub out: Vec<u8>,
    /// Bumped whenever `out` (and so possibly the mask) changes.
    pub version: u32,
    /// Cells whose `out` is stale; `full` when all of them are (a reset, a re-weld, glowing cells).
    touched: Vec<u32>,
    full: bool,
    glowing: bool,
}

impl Erosion {
    pub fn new() -> Self {
        Erosion {
            w: 0,
            h: 0,
            original: Vec::new(),
            solid_total: 0,
            scorch: Vec::new(),
            glow: Vec::new(),
            hot: Vec::new(),
            gone: 0,
            reweld: None,
            out: Vec::new(),
            version: 0,
            touched: Vec::new(),
            full: false,
            glowing: false,
        }
    }

    /// Starts over on a freshly committed mask, `erodible[i]` for `w` × `h` cells: nothing scorched or gone.
    pub fn reset(&mut self, erodible: &[bool], w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.original = erodible.to_vec();
        self.solid_total = erodible.iter().filter(|s| **s).count();
        self.scorch = vec![0.0; w * h];
        self.glow = vec![0.0; w * h];
        self.hot.clear();
        self.gone = 0;
        self.reweld = None;
        self.out = vec![0; w * h * 4];
        self.touched.clear();
        self.full = true;
        self.glowing = false;
    }

    /// The share of the text's cells gone, 0..1.
    pub fn eroded(&self) -> f32 {
        if self.solid_total == 0 { 0.0 } else { self.gone as f32 / self.solid_total as f32 }
    }

    /// A strike of `amount` at cell (`x`, `y`): it scorches that cell fully and its eight neighbours by half.
    pub fn strike(&mut self, x: usize, y: usize, amount: f32) {
        if self.reweld.is_some() || x >= self.w || y >= self.h {
            return;
        }
        for (dx, dy) in [(0i32, 0i32), (-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
            let (cx, cy) = (x as i32 + dx, y as i32 + dy);
            if cx < 0 || cy < 0 || cx as usize >= self.w || cy as usize >= self.h {
                continue;
            }
            let i = cy as usize * self.w + cx as usize;
            if !self.original[i] {
                continue;
            }
            self.scorch[i] += if dx == 0 && dy == 0 { amount } else { amount * 0.5 };
            self.hot.push(i as u32);
            self.touched.push(i as u32);
        }
    }

    /// Moves on `dt` seconds over `mask` (255 solid, 0 open): flakes off what's scorched through, starts a re-weld
    /// once `reweld_at` of the text is gone, and carries one on. Returns the cells that flaked this tick, and whether
    /// the mask changed (so the SDF must be rebuilt).
    // Walks the mask, the original and the scorch together, cell by cell.
    #[allow(clippy::needless_range_loop)]
    pub fn tick(
        &mut self,
        dt: f32,
        mask: &mut [u8],
        reweld_at: f32,
        rand: &mut impl FnMut() -> f32,
    ) -> (Vec<u32>, bool) {
        let mut flaked = Vec::new();
        let mut mask_changed = false;
        for &i in &std::mem::take(&mut self.hot) {
            let i = i as usize;
            if self.scorch[i] < 1.0 || mask[i] == 0 {
                continue;
            }
            // Through: it flakes, and badly scorched neighbours crumble with it.
            let (x, y) = (i % self.w, i / self.w);
            for (dx, dy) in [(0i32, 0i32), (-1, 0), (1, 0), (0, -1), (0, 1)] {
                let (cx, cy) = (x as i32 + dx, y as i32 + dy);
                if cx < 0 || cy < 0 || cx as usize >= self.w || cy as usize >= self.h {
                    continue;
                }
                let j = cy as usize * self.w + cx as usize;
                if self.original[j] && mask[j] != 0 && (j == i || self.scorch[j] >= CRUMBLE) {
                    mask[j] = 0;
                    self.gone += 1;
                    flaked.push(j as u32);
                    self.touched.push(j as u32);
                }
            }
            mask_changed = true;
        }
        if self.reweld.is_none() && self.solid_total > 0 && self.eroded() >= reweld_at {
            self.reweld = Some(0.0);
        }
        if let Some(t) = self.reweld {
            let t = t + dt;
            let share = (t / REWELD).min(1.0);
            for i in 0..mask.len() {
                if !self.original[i] {
                    continue;
                }
                // Each lost cell returns at its own moment through the re-weld, glowing.
                if mask[i] == 0 && rand() < dt / (REWELD * (1.0 - share).max(dt / REWELD)) {
                    mask[i] = 255;
                    self.glow[i] = 1.0;
                    self.glowing = true;
                    self.gone -= 1;
                    mask_changed = true;
                }
                self.scorch[i] *= 1.0 - share;
            }
            if share >= 1.0 {
                for i in 0..mask.len() {
                    if self.original[i] && mask[i] == 0 {
                        mask[i] = 255;
                        self.glow[i] = 1.0;
                    }
                    self.scorch[i] = 0.0;
                }
                self.gone = 0;
                self.reweld = None;
                mask_changed = true;
            } else {
                self.reweld = Some(t);
            }
            self.glowing = true;
            self.full = true;
        }
        if self.glowing {
            // Re-welded cells cool; while any glow, the whole picture is repacked each tick.
            self.glowing = false;
            for g in self.glow.iter_mut().filter(|g| **g > 0.0) {
                *g = (*g - dt / GLOW).max(0.0);
                self.glowing = true;
            }
            self.full = true;
        }
        let pack = |out: &mut [u8], i: usize, scorch: f32, glow: f32, gone: bool| {
            out[i * 4] = (scorch.min(1.0) * 255.0) as u8;
            out[i * 4 + 1] = if gone { 255 } else { 0 };
            out[i * 4 + 2] = (glow * 255.0) as u8;
        };
        if self.full {
            for i in 0..mask.len() {
                pack(&mut self.out, i, self.scorch[i], self.glow[i], self.original[i] && mask[i] == 0);
            }
        } else {
            // Most frames only a few cells were struck or flaked: repack just those.
            for &i in &self.touched {
                let i = i as usize;
                pack(&mut self.out, i, self.scorch[i], self.glow[i], self.original[i] && mask[i] == 0);
            }
        }
        if self.full || !self.touched.is_empty() {
            self.version = self.version.wrapping_add(1);
        }
        self.touched.clear();
        self.full = false;
        (flaked, mask_changed)
    }
}

impl Default for Erosion {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 20 × 10 mask with a 10 × 6 block of text in it.
    fn block() -> (Erosion, Vec<u8>) {
        let (w, h) = (20, 10);
        let mask: Vec<u8> =
            (0..w * h).map(|i| if (5..15).contains(&(i % w)) && (2..8).contains(&(i / w)) { 255 } else { 0 }).collect();
        let mut e = Erosion::new();
        e.reset(&mask.iter().map(|m| *m > 127).collect::<Vec<_>>(), w, h);
        (e, mask)
    }

    fn rng() -> impl FnMut() -> f32 {
        let mut s = 12345u32;
        move || {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            (s % 10_000) as f32 / 10_000.0
        }
    }

    #[test]
    fn text_the_page_kept_from_eroding_never_scorches_or_flakes() {
        let (w, h) = (20, 10);
        // The tagline: solid (200) but not erodible (only 255 is).
        let mut mask: Vec<u8> = (0..w * h).map(|i| if (5..15).contains(&(i % w)) { 200 } else { 0 }).collect();
        let mut e = Erosion::new();
        e.reset(&mask.iter().map(|m| *m == 255).collect::<Vec<_>>(), w, h);
        let mut r = rng();
        for _ in 0..10 {
            e.strike(8, 4, 1.0);
        }
        let (flaked, changed) = e.tick(0.016, &mut mask, 0.4, &mut r);
        assert!(flaked.is_empty() && !changed);
        assert!(mask.iter().all(|m| *m == 0 || *m == 200), "untouched");
        assert!(e.out.chunks(4).all(|c| c[0] == 0), "unscorched");
    }

    #[test]
    fn strikes_scorch_the_text_before_any_of_it_flakes() {
        let (mut e, mut mask) = block();
        let mut r = rng();
        e.strike(8, 4, 0.4);
        e.strike(8, 4, 0.4);
        let (flaked, changed) = e.tick(0.016, &mut mask, 0.4, &mut r);
        assert!(flaked.is_empty() && !changed, "scorched, not through");
        assert!(e.out[(4 * 20 + 8) * 4] > 150, "the struck cell shows its scorch");
        assert!(e.out[(4 * 20 + 9) * 4] > 50, "and its neighbour some");
        assert_eq!(e.out[0], 0, "open air (the corner cell) never scorches");
    }

    #[test]
    fn a_cell_scorched_through_flakes_off_with_its_badly_scorched_neighbours() {
        let (mut e, mut mask) = block();
        let mut r = rng();
        for _ in 0..3 {
            e.strike(8, 4, 0.5);
        }
        let (flaked, changed) = e.tick(0.016, &mut mask, 0.4, &mut r);
        assert!(changed && flaked.contains(&(4 * 20 + 8)), "flaked: {flaked:?}");
        assert!(flaked.len() >= 5, "neighbours scorched to 0.75 crumble too: {flaked:?}");
        assert_eq!(mask[4 * 20 + 8], 0, "the mask lost it: sparks pass through");
        assert_eq!(e.out[(4 * 20 + 8) * 4 + 1], 255, "and the renderer sees a hole");
        assert!(e.eroded() > 0.05);
    }

    #[test]
    fn at_the_threshold_the_letters_re_weld_glowing_and_cool() {
        let (mut e, mut mask) = block();
        let mut r = rng();
        // Burn away more than 40% of it.
        for y in 2..8 {
            for x in 5..10 {
                e.strike(x, y, 3.0);
            }
        }
        e.tick(0.016, &mut mask, 0.4, &mut r);
        assert!(e.eroded() >= 0.4, "eroded {}", e.eroded());
        let mut t = 0.0;
        let mut saw_glow = false;
        while t < REWELD + 0.1 {
            e.tick(0.016, &mut mask, 0.4, &mut r);
            saw_glow |= e.out.chunks(4).any(|c| c[2] > 100);
            t += 0.016;
        }
        assert!(saw_glow, "returning cells glow");
        assert_eq!(e.eroded(), 0.0);
        let solid = mask.iter().filter(|m| **m == 255).count();
        assert_eq!(solid, 60, "every cell of the block is back");
        assert!(e.out.chunks(4).all(|c| c[0] == 0 && c[1] == 0), "no scorch, no holes");
    }
}
