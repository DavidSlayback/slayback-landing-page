//! The lettering erodes where sparks strike it, then re-welds (David, 2026-10-17: "what if we had the letters themselves
//! erode over time on being struck, and then occasionally reform when they reach some percentage eroded?"; chose
//! "Scorch, then flake").
//!
//! Works on the text mask's cells, only those the page marked erodible (255; other text is 200 and never erodes, so
//! the tagline and links stay readable). The erodible text is cut into shards when it's committed: irregular pieces
//! about [`SHARD`] cells across (a jittered Voronoi pattern). A strike scorches the cell it lands on and, less, its
//! neighbours; once a shard's cells average [`BREAK`] scorch, the whole shard breaks off (David: "make the pieces
//! actually come off when eroding instead of leaving the shapes intact"). The mask loses its cells at once, so the SDF
//! and the sparks treat it as open air, and its id joins [`Erosion::broken`] for the page to drop it as a falling
//! fragment. Once the given share of the text is gone, the letters re-weld: the lost cells come back in a random
//! order over [`REWELD`] seconds, glowing as they return, while all scorch cools away.

/// How long a re-weld takes, seconds.
pub const REWELD: f32 = 1.5;
/// How long a re-welded cell glows, seconds.
const GLOW: f32 = 0.6;
/// Shards' spacing, mask cells (the page's mask is half its CSS pixels: about 12 px).
pub const SHARD: usize = 6;
/// The mean scorch over a shard's cells at which it breaks off.
const BREAK: f32 = 0.5;
/// No shard (open air, or text that doesn't erode).
const NONE: u32 = u32::MAX;

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
    /// Each cell's shard, or [`NONE`].
    shard: Vec<u32>,
    /// Each shard's cells, its summed scorch, and whether it has broken off.
    cells: Vec<Vec<u32>>,
    heat: Vec<f32>,
    off: Vec<bool>,
    /// Per cell for the page: its shard's id plus one, little-endian in two bytes (0: no shard).
    pub shard_ids: Vec<u8>,
    /// Per shard for the page: its bounding box in cells, x0, y0, x1, y1 (inclusive).
    pub shard_boxes: Vec<u16>,
    /// Shards that broke off since the page last took them.
    pub broken: Vec<u32>,
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
            shard: Vec::new(),
            cells: Vec::new(),
            heat: Vec::new(),
            off: Vec::new(),
            shard_ids: Vec::new(),
            shard_boxes: Vec::new(),
            broken: Vec::new(),
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
        self.cut_shards();
    }

    /// Cuts the erodible cells into shards: each goes to the nearest of a grid of seeds, one per [`SHARD`]-cell square,
    /// each placed at a fixed pseudo-random spot in its square (so the same text always breaks the same way).
    fn cut_shards(&mut self) {
        let (w, h) = (self.w, self.h);
        let (gw, gh) = (w.div_ceil(SHARD), h.div_ceil(SHARD));
        let seed = |gx: usize, gy: usize| {
            let mut v = (gx as u32).wrapping_mul(0x9E37_79B9) ^ (gy as u32).wrapping_mul(0x85EB_CA6B) ^ 0x2545_F491;
            let mut next = || {
                v ^= v << 13;
                v ^= v >> 17;
                v ^= v << 5;
                0.15 + 0.7 * (v % 10_000) as f32 / 10_000.0
            };
            ((gx as f32 + next()) * SHARD as f32, (gy as f32 + next()) * SHARD as f32)
        };
        let mut id_of = vec![NONE; gw * gh];
        self.shard = vec![NONE; w * h];
        self.cells.clear();
        for i in (0..w * h).filter(|&i| self.original[i]) {
            let (x, y) = (i % w, i / w);
            let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
            let (gx0, gy0) = (x / SHARD, y / SHARD);
            let mut best = (f32::MAX, 0usize);
            for gy in gy0.saturating_sub(1)..(gy0 + 2).min(gh) {
                for gx in gx0.saturating_sub(1)..(gx0 + 2).min(gw) {
                    let (sx, sy) = seed(gx, gy);
                    let d = (sx - cx).powi(2) + (sy - cy).powi(2);
                    if d < best.0 {
                        best = (d, gy * gw + gx);
                    }
                }
            }
            if id_of[best.1] == NONE {
                id_of[best.1] = self.cells.len() as u32;
                self.cells.push(Vec::new());
            }
            let id = id_of[best.1];
            self.shard[i] = id;
            self.cells[id as usize].push(i as u32);
        }
        let n = self.cells.len();
        self.heat = vec![0.0; n];
        self.off = vec![false; n];
        self.broken.clear();
        self.shard_ids = vec![0; w * h * 2];
        for (i, &s) in self.shard.iter().enumerate().filter(|(_, s)| **s != NONE) {
            let tag = (s + 1) as u16;
            self.shard_ids[i * 2] = (tag & 0xFF) as u8;
            self.shard_ids[i * 2 + 1] = (tag >> 8) as u8;
        }
        self.shard_boxes = self
            .cells
            .iter()
            .flat_map(|cells| {
                let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
                for &c in cells {
                    let (x, y) = (c as usize % w, c as usize / w);
                    (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
                }
                [x0 as u16, y0 as u16, x1 as u16, y1 as u16]
            })
            .collect();
    }

    /// How many shards the text was cut into.
    pub fn shards(&self) -> usize {
        self.cells.len()
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
            let add = if dx == 0 && dy == 0 { amount } else { amount * 0.5 };
            self.scorch[i] += add;
            self.heat[self.shard[i] as usize] += add;
            self.hot.push(i as u32);
            self.touched.push(i as u32);
        }
    }

    /// Moves on `dt` seconds over `mask` (255 solid, 0 open): breaks off the shards scorched through (adding them to
    /// [`Erosion::broken`]), starts a re-weld once `reweld_at` of the text is gone, and carries one on. Returns the
    /// cells that came off this tick, and whether the mask changed (so the SDF must be rebuilt).
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
            let s = self.shard[i as usize] as usize;
            if self.off[s] || self.heat[s] < BREAK * self.cells[s].len() as f32 {
                continue;
            }
            // Through: the whole shard comes away.
            self.off[s] = true;
            self.broken.push(s as u32);
            for &j in &self.cells[s] {
                let j = j as usize;
                if mask[j] != 0 {
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
                self.heat.iter_mut().for_each(|h| *h = 0.0);
                self.off.iter_mut().for_each(|o| *o = false);
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

    /// A bigger block, 60 × 36 cells inside a 70 × 40 mask, so it's cut into many shards.
    fn slab() -> (Erosion, Vec<u8>) {
        let (w, h) = (70, 40);
        let mask: Vec<u8> = (0..w * h)
            .map(|i| if (5..65).contains(&(i % w)) && (2..38).contains(&(i / w)) { 255 } else { 0 })
            .collect();
        let mut e = Erosion::new();
        e.reset(&mask.iter().map(|m| *m > 127).collect::<Vec<_>>(), w, h);
        (e, mask)
    }

    #[test]
    fn the_text_is_cut_into_irregular_shards_about_the_shard_size_across() {
        let (e, mask) = slab();
        let cells = mask.iter().filter(|m| **m == 255).count();
        let mean = cells as f32 / e.shards() as f32;
        let square = (SHARD * SHARD) as f32;
        assert!(mean > square * 0.6 && mean < square * 1.4, "{} shards, {mean} cells each", e.shards());
        assert!(e.cells.iter().all(|c| !c.is_empty()));
        // Every erodible cell is in exactly one shard, and the page's id map says which.
        let tagged = e.shard_ids.chunks(2).filter(|t| t[0] != 0 || t[1] != 0).count();
        assert_eq!(tagged, cells);
        // Not a grid: shards' boxes differ in size.
        let sizes: std::collections::HashSet<_> = e.shard_boxes.chunks(4).map(|b| (b[2] - b[0], b[3] - b[1])).collect();
        assert!(sizes.len() > 3, "{sizes:?}");
    }

    #[test]
    fn a_shard_scorched_through_breaks_off_whole_and_its_neighbours_stand() {
        let (mut e, mut mask) = slab();
        let mut r = rng();
        let (x, y) = (30, 20);
        let s = e.shard[y * 70 + x] as usize;
        let mut n = 0;
        while !e.off[s] && n < 1000 {
            e.strike(x, y, 0.2);
            e.tick(0.016, &mut mask, 0.9, &mut r);
            n += 1;
        }
        assert!(e.off[s], "broke after {n} strikes");
        assert_eq!(e.broken, vec![s as u32], "reported once, for the page");
        for &c in &e.cells[s] {
            assert_eq!(mask[c as usize], 0, "every cell of it is gone");
            assert_eq!(e.out[c as usize * 4 + 1], 255, "and the renderer sees the hole");
        }
        let left = mask.iter().filter(|m| **m == 255).count();
        assert!(left + e.cells[s].len() == 60 * 36, "nothing else came off");
    }

    #[test]
    fn at_the_threshold_the_letters_re_weld_glowing_and_cool() {
        let (mut e, mut mask) = block();
        let mut r = rng();
        // Burn away more than 40% of it.
        for y in 2..8 {
            for x in 5..12 {
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
