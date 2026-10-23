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
//! fragment. Shards don't span letters: each letter (a run of text cells joined edge to edge) is cut on its own.
//!
//! The letters stand on their line (David: "edges of the letters show awkwardly even when there's nothing there to
//! support them ... More realistic physics"): the shards at a letter's foot hold up every shard joined to them, edge to
//! edge, and whatever a break leaves joined to no foot falls at once, each loose island as one piece ([`LOOSE`],
//! [`JOINED`]). The cells a letter's anti-aliased edge only grazes are part of it (the page marks them erodible too),
//! and each open cell around the text follows the text cell beside it, so a piece takes its whole outline with it.
//!
//! Once the given share of the text is gone, the letters re-weld: the lost cells come back in a random order over
//! [`REWELD`] seconds, glowing as they return, while all scorch cools away.

/// How long a re-weld takes, seconds.
pub const REWELD: f32 = 1.5;
/// How long a re-welded cell glows, seconds.
const GLOW: f32 = 0.6;
/// Shards' spacing, mask cells (the page's mask is half its CSS pixels: about 12 px).
pub const SHARD: usize = 6;
/// The mean scorch over a shard's cells at which it breaks off.
const BREAK: f32 = 0.5;
/// The bottom rows of a letter, which stand on its line: a shard with a cell there is a foot.
const FOOTING: usize = 2;
/// No shard (open air, or text that doesn't erode).
const NONE: u32 = u32::MAX;
/// Set on a [`Erosion::broken`] entry that fell for want of support, rather than being scorched through.
pub const LOOSE: u32 = 1 << 31;
/// Set on a [`Erosion::broken`] entry that falls as one piece with the entry before it.
pub const JOINED: u32 = 1 << 30;

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
    /// Each shard's neighbours, edge to edge within its letter, and whether it's a foot.
    joins: Vec<Vec<u32>>,
    foot: Vec<bool>,
    /// The open ring around the text: each such cell's text cell beside it, whose fate it shares ([`NONE`] for every
    /// other cell), and each shard's share of the ring.
    anchor: Vec<u32>,
    fringe: Vec<Vec<u32>>,
    /// Per cell for the page: its shard's id plus one, little-endian in two bytes (0: no shard).
    pub shard_ids: Vec<u8>,
    /// Per shard for the page: its bounding box in cells, x0, y0, x1, y1 (inclusive).
    pub shard_boxes: Vec<u16>,
    /// Shards that broke off since the page last took them: ids, perhaps with [`LOOSE`] and [`JOINED`] set.
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
            joins: Vec::new(),
            foot: Vec::new(),
            anchor: Vec::new(),
            fringe: Vec::new(),
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

    /// Starts over on the page's freshly committed mask of `w` × `h` cells (above 127 solid, exactly 255 text that may
    /// erode): nothing scorched or gone.
    pub fn reset(&mut self, mask: &[u8], w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.original = mask.iter().map(|&m| m == 255).collect();
        self.solid_total = self.original.iter().filter(|s| **s).count();
        self.scorch = vec![0.0; w * h];
        self.glow = vec![0.0; w * h];
        self.hot.clear();
        self.gone = 0;
        self.reweld = None;
        self.out = vec![0; w * h * 4];
        self.touched.clear();
        self.full = true;
        self.glowing = false;
        self.cut_shards(mask);
    }

    /// Cuts each letter's cells into shards: each goes to the nearest of a grid of seeds, one per [`SHARD`]-cell
    /// square, each placed at a fixed pseudo-random spot in its square (so the same text always breaks the same way).
    /// Then finds which shards join which, the feet, and the open ring around the text.
    fn cut_shards(&mut self, mask: &[u8]) {
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
        // The letters: runs of erodible cells joined edge to edge, and each one's bottom row.
        let mut letter = vec![NONE; w * h];
        let mut bottom = Vec::new();
        for start in 0..w * h {
            if !self.original[start] || letter[start] != NONE {
                continue;
            }
            let l = bottom.len() as u32;
            let (mut run, mut low) = (vec![start], 0);
            letter[start] = l;
            while let Some(i) = run.pop() {
                low = low.max(i / w);
                for j in self.beside(i) {
                    if self.original[j] && letter[j] == NONE {
                        letter[j] = l;
                        run.push(j);
                    }
                }
            }
            bottom.push(low);
        }
        let mut id_of: Vec<Vec<(u32, u32)>> = vec![Vec::new(); gw * gh];
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
            // A seed's region can reach into two letters: a shard each.
            let id = match id_of[best.1].iter().find(|(l, _)| *l == letter[i]) {
                Some(&(_, id)) => id,
                None => {
                    let id = self.cells.len() as u32;
                    id_of[best.1].push((letter[i], id));
                    self.cells.push(Vec::new());
                    id
                }
            };
            self.shard[i] = id;
            self.cells[id as usize].push(i as u32);
        }
        let n = self.cells.len();
        self.heat = vec![0.0; n];
        self.off = vec![false; n];
        self.broken.clear();
        self.joins = vec![Vec::new(); n];
        self.foot = vec![false; n];
        for i in (0..w * h).filter(|&i| self.original[i]) {
            let s = self.shard[i];
            self.foot[s as usize] |= i / w + FOOTING > bottom[letter[i] as usize];
            for j in self.beside(i) {
                if self.original[j] && self.shard[j] != s {
                    self.joins[s as usize].push(self.shard[j]);
                }
            }
        }
        for j in &mut self.joins {
            j.sort_unstable();
            j.dedup();
        }
        // The open ring: text drawn at the page's full resolution can stray a little past the mask's cells.
        self.anchor = vec![NONE; w * h];
        self.fringe = vec![Vec::new(); n];
        for i in (0..w * h).filter(|&i| !self.original[i] && mask[i] < 128) {
            let (x, y) = ((i % w) as i32, (i / w) as i32);
            let beside = (-1..=1)
                .flat_map(|dy| (-1..=1).map(move |dx| (x + dx, y + dy)))
                .filter(|&(cx, cy)| cx >= 0 && cy >= 0 && (cx as usize) < w && (cy as usize) < h)
                .map(|(cx, cy)| cy as usize * w + cx as usize)
                .find(|&j| self.original[j]);
            if let Some(j) = beside {
                self.anchor[i] = j as u32;
                self.shard[i] = self.shard[j];
                self.fringe[self.shard[j] as usize].push(i as u32);
            }
        }
        self.shard_ids = vec![0; w * h * 2];
        // The page draws a piece with its ring: both carry its id, and its box holds both.
        for (i, &s) in self.shard.iter().enumerate().filter(|(_, s)| **s != NONE) {
            let tag = (s + 1) as u16;
            self.shard_ids[i * 2] = (tag & 0xFF) as u8;
            self.shard_ids[i * 2 + 1] = (tag >> 8) as u8;
        }
        self.shard_boxes = self
            .cells
            .iter()
            .zip(&self.fringe)
            .flat_map(|(cells, ring)| {
                let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
                for &c in cells.iter().chain(ring) {
                    let (x, y) = (c as usize % w, c as usize / w);
                    (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
                }
                [x0 as u16, y0 as u16, x1 as u16, y1 as u16]
            })
            .collect();
    }

    /// The cells edge to edge with cell `i`.
    fn beside(&self, i: usize) -> impl Iterator<Item = usize> + use<> {
        let (w, h, x, y) = (self.w, self.h, i % self.w, i / self.w);
        [(x > 0).then(|| i - 1), (x + 1 < w).then(|| i + 1), (y > 0).then(|| i - w), (y + 1 < h).then(|| i + w)]
            .into_iter()
            .flatten()
    }

    /// Whether cell `i`'s text is gone from `mask`: its own, or for a cell of the open ring, its anchor's.
    fn gone(&self, mask: &[u8], i: usize) -> bool {
        if self.original[i] { mask[i] == 0 } else { self.anchor[i] != NONE && mask[self.anchor[i] as usize] == 0 }
    }

    /// Shard `s` comes away: its cells leave the mask, and it's listed (with `flags`) for the page to drop.
    fn break_off(&mut self, s: usize, flags: u32, mask: &mut [u8], flaked: &mut Vec<u32>) {
        self.off[s] = true;
        self.broken.push(s as u32 | flags);
        for &j in &self.cells[s] {
            let j = j as usize;
            if mask[j] != 0 {
                mask[j] = 0;
                self.gone += 1;
                flaked.push(j as u32);
                self.touched.push(j as u32);
            }
        }
        self.touched.extend_from_slice(&self.fringe[s]);
    }

    /// Drops every standing shard that no foot holds up through shards joined edge to edge: each loose island falls
    /// as one piece.
    fn collapse(&mut self, mask: &mut [u8], flaked: &mut Vec<u32>) {
        let n = self.cells.len();
        let mut held: Vec<bool> = (0..n).map(|s| self.foot[s] && !self.off[s]).collect();
        let mut reach: Vec<usize> = (0..n).filter(|&s| held[s]).collect();
        while let Some(s) = reach.pop() {
            for &t in &self.joins[s] {
                let t = t as usize;
                if !held[t] && !self.off[t] {
                    held[t] = true;
                    reach.push(t);
                }
            }
        }
        for s in 0..n {
            if held[s] || self.off[s] {
                continue;
            }
            held[s] = true;
            let mut island = vec![s];
            let mut k = 0;
            while k < island.len() {
                for &t in &self.joins[island[k]] {
                    let t = t as usize;
                    if !held[t] && !self.off[t] {
                        held[t] = true;
                        island.push(t);
                    }
                }
                k += 1;
            }
            for (k, &t) in island.iter().enumerate() {
                self.break_off(t, if k == 0 { LOOSE } else { LOOSE | JOINED }, mask, flaked);
            }
        }
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

    /// Moves on `dt` seconds over `mask` (255 solid, 0 open): breaks off the shards scorched through, then whatever
    /// that leaves unsupported (adding them to [`Erosion::broken`]), starts a re-weld once `reweld_at` of the text is
    /// gone, and carries one on. Returns the cells that came off this tick, and whether the mask changed (so the SDF
    /// must be rebuilt).
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
            self.break_off(s, 0, mask, &mut flaked);
            mask_changed = true;
        }
        if mask_changed {
            self.collapse(mask, &mut flaked);
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
        let pack = |e: &mut Self, i: usize| {
            let gone = e.gone(mask, i);
            e.out[i * 4] = (e.scorch[i].min(1.0) * 255.0) as u8;
            e.out[i * 4 + 1] = if gone { 255 } else { 0 };
            e.out[i * 4 + 2] = (e.glow[i] * 255.0) as u8;
        };
        if self.full {
            for i in 0..mask.len() {
                pack(self, i);
            }
        } else {
            // Most frames only a few cells were struck or flaked: repack just those.
            for k in 0..self.touched.len() {
                pack(self, self.touched[k] as usize);
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
        e.reset(&mask, w, h);
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
        e.reset(&mask, w, h);
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
        e.reset(&mask, w, h);
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
        // Every erodible cell is in exactly one shard, and the page's id map says which (and of the open ring round it).
        let tagged = e.shard_ids.chunks(2).filter(|t| t[0] != 0 || t[1] != 0).count();
        assert_eq!(tagged, cells + e.fringe.iter().map(Vec::len).sum::<usize>());
        assert_eq!(e.fringe.iter().map(Vec::len).sum::<usize>(), 2 * (60 + 36) + 4, "the ring is one cell wide");
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

    #[test]
    fn what_a_break_leaves_joined_to_no_foot_falls_as_one_piece() {
        let (mut e, mut mask) = slab();
        let mut r = rng();
        // Burn through a band across the whole slab, rows 18 and 19.
        for x in 5..65 {
            e.strike(x, 18, 100.0);
            e.strike(x, 19, 100.0);
        }
        e.tick(0.016, &mut mask, 0.99, &mut r);
        assert!((2..16).all(|y| (5..65).all(|x| mask[y * 70 + x] == 0)), "everything above the band fell");
        assert!((28..38).all(|y| (5..65).all(|x| mask[y * 70 + x] == 255)), "what stands on the line stays");
        let loose: Vec<u32> = e.broken.iter().copied().filter(|b| b & LOOSE != 0).collect();
        assert!(loose.len() > 5, "the upper part was {} shards", loose.len());
        assert_eq!(loose[0] & JOINED, 0, "an island starts a piece");
        assert!(loose[1..].iter().all(|b| b & JOINED != 0), "and the rest of it falls with that one: {loose:?}");
    }

    #[test]
    fn a_piece_takes_its_anti_aliased_edge_and_the_ring_beside_it() {
        let (w, h) = (20, 10);
        let mut mask: Vec<u8> =
            (0..w * h).map(|i| if (5..9).contains(&(i % w)) && (2..8).contains(&(i / w)) { 255 } else { 0 }).collect();
        mask[4 * w + 9] = 80; // open, but the glyph drawn at full resolution may stray into it
        let mut e = Erosion::new();
        e.reset(&mask, w, h);
        let id = |e: &Erosion, i: usize| e.shard_ids[i * 2] as u16 | (e.shard_ids[i * 2 + 1] as u16) << 8;
        assert_eq!(id(&e, 4 * w + 9), id(&e, 3 * w + 8), "the edge cell is drawn with the piece beside it");
        assert_eq!(id(&e, w + 4), id(&e, 2 * w + 5), "as is the ring at the letter's corner");
        assert_eq!(id(&e, 4 * w + 11), 0, "open air two cells out isn't");
        assert_eq!(e.out[(4 * w + 9) * 4 + 1], 0);
        e.strike(8, 3, 100.0);
        e.strike(5, 2, 100.0);
        e.tick(0.016, &mut mask, 0.99, &mut rng());
        assert_eq!(e.out[(4 * w + 9) * 4 + 1], 255, "and goes with it");
        assert_eq!(e.out[(w + 4) * 4 + 1], 255, "as does the ring at its corner");
    }

    #[test]
    fn no_shard_spans_two_letters() {
        let (w, h) = (40, 12);
        // Two letters a cell apart: the seeds' squares straddle the gap.
        let mask: Vec<u8> = (0..w * h)
            .map(|i| {
                if (2..10).contains(&(i / w)) && ((4..17).contains(&(i % w)) || (18..30).contains(&(i % w))) {
                    255
                } else {
                    0
                }
            })
            .collect();
        let mut e = Erosion::new();
        e.reset(&mask, w, h);
        for cells in &e.cells {
            let left = cells.iter().filter(|&&c| (c as usize % w) < 17).count();
            assert!(left == 0 || left == cells.len(), "a shard on both sides of the gap");
        }
        assert!(e.foot.iter().filter(|f| **f).count() >= 2, "each letter has its own feet");
    }
}
