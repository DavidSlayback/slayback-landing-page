//! Spike: 2D particle physics for the landing-page hero: granular grains or short-lived welding sparks.
//!
//! Sparks come from wandering emitters ("torches") that relocate at random, ride a divergence-free current
//! field, ricochet off text and a cursor forcefield, and cool over their lifetime.
//!
//! Position-based dynamics per substep: apply forces and predict positions, bin grains into a uniform grid
//! (counting sort), resolve contacts (grain–grain, text SDF, cursor disc, arena walls), then derive velocity
//! from the corrected positions. Units are CSS pixels and seconds; +y points down.
//! Exposed to JavaScript through a plain C ABI and raw pointers into WASM memory (no wasm-bindgen).

// The C-ABI functions take raw pointers from JS by design (see `world`); they are the FFI boundary.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::f32::consts::TAU;

#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub gravity: f32,        // px/s²
    pub wind: f32,           // px/s² toward +x, gusting
    pub turb: f32,           // px/s typical speed of the swirling currents
    pub drag: f32,           // 1/s coupling of grains to the current field
    pub radius: f32,         // grain radius, px
    pub friction: f32,       // tangential velocity removed per text/wall contact (0..1)
    pub floor_friction: f32, // same, for the floor (low, so wind can drive dunes)
    pub restitution: f32,
    pub shield_restitution: f32, // bounce off the cursor disc and forcefield
    pub cursor_r: f32, // cursor disc radius, px
    pub spawn_rate: f32, // grains activated per second until capacity
    pub source: u32,     // 0 = left edge, 1 = top edge (used when there are no emitters)
    pub max_speed: f32,
    pub collide: bool,    // grain-grain contacts (sparks pass through each other)
    pub life_min: f32,    // seconds; life_max = 0 means grains never expire
    pub life_max: f32,
    pub open_edges: bool, // top, left and right are exits; only the floor is solid
    pub emitters: u32,    // wandering spark sources; 0 = use `source`
    pub launch_min: f32,  // emitter launch speed range, px/s
    pub launch_max: f32,
    pub open_floor: bool, // the floor is an exit (and an emitter edge) too
    pub wander: f32,      // px/s² per-particle random steering (each particle's heading drifts on its own)
    pub flow_scale: f32,  // wavelength of the current field, px (smaller = tighter, more circuitous swirls)
    pub inward: f32,      // px/s² pull toward the screen centre, fading over a particle's first ~2 s
}

impl Default for Params {
    fn default() -> Self {
        Params {
            gravity: 320.0,
            wind: 240.0,
            turb: 130.0,
            drag: 1.6,
            radius: 2.6,
            friction: 0.3,
            floor_friction: 0.04,
            restitution: 0.15,
            shield_restitution: 0.8,
            cursor_r: 22.0,
            spawn_rate: 2500.0,
            source: 0,
            max_speed: 1600.0,
            collide: true,
            life_min: 0.0,
            life_max: 0.0,
            open_edges: false,
            emitters: 0,
            launch_min: 400.0,
            launch_max: 1000.0,
            open_floor: false,
            wander: 0.0,
            flow_scale: 360.0,
            inward: 0.0,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Contact {
    None,
    Solid,
    Floor,
    Bouncy, // cursor disc and forcefield
}

/// A spark source that sprays a wobbling cone for a few seconds, then reappears somewhere else.
#[derive(Clone, Copy, Debug, Default)]
pub struct Emitter {
    pub x: f32,
    pub y: f32,
    pub angle: f32,
    pub spread: f32,
    pub age: f32,
    pub ttl: f32,
    pub phase: f32,
    pub hue: f32,
}

impl Emitter {
    /// 0..1 envelope: fades in and out at the ends of its life, with a torch-like flicker.
    pub fn power(&self) -> f32 {
        let env = (self.age / 0.3).min((self.ttl - self.age) / 0.3).clamp(0.0, 1.0);
        env * (0.7 + 0.3 * (self.age * 23.0 + self.phase).sin())
    }
}

pub struct World {
    pub w: f32,
    pub h: f32,
    pub cap: usize,
    pub active: usize,
    pub params: Params,
    pub time: f32,
    pub x: Vec<f32>,
    pub y: Vec<f32>,
    pub vx: Vec<f32>,
    pub vy: Vec<f32>,
    pub energy: Vec<f32>, // 0..1 impact glow
    pub hue: Vec<f32>,
    pub age: Vec<f32>,
    pub life: Vec<f32>,
    heading: Vec<f32>, // wander steering angle, drifts as a random walk
    pub emitters: Vec<Emitter>,
    emitter_out: Vec<f32>,
    shield: [f32; 3], // x, y, radius (0 = off)
    pub shield_hits: u32,
    px: Vec<f32>,
    py: Vec<f32>,
    cnx: Vec<f32>,
    cny: Vec<f32>,
    contact: Vec<Contact>,
    // uniform grid
    cell: f32,
    gw: usize,
    gh: usize,
    cell_start: Vec<u32>,
    fill: Vec<u32>,
    sorted: Vec<u32>,
    pcell: Vec<u32>,
    scratch: Vec<f32>,
    // text obstacles: mask (written by JS) → signed distance field in CSS px (+ outside, − inside)
    pub mask: Vec<u8>,
    pub mw: usize,
    pub mh: usize,
    pub mscale: f32, // mask pixels per CSS px
    pub sdf: Vec<f32>,
    // divergence-free current field on a coarse grid
    fcell: f32,
    fw: usize,
    fh: usize,
    psi: Vec<f32>,
    fu: Vec<f32>,
    fv: Vec<f32>,
    // input
    cursor: [f32; 4], // x, y, previous x, previous y
    cursor_on: bool,
    cursor_was_on: bool,
    bursts: Vec<[f32; 4]>, // x, y, radius, strength
    spawn_acc: f32,
    rng: u32,
    pub out: Vec<f32>, // interleaved x, y, vx, vy, energy, hue, heat (1 -> 0 over life), 0 per active particle
}

pub const OUT_STRIDE: usize = 8;
/// Emitters sit this far outside the screen; particles are only recycled once they are further out than this.
pub const EMITTER_OFFSET: f32 = 50.0;
const EXIT_MARGIN: f32 = 90.0;

impl World {
    pub fn new(cap: usize, w: f32, h: f32, seed: u32) -> World {
        let fcell = 24.0;
        let fw = (w / fcell).ceil() as usize + 2;
        let fh = (h / fcell).ceil() as usize + 2;
        let mut world = World {
            w,
            h,
            cap,
            active: 0,
            params: Params::default(),
            time: 0.0,
            x: vec![0.0; cap],
            y: vec![0.0; cap],
            vx: vec![0.0; cap],
            vy: vec![0.0; cap],
            energy: vec![0.0; cap],
            hue: vec![0.0; cap],
            age: vec![0.0; cap],
            life: vec![0.0; cap],
            heading: vec![0.0; cap],
            emitters: Vec::new(),
            emitter_out: Vec::new(),
            shield: [0.0; 3],
            shield_hits: 0,
            px: vec![0.0; cap],
            py: vec![0.0; cap],
            cnx: vec![0.0; cap],
            cny: vec![0.0; cap],
            contact: vec![Contact::None; cap],
            cell: 1.0,
            gw: 1,
            gh: 1,
            cell_start: Vec::new(),
            fill: Vec::new(),
            sorted: vec![0; cap],
            pcell: vec![0; cap],
            scratch: vec![0.0; cap],
            mask: Vec::new(),
            mw: 0,
            mh: 0,
            mscale: 1.0,
            sdf: Vec::new(),
            fcell,
            fw,
            fh,
            psi: vec![0.0; fw * fh],
            fu: vec![0.0; fw * fh],
            fv: vec![0.0; fw * fh],
            cursor: [0.0; 4],
            cursor_on: false,
            cursor_was_on: false,
            bursts: Vec::new(),
            spawn_acc: 0.0,
            rng: seed.max(1),
            out: vec![0.0; cap * OUT_STRIDE],
        };
        world.resize_grid();
        world
    }

    pub fn set_params(&mut self, p: Params) {
        let regrid = p.radius != self.params.radius;
        self.params = p;
        if regrid {
            self.resize_grid();
        }
        let n = p.emitters as usize;
        self.emitters.truncate(n);
        while self.emitters.len() < n {
            let mut e = Emitter { hue: self.emitters.len() as f32 * 0.25 + 0.1, ..Emitter::default() };
            self.relocate(&mut e);
            e.age = self.rand() * e.ttl * 0.8; // staggered: torches don't all jump at once
            self.emitters.push(e);
        }
    }

    /// Sets one parameter by key (the JS tuning surface). Unknown keys are ignored.
    pub fn set(&mut self, key: u32, v: f32) {
        let mut p = self.params;
        match key {
            0 => p.gravity = v,
            1 => p.wind = v,
            2 => p.turb = v,
            3 => p.drag = v,
            4 => p.radius = v,
            5 => p.friction = v,
            6 => p.floor_friction = v,
            7 => p.restitution = v,
            8 => p.shield_restitution = v,
            9 => p.cursor_r = v,
            10 => p.spawn_rate = v,
            11 => p.source = v as u32,
            12 => p.collide = v != 0.0,
            13 => p.life_min = v,
            14 => p.life_max = v,
            15 => p.open_edges = v != 0.0,
            16 => p.emitters = v as u32,
            17 => p.launch_min = v,
            18 => p.launch_max = v,
            19 => p.open_floor = v != 0.0,
            20 => p.wander = v,
            21 => p.flow_scale = v.max(40.0),
            22 => p.inward = v,
            _ => return,
        }
        self.set_params(p);
    }

    /// Edges sparks may enter through: top, left and right when open, the floor only when it is open too.
    fn open_sides(&self) -> Vec<u32> {
        let mut sides = Vec::new();
        if self.params.open_edges {
            sides.extend([0, 2, 3]);
        }
        if self.params.open_floor {
            sides.push(1);
        }
        if sides.is_empty() {
            sides.push(0);
        }
        sides
    }

    /// Moves an emitter just outside a random open edge of the screen, aimed inward within a random cone.
    fn relocate(&mut self, e: &mut Emitter) {
        let (w, h) = (self.w, self.h);
        let sides = self.open_sides();
        let side = sides[(self.rand() * sides.len() as f32) as usize % sides.len()];
        let t = self.rand();
        let (x, y, inward) = match side {
            0 => (w * t, -EMITTER_OFFSET, TAU * 0.25),        // above the top edge, aiming down
            1 => (w * t, h + EMITTER_OFFSET, -TAU * 0.25),    // below the floor, aiming up
            2 => (-EMITTER_OFFSET, h * t, 0.0),               // left of the screen, aiming right
            _ => (w + EMITTER_OFFSET, h * t, TAU * 0.5),      // right of the screen, aiming left
        };
        (e.x, e.y) = (x, y);
        e.angle = inward + (self.rand() - 0.5) * 2.2; // up to ~63 degrees off the inward normal
        e.spread = 0.3 + 0.7 * self.rand();
        e.age = 0.0;
        e.ttl = 2.5 + 4.0 * self.rand();
        e.phase = self.rand() * TAU;
    }

    fn update_emitters(&mut self, dt: f32) {
        let mut list = std::mem::take(&mut self.emitters);
        for e in list.iter_mut() {
            e.age += dt;
            if e.age >= e.ttl {
                self.relocate(e);
                e.hue = (e.hue + 0.37).fract(); // a new torch gets a new colour
            }
        }
        self.emitters = list;
    }

    fn resize_grid(&mut self) {
        self.cell = 2.0 * self.params.radius;
        self.gw = (self.w / self.cell).ceil() as usize + 1;
        self.gh = (self.h / self.cell).ceil() as usize + 1;
        self.cell_start = vec![0; self.gw * self.gh + 1];
        self.fill = vec![0; self.gw * self.gh];
    }

    fn rand(&mut self) -> f32 {
        let mut s = self.rng;
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        self.rng = s;
        (s >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Adds one grain directly (tests and scripted scenes); its hue holds its insertion order as a stable tag,
    /// since grains are reordered by grid cell every substep. Returns false when full.
    pub fn put(&mut self, x: f32, y: f32, vx: f32, vy: f32) -> bool {
        if self.active >= self.cap {
            return false;
        }
        let i = self.active;
        (self.x[i], self.y[i], self.vx[i], self.vy[i]) = (x, y, vx, vy);
        self.energy[i] = 0.0;
        self.hue[i] = i as f32;
        self.age[i] = 0.0;
        self.life[i] = 0.0;
        self.active += 1;
        true
    }

    fn spawn(&mut self, i: usize) {
        let r = self.params.radius;
        let (a, b, c, d) = (self.rand(), self.rand(), self.rand(), self.rand());
        self.age[i] = 0.0;
        let p = self.params;
        self.life[i] = if p.life_max > 0.0 { p.life_min + (p.life_max - p.life_min) * self.rand() } else { 0.0 };
        if !self.emitters.is_empty() {
            // pick a torch, skipping dim ones; spray inside its wobbling cone
            let n = self.emitters.len();
            let mut k = (a * n as f32) as usize % n;
            if self.emitters[k].power() < 0.15 {
                k = (k + 1) % n;
            }
            let e = self.emitters[k];
            let wobble = 0.35 * (self.time * 1.7 + e.phase).sin() + 0.2 * (self.time * 4.3 + e.phase * 2.0).sin();
            let ang = e.angle + wobble + (b - 0.5) * e.spread;
            let sp = p.launch_min + (p.launch_max - p.launch_min) * c.powf(0.7);
            self.x[i] = e.x + ang.cos() * 3.0;
            self.y[i] = e.y + ang.sin() * 3.0;
            self.vx[i] = ang.cos() * sp;
            self.vy[i] = ang.sin() * sp;
            self.energy[i] = 0.0; // heat (age) carries the white-hot start; energy is reserved for impacts
            self.heading[i] = self.rand() * TAU;
            self.hue[i] = (e.hue + d * 0.08).fract();
            return;
        }
        if self.params.source == 1 {
            self.x[i] = self.w * (0.04 + 0.92 * a);
            self.y[i] = r + 1.0 + 6.0 * b;
            self.vx[i] = (c - 0.5) * 40.0;
            self.vy[i] = 60.0 + 140.0 * d;
        } else {
            self.x[i] = r + 1.0 + 10.0 * a;
            self.y[i] = self.h * (0.04 + 0.6 * b);
            self.vx[i] = 180.0 + 240.0 * c;
            self.vy[i] = (d - 0.5) * 60.0;
        }
        self.energy[i] = 0.0;
        self.hue[i] = (self.time * 0.045 + self.rand() * 0.1).fract();
    }

    // ---------- obstacles ----------

    /// Allocates the obstacle mask (JS fills it, 255 = solid) and returns a pointer to it.
    pub fn mask_alloc(&mut self, mw: usize, mh: usize, scale: f32) -> *mut u8 {
        self.mw = mw;
        self.mh = mh;
        self.mscale = scale;
        self.mask = vec![0; mw * mh];
        self.mask.as_mut_ptr()
    }

    /// Rebuilds the signed distance field from the mask (two-pass 8-neighbour chamfer each way).
    pub fn mask_commit(&mut self) {
        let (w, h) = (self.mw, self.mh);
        let solid: Vec<bool> = self.mask.iter().map(|&m| m > 127).collect();
        let d_out = chamfer(&solid, w, h, true);
        let d_in = chamfer(&solid, w, h, false);
        self.sdf = d_out
            .iter()
            .zip(&d_in)
            .map(|(o, i)| ((o - i) / self.mscale).min(60000.0))
            .collect();
    }

    pub fn sdf_at(&self, x: f32, y: f32) -> f32 {
        if self.mw < 2 || self.sdf.len() != self.mw * self.mh {
            return 1e6;
        }
        let sx = x * self.mscale - 0.5;
        let sy = y * self.mscale - 0.5;
        if sx < 0.0 || sy < 0.0 || sx >= (self.mw - 1) as f32 || sy >= (self.mh - 1) as f32 {
            return 1e6;
        }
        let (ix, iy) = (sx as usize, sy as usize);
        let (fx, fy) = (sx - ix as f32, sy - iy as f32);
        let i = iy * self.mw + ix;
        let s = &self.sdf;
        let top = s[i] + (s[i + 1] - s[i]) * fx;
        let bot = s[i + self.mw] + (s[i + self.mw + 1] - s[i + self.mw]) * fx;
        top + (bot - top) * fy
    }

    fn sdf_normal(&self, x: f32, y: f32) -> (f32, f32) {
        let e = 1.0 / self.mscale;
        let gx = self.sdf_at(x + e, y) - self.sdf_at(x - e, y);
        let gy = self.sdf_at(x, y + e) - self.sdf_at(x, y - e);
        let len = (gx * gx + gy * gy).sqrt();
        if len < 1e-6 { (0.0, -1.0) } else { (gx / len, gy / len) }
    }

    // ---------- input ----------

    pub fn set_cursor(&mut self, x: f32, y: f32, on: bool) {
        if on && !self.cursor_on {
            self.cursor[2] = x; // entering: no sweep from a stale position
            self.cursor[3] = y;
        }
        self.cursor[0] = x;
        self.cursor[1] = y;
        self.cursor_on = on;
    }

    /// Forcefield bubble around (x, y); radius 0 switches it off.
    pub fn set_shield(&mut self, x: f32, y: f32, r: f32) {
        self.shield = [x, y, r.max(0.0)];
    }

    /// Queues a radial impulse that only reaches grains within `radius`.
    pub fn burst(&mut self, x: f32, y: f32, radius: f32, strength: f32) {
        self.bursts.push([x, y, radius, strength]);
    }

    // ---------- currents ----------

    fn update_flow(&mut self) {
        // Stream function = ramp(sdf) × sum of travelling sine waves; its curl is divergence-free and,
        // because the ramp is zero at glyph edges, tangent to the text (Bridson's curl-noise boundary trick).
        const TERMS: [[f32; 5]; 6] = [
            [0.80, 0.60, 1.00, 0.35, 0.0],
            [-0.50, 0.87, 1.37, -0.27, 1.7],
            [0.17, -0.98, 0.71, 0.21, 4.1],
            [-0.92, -0.40, 1.90, 0.45, 2.6],
            [0.34, 0.94, 2.60, -0.52, 5.3],
            [-0.71, 0.71, 3.30, 0.61, 0.9],
        ];
        let k = TAU / self.params.flow_scale;
        let amp = self.params.turb / k * 0.5;
        let infl = 36.0;
        let t = self.time;
        for j in 0..self.fh {
            for i in 0..self.fw {
                let (x, y) = (i as f32 * self.fcell, j as f32 * self.fcell);
                let mut n = 0.0;
                for [dx, dy, km, om, ph] in TERMS {
                    n += (k * km * (dx * x + dy * y) + om * t + ph).sin() / km;
                }
                self.psi[j * self.fw + i] = ramp(self.sdf_at(x, y) / infl) * amp * n;
            }
        }
        let inv = 1.0 / (2.0 * self.fcell);
        for j in 0..self.fh {
            for i in 0..self.fw {
                let at = |ii: usize, jj: usize| self.psi[jj.min(self.fh - 1) * self.fw + ii.min(self.fw - 1)];
                let u = (at(i, j + 1) - at(i, j.saturating_sub(1))) * inv;
                let v = -(at(i + 1, j) - at(i.saturating_sub(1), j)) * inv;
                self.fu[j * self.fw + i] = u;
                self.fv[j * self.fw + i] = v;
            }
        }
    }

    fn flow_at(&self, x: f32, y: f32) -> (f32, f32) {
        let sx = (x / self.fcell).clamp(0.0, (self.fw - 2) as f32);
        let sy = (y / self.fcell).clamp(0.0, (self.fh - 2) as f32);
        let (ix, iy) = (sx as usize, sy as usize);
        let (fx, fy) = (sx - ix as f32, sy - iy as f32);
        let i = iy * self.fw + ix;
        let lerp2 = |f: &Vec<f32>| {
            let top = f[i] + (f[i + 1] - f[i]) * fx;
            let bot = f[i + self.fw] + (f[i + self.fw + 1] - f[i + self.fw]) * fx;
            top + (bot - top) * fy
        };
        (lerp2(&self.fu), lerp2(&self.fv))
    }

    // ---------- grid ----------

    fn cell_of(&self, x: f32, y: f32) -> usize {
        let cx = ((x / self.cell).max(0.0) as usize).min(self.gw - 1);
        let cy = ((y / self.cell).max(0.0) as usize).min(self.gh - 1);
        cy * self.gw + cx
    }

    fn build_grid(&mut self) {
        let cells = self.gw * self.gh;
        self.cell_start.iter_mut().for_each(|c| *c = 0);
        for i in 0..self.active {
            let c = self.cell_of(self.px[i], self.py[i]);
            self.pcell[i] = c as u32;
            self.cell_start[c + 1] += 1;
        }
        for c in 0..cells {
            self.cell_start[c + 1] += self.cell_start[c];
        }
        self.fill.copy_from_slice(&self.cell_start[..cells]);
        for i in 0..self.active {
            let c = self.pcell[i] as usize;
            self.sorted[self.fill[c] as usize] = i as u32;
            self.fill[c] += 1;
        }
        if !self.params.collide {
            return; // only grain-grain contacts benefit from the reorder; `sorted` maps cells to particles as is
        }
        // Reorder grains so each cell's members are contiguous: contact loops then stream through memory.
        let n = self.active;
        for arr in [&mut self.x, &mut self.y, &mut self.vx, &mut self.vy, &mut self.px, &mut self.py, &mut self.energy, &mut self.hue, &mut self.age, &mut self.life, &mut self.heading] {
            for k in 0..n {
                self.scratch[k] = arr[self.sorted[k] as usize];
            }
            arr[..n].copy_from_slice(&self.scratch[..n]);
        }
        for k in 0..n {
            self.sorted[k] = k as u32;
        }
    }

    /// Calls `f(j)` for every grain whose cell lies within the box around (x, y) ± reach.
    fn for_near(&self, x: f32, y: f32, reach: f32, mut f: impl FnMut(usize)) {
        let x0 = ((x - reach) / self.cell).max(0.0) as usize;
        let y0 = ((y - reach) / self.cell).max(0.0) as usize;
        let x1 = (((x + reach) / self.cell).max(0.0) as usize).min(self.gw - 1);
        let y1 = (((y + reach) / self.cell).max(0.0) as usize).min(self.gh - 1);
        for cy in y0..=y1 {
            for cx in x0..=x1 {
                let c = cy * self.gw + cx;
                for k in self.cell_start[c]..self.cell_start[c + 1] {
                    f(self.sorted[k as usize] as usize);
                }
            }
        }
    }

    fn solve_pairs(&mut self) {
        // Half stencil over contiguous cell ranges: each pair is visited exactly once.
        let d0 = 2.0 * self.params.radius;
        let (gw, gh) = (self.gw, self.gh);
        let cs = &self.cell_start;
        let (px, py) = (&mut self.px, &mut self.py);
        for cy in 0..gh {
            for cx in 0..gw {
                let c = cy * gw + cx;
                let (a0, a1) = (cs[c] as usize, cs[c + 1] as usize);
                if a0 == a1 {
                    continue;
                }
                for i in a0..a1 {
                    for j in i + 1..a1 {
                        contact(px, py, i, j, d0);
                    }
                }
                for (dx, dy) in [(1isize, 0isize), (-1, 1), (0, 1), (1, 1)] {
                    let (nx, ny) = (cx as isize + dx, cy as isize + dy);
                    if nx < 0 || nx >= gw as isize || ny >= gh as isize {
                        continue;
                    }
                    let nc = ny as usize * gw + nx as usize;
                    let (b0, b1) = (cs[nc] as usize, cs[nc + 1] as usize);
                    for i in a0..a1 {
                        for j in b0..b1 {
                            contact(px, py, i, j, d0);
                        }
                    }
                }
            }
        }
    }

    fn solve_solids(&mut self, cursor: Option<(f32, f32)>) {
        let r = self.params.radius;
        for i in 0..self.active {
            // text
            let d = self.sdf_at(self.px[i], self.py[i]);
            if d < r {
                let (nx, ny) = self.sdf_normal(self.px[i], self.py[i]);
                self.px[i] += nx * (r - d);
                self.py[i] += ny * (r - d);
                (self.cnx[i], self.cny[i], self.contact[i]) = (nx, ny, Contact::Solid);
            }
            // arena: floor, ceiling, left wall (right edge is the exit)
            if self.py[i] > self.h - r && !self.params.open_floor {
                self.py[i] = self.h - r;
                (self.cnx[i], self.cny[i], self.contact[i]) = (0.0, -1.0, Contact::Floor);
            } else if self.py[i] < r && !self.params.open_edges {
                self.py[i] = r;
                (self.cnx[i], self.cny[i], self.contact[i]) = (0.0, 1.0, Contact::Solid);
            }
            if self.px[i] < r && !self.params.open_edges {
                self.px[i] = r;
                (self.cnx[i], self.cny[i], self.contact[i]) = (1.0, 0.0, Contact::Solid);
            }
        }
        // cursor disc and forcefield: kinematic circles; their motion shoves particles and hands over momentum
        if let Some((cx, cy)) = cursor {
            self.push_out_of_circle(cx, cy, self.params.cursor_r + r);
        }
        if self.shield[2] > 0.0 {
            let [sx, sy, sr] = self.shield;
            self.shield_hits += self.push_out_of_circle(sx, sy, sr + r);
        }
    }

    /// Pushes every particle inside the circle to its rim; returns how many it moved.
    fn push_out_of_circle(&mut self, cx: f32, cy: f32, reach: f32) -> u32 {
        let mut hits = Vec::new();
        self.for_near(cx, cy, reach, |j| hits.push(j));
        let mut n = 0;
        for j in hits {
            let dx = self.px[j] - cx;
            let dy = self.py[j] - cy;
            let d2 = dx * dx + dy * dy;
            if d2 < reach * reach {
                let d = d2.sqrt().max(1e-4);
                let (nx, ny) = (dx / d, dy / d);
                self.px[j] = cx + nx * reach;
                self.py[j] = cy + ny * reach;
                (self.cnx[j], self.cny[j], self.contact[j]) = (nx, ny, Contact::Bouncy);
                n += 1;
            }
        }
        n
    }

    fn apply_bursts(&mut self, h: f32) {
        let bursts = std::mem::take(&mut self.bursts);
        for [bx, by, radius, strength] in bursts {
            let mut hits = Vec::new();
            self.for_near(bx, by, radius, |j| hits.push(j));
            for j in hits {
                let dx = self.px[j] - bx;
                let dy = self.py[j] - by;
                let d = (dx * dx + dy * dy).sqrt();
                if d >= radius {
                    continue;
                }
                let fall = (1.0 - d / radius).powi(2);
                let (nx, ny) = if d > 1e-3 { (dx / d, dy / d) } else { (0.0, -1.0) };
                self.vx[j] += nx * strength * fall;
                self.vy[j] += ny * strength * fall;
                self.px[j] += nx * strength * fall * h;
                self.py[j] += ny * strength * fall * h;
                self.energy[j] = self.energy[j].max(fall);
            }
        }
    }

    fn finalize(&mut self, h: f32) {
        let p = self.params;
        let decay = (-h * 2.5).exp();
        for i in 0..self.active {
            let mut nvx = (self.px[i] - self.x[i]) / h;
            let mut nvy = (self.py[i] - self.y[i]) / h;
            if self.contact[i] != Contact::None {
                let (nx, ny) = (self.cnx[i], self.cny[i]);
                let vn = nvx * nx + nvy * ny;
                let (tx, ty) = (nvx - vn * nx, nvy - vn * ny);
                let (fr, rest) = match self.contact[i] {
                    Contact::Floor => (p.floor_friction, p.restitution),
                    Contact::Bouncy => (0.0, p.shield_restitution),
                    _ => (p.friction, p.restitution),
                };
                // Bounce: the position correction already removed the incoming normal speed, so restore a
                // `rest` fraction of what the particle carried into the contact (pre-correction velocity).
                let vin = self.vx[i] * nx + self.vy[i] * ny;
                let vn = if vin < 0.0 { vn.max(-vin * rest) } else { vn };
                nvx = tx * (1.0 - fr) + vn * nx;
                nvy = ty * (1.0 - fr) + vn * ny;
            }
            let sp = (nvx * nvx + nvy * nvy).sqrt();
            if sp > p.max_speed {
                nvx *= p.max_speed / sp;
                nvy *= p.max_speed / sp;
            }
            // impact glow: velocity change caused by contacts this substep
            let dv = ((nvx - self.vx[i]).powi(2) + (nvy - self.vy[i]).powi(2)).sqrt();
            let impact = ((dv - 120.0) / 900.0).clamp(0.0, 1.0);
            self.energy[i] = (self.energy[i] * decay).max(impact);
            (self.x[i], self.y[i], self.vx[i], self.vy[i]) = (self.px[i], self.py[i], nvx, nvy);
            self.contact[i] = Contact::None;
            self.age[i] += h;
            let expired = self.life[i] > 0.0 && self.age[i] > self.life[i];
            let out = if p.open_edges {
                self.x[i] < -EXIT_MARGIN || self.x[i] > self.w + EXIT_MARGIN || self.y[i] < -EXIT_MARGIN || self.y[i] > self.h + EXIT_MARGIN
            } else {
                self.x[i] > self.w + 12.0 || self.y[i] > self.h + 40.0
            };
            if expired || out {
                self.spawn(i); // burned out or left the arena: relaunch from a source
            }
        }
    }

    /// Runs `n` frames and returns average milliseconds per phase (native profiling only).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn profile(&mut self, n: usize) -> Vec<(&'static str, f64)> {
        use std::time::Instant;
        let mut acc = [0.0f64; 5];
        let h = 1.0 / 180.0;
        for _ in 0..n {
            let t = Instant::now(); self.update_flow(); acc[0] += t.elapsed().as_secs_f64();
            for _ in 0..3 {
                let t = Instant::now();
                for i in 0..self.active {
                    let (u, v) = self.flow_at(self.x[i], self.y[i]);
                    self.vx[i] += (self.params.wind + self.params.drag * (u - self.vx[i])) * h;
                    self.vy[i] += (self.params.gravity + self.params.drag * (v - self.vy[i])) * h;
                    self.px[i] = self.x[i] + self.vx[i] * h;
                    self.py[i] = self.y[i] + self.vy[i] * h;
                }
                acc[1] += t.elapsed().as_secs_f64();
                let t = Instant::now(); self.build_grid(); acc[2] += t.elapsed().as_secs_f64();
                let t = Instant::now(); self.solve_pairs(); self.solve_pairs(); acc[3] += t.elapsed().as_secs_f64();
                let t = Instant::now(); self.solve_solids(None); self.finalize(h); acc[4] += t.elapsed().as_secs_f64();
            }
        }
        ["flow", "predict", "grid", "pairs", "solids"].into_iter().zip(acc.map(|a| a * 1e3 / n as f64)).collect()
    }

    pub fn step(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 1.0 / 30.0);
        if dt <= 0.0 {
            return;
        }
        self.spawn_acc += self.params.spawn_rate * dt;
        while self.spawn_acc >= 1.0 && self.active < self.cap {
            let i = self.active;
            self.spawn(i);
            self.active += 1;
            self.spawn_acc -= 1.0;
        }
        if self.active >= self.cap {
            self.spawn_acc = 0.0;
        }
        self.shield_hits = 0;
        self.update_emitters(dt);
        self.update_flow();
        // Fixed-size substeps (~1/180 s) whatever the frame rate: a longer step lets fast grains travel a full
        // diameter per substep, land deep inside their neighbours, and get flung out by the overlap correction.
        let sub = ((dt * 180.0).ceil() as usize).clamp(1, 8);
        let h = dt / sub as f32;
        let p = self.params;
        let t = self.time;
        let gust = 0.6 + 0.4 * (t * 0.37).sin() + 0.25 * (t * 1.31 + 1.0).sin();
        for s in 0..sub {
            let a = (s + 1) as f32 / sub as f32;
            let cursor = self.cursor_on.then(|| {
                (self.cursor[2] + (self.cursor[0] - self.cursor[2]) * a, self.cursor[3] + (self.cursor[1] - self.cursor[3]) * a)
            });
            let steer = 9.0 * h.sqrt(); // heading random walk: uniform steps, ~2.6 rad standard deviation per sqrt(second)
            for i in 0..self.active {
                let (u, v) = self.flow_at(self.x[i], self.y[i]);
                let (wx, wy) = if p.wander > 0.0 {
                    self.heading[i] += (self.rand() - 0.5) * steer;
                    (self.heading[i].cos() * p.wander, self.heading[i].sin() * p.wander)
                } else {
                    (0.0, 0.0)
                };
                let (ix, iy) = if p.inward > 0.0 {
                    // carries newcomers from the off-screen torches into the arena before the currents take over
                    let (dx, dy) = (self.w * 0.5 - self.x[i], self.h * 0.5 - self.y[i]);
                    let k = p.inward * (1.0 - self.age[i] / 2.0).max(0.0) / (dx * dx + dy * dy).sqrt().max(1.0);
                    (dx * k, dy * k)
                } else {
                    (0.0, 0.0)
                };
                self.vx[i] += (p.wind * gust + wx + ix + p.drag * (u - self.vx[i])) * h;
                self.vy[i] += (p.gravity + wy + iy + p.drag * (v - self.vy[i])) * h;
                self.px[i] = self.x[i] + self.vx[i] * h;
                self.py[i] = self.y[i] + self.vy[i] * h;
            }
            // The grid serves contacts and the cursor/forcefield/burst queries; sparks alone don't need it.
            if p.collide || cursor.is_some() || self.shield[2] > 0.0 || !self.bursts.is_empty() {
                self.build_grid();
            }
            if s == 0 {
                self.apply_bursts(h);
            }
            if p.collide {
                for _ in 0..2 {
                    self.solve_pairs();
                }
            }
            self.solve_solids(cursor);
            self.finalize(h);
        }
        self.time += dt;
        self.cursor[2] = self.cursor[0];
        self.cursor[3] = self.cursor[1];
        self.cursor_was_on = self.cursor_on;
        for i in 0..self.active {
            let heat = if self.life[i] > 0.0 { (1.0 - self.age[i] / self.life[i]).clamp(0.0, 1.0) } else { 1.0 };
            let o = i * OUT_STRIDE;
            self.out[o..o + OUT_STRIDE]
                .copy_from_slice(&[self.x[i], self.y[i], self.vx[i], self.vy[i], self.energy[i], self.hue[i], heat, 0.0]);
        }
        self.emitter_out.clear();
        for e in &self.emitters {
            self.emitter_out.extend_from_slice(&[e.x, e.y, e.angle, e.power(), e.hue, 0.0, 0.0, 0.0]);
        }
    }
}

/// Pushes two equal-mass grains apart along their centre line until they just touch.
#[inline(always)]
fn contact(px: &mut [f32], py: &mut [f32], i: usize, j: usize, d0: f32) {
    let dx = px[j] - px[i];
    let dy = py[j] - py[i];
    let d2 = dx * dx + dy * dy;
    if d2 >= d0 * d0 {
        return;
    }
    if d2 < 1e-12 {
        px[j] += 0.05 * d0; // coincident: nudge apart deterministically
        return;
    }
    let d = d2.sqrt();
    let corr = 0.5 * (d0 - d) / d;
    px[i] -= dx * corr;
    py[i] -= dy * corr;
    px[j] += dx * corr;
    py[j] += dy * corr;
}

/// Smooth ramp from Bridson et al., "Curl-Noise for Procedural Fluid Flow": 0 at r=0, 1 for r ≥ 1.
fn ramp(r: f32) -> f32 {
    let r = r.clamp(-1.0, 1.0);
    let r3 = r * r * r;
    1.875 * r - 1.25 * r3 + 0.375 * r3 * r * r
}

/// Distance (in mask pixels) to the nearest pixel whose `solid` flag equals `target`.
fn chamfer(solid: &[bool], w: usize, h: usize, target: bool) -> Vec<f32> {
    let mut d: Vec<f32> = solid.iter().map(|&s| if s == target { 0.0 } else { 1e6 }).collect();
    let r2 = std::f32::consts::SQRT_2;
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let mut v = d[i];
            if x > 0 { v = v.min(d[i - 1] + 1.0); }
            if y > 0 {
                v = v.min(d[i - w] + 1.0);
                if x > 0 { v = v.min(d[i - w - 1] + r2); }
                if x + 1 < w { v = v.min(d[i - w + 1] + r2); }
            }
            d[i] = v;
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let i = y * w + x;
            let mut v = d[i];
            if x + 1 < w { v = v.min(d[i + 1] + 1.0); }
            if y + 1 < h {
                v = v.min(d[i + w] + 1.0);
                if x + 1 < w { v = v.min(d[i + w + 1] + r2); }
                if x > 0 { v = v.min(d[i + w - 1] + r2); }
            }
            d[i] = v;
        }
    }
    d
}

// ---------- C ABI for JavaScript ----------

fn world<'a>(p: *mut World) -> &'a mut World {
    // SAFETY: JS only passes pointers returned by `world_new` and never after `world_free`.
    unsafe { &mut *p }
}

#[unsafe(no_mangle)]
pub extern "C" fn world_new(cap: u32, w: f32, h: f32, seed: u32) -> *mut World {
    Box::into_raw(Box::new(World::new(cap as usize, w, h, seed)))
}

#[unsafe(no_mangle)]
pub extern "C" fn world_free(p: *mut World) {
    if !p.is_null() {
        // SAFETY: `p` came from `world_new` (Box::into_raw) and is freed once.
        drop(unsafe { Box::from_raw(p) });
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn world_set(p: *mut World, key: u32, value: f32) {
    world(p).set(key, value);
}

#[unsafe(no_mangle)]
pub extern "C" fn world_shield(p: *mut World, x: f32, y: f32, r: f32) {
    world(p).set_shield(x, y, r);
}

#[unsafe(no_mangle)]
pub extern "C" fn world_shield_hits(p: *mut World) -> u32 {
    world(p).shield_hits
}

/// Emitters as 8 floats each: x, y, angle, power, hue, 0, 0, 0.
#[unsafe(no_mangle)]
pub extern "C" fn world_emitters_ptr(p: *mut World) -> *const f32 {
    world(p).emitter_out.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn world_emitter_count(p: *mut World) -> u32 {
    world(p).emitters.len() as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn world_mask_alloc(p: *mut World, mw: u32, mh: u32, scale: f32) -> *mut u8 {
    world(p).mask_alloc(mw as usize, mh as usize, scale)
}

#[unsafe(no_mangle)]
pub extern "C" fn world_mask_commit(p: *mut World) {
    world(p).mask_commit();
}

#[unsafe(no_mangle)]
pub extern "C" fn world_sdf_ptr(p: *mut World) -> *const f32 {
    world(p).sdf.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn world_cursor(p: *mut World, x: f32, y: f32, on: u32) {
    world(p).set_cursor(x, y, on != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn world_burst(p: *mut World, x: f32, y: f32, radius: f32, strength: f32) {
    world(p).burst(x, y, radius, strength);
}

#[unsafe(no_mangle)]
pub extern "C" fn world_step(p: *mut World, dt: f32) {
    world(p).step(dt);
}

#[unsafe(no_mangle)]
pub extern "C" fn world_out_ptr(p: *mut World) -> *const f32 {
    world(p).out.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn world_active(p: *mut World) -> u32 {
    world(p).active as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quiet(cap: usize, w: f32, h: f32) -> World {
        let mut world = World::new(cap, w, h, 7);
        world.set_params(Params { gravity: 0.0, wind: 0.0, turb: 0.0, drag: 0.0, spawn_rate: 0.0, ..Params::default() });
        world
    }

    /// Solid block in mask pixels [x0, x1) × [y0, y1) at 0.5 mask px per CSS px.
    fn with_block(world: &mut World, x0: usize, x1: usize, y0: usize, y1: usize) {
        let (mw, mh) = ((world.w * 0.5) as usize, (world.h * 0.5) as usize);
        world.mask_alloc(mw, mh, 0.5);
        for y in y0..y1 {
            for x in x0..x1 {
                world.mask[y * mw + x] = 255;
            }
        }
        world.mask_commit();
    }

    fn dist(world: &World, i: usize, j: usize) -> f32 {
        ((world.x[i] - world.x[j]).powi(2) + (world.y[i] - world.y[j]).powi(2)).sqrt()
    }

    /// Current index of the grain added `tag`-th via `put`.
    fn tagged(world: &World, tag: usize) -> usize {
        (0..world.active).find(|&i| world.hue[i] == tag as f32).expect("tag present")
    }

    #[test]
    fn overlapping_grains_separate() {
        let mut world = quiet(2, 200.0, 200.0);
        world.put(100.0, 100.0, 0.0, 0.0);
        world.put(101.0, 100.0, 0.0, 0.0);
        world.step(1.0 / 60.0);
        assert!(dist(&world, 0, 1) >= 2.0 * world.params.radius - 0.05, "still overlapping: {}", dist(&world, 0, 1));
    }

    #[test]
    fn head_on_collision_conserves_momentum() {
        let mut world = quiet(2, 400.0, 200.0);
        world.put(190.0, 100.0, 300.0, 0.0);
        world.put(210.0, 100.0, -100.0, 0.0);
        for _ in 0..30 {
            world.step(1.0 / 60.0);
        }
        let total = world.vx[0] + world.vx[1];
        assert!((total - 200.0).abs() < 2.0, "momentum drifted to {total}");
        let (a, b) = (tagged(&world, 0), tagged(&world, 1));
        assert!(world.x[a] < world.x[b], "grains passed through each other");
        assert!(world.vx[a] <= world.vx[b] + 1e-3, "grains still approaching after contact");
    }

    #[test]
    fn falling_grain_rests_on_text_without_entering_it() {
        let mut world = quiet(1, 200.0, 200.0);
        world.params.gravity = 600.0;
        with_block(&mut world, 30, 70, 50, 60); // CSS 60..140 × 100..120
        world.put(100.0, 20.0, 0.0, 0.0);
        let r = world.params.radius;
        for _ in 0..300 {
            world.step(1.0 / 60.0);
            assert!(world.sdf_at(world.x[0], world.y[0]) > r - 0.75, "grain entered the glyph at y = {}", world.y[0]);
        }
        assert!((world.y[0] - (100.0 - r)).abs() < 1.5, "should rest on the top face, y = {}", world.y[0]);
        assert!(world.vy[0].abs() < 5.0, "should be at rest, vy = {}", world.vy[0]);
    }

    #[test]
    fn new_text_pushes_grains_out() {
        let mut world = quiet(1, 200.0, 200.0);
        world.put(100.0, 110.0, 0.0, 0.0); // inside where the block will appear
        with_block(&mut world, 30, 70, 50, 60);
        world.step(1.0 / 60.0);
        assert!(world.sdf_at(world.x[0], world.y[0]) >= world.params.radius - 0.75);
    }

    #[test]
    fn cursor_disc_shoves_grains_out_of_its_radius() {
        let mut world = quiet(1, 200.0, 200.0);
        world.put(100.0, 100.0, 0.0, 0.0);
        world.set_cursor(95.0, 100.0, true);
        world.step(1.0 / 60.0);
        let d = ((world.x[0] - 95.0).powi(2) + (world.y[0] - 100.0).powi(2)).sqrt();
        assert!(d >= world.params.cursor_r + world.params.radius - 0.05, "grain left inside the cursor disc: {d}");
    }

    #[test]
    fn burst_only_reaches_grains_inside_its_radius() {
        let mut world = quiet(2, 400.0, 400.0);
        world.put(230.0, 200.0, 0.0, 0.0); // 30 px from the burst
        world.put(300.0, 200.0, 0.0, 0.0); // 100 px away, outside a 70 px burst
        world.burst(200.0, 200.0, 70.0, 900.0);
        world.step(1.0 / 60.0);
        let (near, far) = (tagged(&world, 0), tagged(&world, 1));
        assert!(world.vx[near] > 100.0, "near grain should be thrown, vx = {}", world.vx[near]);
        assert!(world.vx[far].abs() < 1e-3 && world.vy[far].abs() < 1e-3, "far grain moved");
    }

    #[test]
    fn pile_settles_inside_the_arena_without_deep_overlap() {
        let mut world = quiet(400, 200.0, 200.0);
        world.params.gravity = 600.0;
        for k in 0..400 {
            world.put(20.0 + (k % 20) as f32 * 8.0, 10.0 + (k / 20) as f32 * 6.0, 0.0, 0.0);
        }
        for _ in 0..600 {
            world.step(1.0 / 60.0);
        }
        let r = world.params.radius;
        let mut worst = 0.0f32;
        for i in 0..world.active {
            assert!(world.y[i] <= world.h - r + 0.01 && world.x[i] >= r - 0.01, "grain {i} left the arena");
            for j in i + 1..world.active {
                worst = worst.max(2.0 * r - dist(&world, i, j));
            }
        }
        assert!(worst < 0.5 * r, "deepest overlap {worst} px");
    }

    #[test]
    fn heavy_pile_comes_to_rest_at_30_fps() {
        // Low frame rates must not inject energy: a pile under strong gravity settles at dt = 1/30 as well as 1/60.
        let mut world = quiet(600, 300.0, 200.0);
        world.params.gravity = 900.0;
        for k in 0..600 {
            world.put(20.0 + (k % 40) as f32 * 6.5, 10.0 + (k / 40) as f32 * 6.0, 0.0, 0.0);
        }
        for _ in 0..240 {
            world.step(1.0 / 30.0);
        }
        let n = world.active as f32;
        let mean_speed = (0..world.active).map(|i| world.vx[i].hypot(world.vy[i])).sum::<f32>() / n;
        let hot = (0..world.active).filter(|&i| world.energy[i] > 0.2).count();
        assert!(mean_speed < 15.0, "pile still moving: mean speed {mean_speed} px/s");
        assert!(hot == 0, "{hot} resting grains still glowing from contact jitter");
    }

    fn sparks(cap: usize) -> World {
        let mut world = quiet(cap, 400.0, 300.0);
        world.set_params(Params { collide: false, open_edges: true, life_min: 0.3, life_max: 0.6, emitters: 3, spawn_rate: 1e6, ..world.params });
        world
    }

    #[test]
    fn sparks_burn_out_and_relaunch_from_an_emitter() {
        let mut world = sparks(300);
        for _ in 0..90 {
            world.step(1.0 / 60.0);
        }
        assert_eq!(world.active, 300);
        assert!((0..world.active).all(|i| world.age[i] <= world.life[i] + 1.0 / 60.0), "a spark outlived its life");
        assert!((0..world.active).all(|i| world.life[i] >= 0.3 && world.life[i] <= 0.6));
    }

    #[test]
    fn sparks_pass_through_each_other_when_collisions_are_off() {
        let mut world = quiet(2, 200.0, 200.0);
        world.params.collide = false;
        world.put(100.0, 100.0, 0.0, 0.0);
        world.put(101.0, 100.0, 0.0, 0.0);
        world.step(1.0 / 60.0);
        assert!(dist(&world, 0, 1) < 1.5, "collisions should be off");
    }

    #[test]
    fn forcefield_bounces_sparks_and_keeps_them_outside() {
        let mut world = quiet(1, 400.0, 200.0);
        world.params.collide = false;
        world.put(60.0, 100.0, 600.0, 0.0);
        world.set_shield(200.0, 100.0, 60.0);
        let (mut hits, mut rebound) = (0, 0.0f32);
        for _ in 0..30 {
            world.step(1.0 / 60.0);
            hits += world.shield_hits;
            rebound = rebound.min(world.vx[0]); // before it reaches the (much less bouncy) arena wall
            let d = ((world.x[0] - 200.0).powi(2) + (world.y[0] - 100.0).powi(2)).sqrt();
            assert!(d >= 60.0 + world.params.radius - 0.05, "spark inside the forcefield: {d}");
        }
        assert!(hits > 0, "the forcefield never registered a hit");
        assert!(rebound < -400.0, "spark should rebound with most of its 600 px/s, got {rebound}");
    }

    #[test]
    fn sparks_only_originate_outside_the_screen() {
        let mut world = sparks(400);
        world.params.open_floor = true;
        for _ in 0..240 {
            world.step(1.0 / 60.0);
            for e in &world.emitters {
                let outside = e.x < 0.0 || e.x > world.w || e.y < 0.0 || e.y > world.h;
                assert!(outside, "emitter on screen at ({}, {})", e.x, e.y);
            }
        }
        // Fresh sparks (younger than one frame) are all still off screen.
        let fresh: Vec<usize> = (0..world.active).filter(|&i| world.age[i] < 1.0 / 60.0).collect();
        assert!(!fresh.is_empty());
        for i in fresh {
            let outside = world.x[i] < 0.0 || world.x[i] > world.w || world.y[i] < 0.0 || world.y[i] > world.h;
            assert!(outside, "spark born on screen at ({}, {})", world.x[i], world.y[i]);
        }
    }

    #[test]
    fn wander_makes_identical_launches_diverge() {
        let separation = |wander: f32| {
            let mut world = quiet(2, 800.0, 800.0);
            world.params.collide = false;
            world.params.wander = wander;
            world.put(400.0, 400.0, 0.0, 0.0);
            world.put(400.0, 400.0, 0.0, 0.0);
            for _ in 0..120 {
                world.step(1.0 / 60.0);
            }
            dist(&world, 0, 1)
        };
        assert!(separation(0.0) < 1e-3, "without wander, identical launches must stay together");
        assert!(separation(300.0) > 40.0, "with wander, identical launches should take different paths");
    }

    #[test]
    fn emitters_never_sit_inside_text() {
        let mut world = sparks(10);
        with_block(&mut world, 20, 180, 20, 130); // most of the arena is text
        for _ in 0..600 {
            world.step(1.0 / 60.0);
            for e in &world.emitters {
                assert!(world.sdf_at(e.x, e.y) > 0.0, "emitter inside text at ({}, {})", e.x, e.y);
            }
        }
    }

    #[test]
    fn source_pours_until_capacity_and_recycles_at_the_exit() {
        let mut world = World::new(500, 400.0, 300.0, 3);
        for _ in 0..120 {
            world.step(1.0 / 60.0);
        }
        assert_eq!(world.active, 500);
        assert!((0..world.active).all(|i| world.x[i] <= world.w + 12.0 + 30.0), "grains escaped the exit");
    }
}
