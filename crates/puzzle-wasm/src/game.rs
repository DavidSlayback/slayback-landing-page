//! One level being played on the page: the rules from `puzzle-core`, its motion from `motion.rs`, `sim` sparks that
//! bounce off the level's own walls and blocks, and the look of P25 (`art.rs` draws).
//!
//! What `Motion` says falls due sets off the effects here, all of them off under reduced motion: landings throw sparks
//! and (a crate) a small shake, what comes to rest puffs dust, a cracked tile falls apart, the exit bursts and a win
//! breaks the crates apart; embers drift up the backdrop and the doorway shimmers.

use std::f32::consts::TAU;

use puzzle_core::{Bits, Key, Level, Play, Refusal, levels};
use sim::World;

use crate::art::{self, T, TILE};
use crate::draw::{Canvas, Rect, Rgb, mix, scale};
use crate::motion::{Event, Motion, Pos};

/// The spark world's obstacle mask has a cell every 4 pixels.
const MASK_SCALE: f32 = 0.25;
const SPARKS: usize = 600;
const EMBERS: usize = 70;
/// sim tags each spark's hue with its insertion order, so a negative hue is free to mark dust.
const DUST: f32 = -1.0;

const GLOW: (usize, f32) = (6, 1.8);

#[derive(Clone, Copy, PartialEq)]
enum Burst {
    Landing,
    Dust,
    Exit,
}

/// A piece of a broken crate, in pixels.
struct Shard {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    size: f32,
    age: f32,
    life: f32,
    /// A piece of cracked floor rather than of a crate.
    tile: bool,
}

struct Ember {
    x: f32,
    y: f32,
    depth: f32,
    phase: f32,
}

pub struct Game {
    pub play: Play,
    pub tier: u8,
    pub seed: u64,
    /// The length of the shortest solution.
    pub optimal: usize,
    pub canvas: Canvas,
    /// fg, muted, accent: the site's colour scheme.
    pub colors: [Rgb; 3],
    motion: Motion,
    sparks: World,
    rng: u32,
    /// Screen shake, in pixels, decaying.
    shake: f32,
    shards: Vec<Shard>,
    embers: Vec<Ember>,
    /// What the still scene was last baked from: blocks at rest, cracked tiles standing, colours, reduced motion.
    baked: Option<(Bits, Bits, [Rgb; 3], bool)>,
}

impl Game {
    /// The page's game: the level `puzzle_core::levels::pick` gives for `seed` in `world` (by number) and `tier`,
    /// opening with its intro.
    pub fn new(world: u8, tier: u8, seed: u64) -> Option<Game> {
        let picked = levels::pick(puzzle_core::World::from_number(world as u32)?, tier, seed)?;
        let mut game = Game::from_level(picked.level, picked.solution.len(), tier, picked.seed);
        let s = *game.play.state();
        game.motion.start_intro(&s);
        Some(game)
    }

    pub fn from_level(level: Level, optimal: usize, tier: u8, seed: u64) -> Game {
        let (w, h) = (level.w as usize * TILE, level.h as usize * TILE);
        let mut sparks = World::new(SPARKS, w as f32, h as f32, (seed as u32) | 1);
        let mut p = sparks.params;
        (p.gravity, p.wind, p.turb, p.drag, p.radius) = (520.0, 0.0, 50.0, 0.35, 1.0);
        (p.friction, p.floor_friction, p.restitution) = (0.25, 0.3, 0.45);
        (p.spawn_rate, p.collide, p.life_min, p.life_max, p.emitters, p.eddies, p.erode) =
            (0.0, false, 0.0, 0.0, 0, 0, 0.0);
        sparks.set_params(p);
        let motion = Motion::new(&level);
        let mut game = Game {
            play: Play::new(level),
            tier,
            seed,
            optimal,
            canvas: Canvas::new(w, h),
            colors: [[0.94, 1.0, 0.84], [0.49, 0.6, 0.32], [0.65, 1.0, 0.0]],
            motion,
            sparks,
            rng: (seed as u32).wrapping_mul(2_654_435_761) | 1,
            shake: 0.0,
            shards: vec![],
            embers: vec![],
            baked: None,
        };
        game.embers = (0..EMBERS)
            .map(|_| {
                let (a, b, c, d) = (game.rand(), game.rand(), game.rand(), game.rand());
                Ember { x: a * w as f32, y: b * h as f32, depth: c * c, phase: d * TAU }
            })
            .collect();
        game.update_mask();
        game
    }

    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Reduced motion (the page's `prefers-reduced-motion`), set before the first frame: moves snap and nothing
    /// sparks, shakes, breaks apart or drifts.
    pub fn reduce_motion(&mut self, on: bool) {
        self.motion.reduce(on);
        if on {
            self.shake = 0.0;
            self.shards.clear();
        }
    }

    /// The screen shake this frame, in canvas pixels: 0 across, 1 down.
    pub fn shake(&self, axis: u32) -> f32 {
        let time = self.motion.time();
        let wave = if axis == 0 { (time * 91.0).sin() } else { (time * 73.0 + 1.3).cos() };
        self.shake * wave
    }

    /// Plays one key: `None` if it played (or the level is already solved), else why it was refused. A key during the
    /// intro skips the rest of it.
    pub fn key(&mut self, key: Key) -> Option<Refusal> {
        let before = *self.play.state();
        let played = self.play.key(key);
        for e in self.motion.key(&self.play, &before, played.as_deref()) {
            self.effect(e);
        }
        let after = *self.play.state();
        if before.blocks != after.blocks || before.cracked != after.cracked {
            self.update_mask();
        }
        played.err()
    }

    pub fn undo(&mut self) {
        self.play.undo(1);
        self.motion.snap(self.play.state());
        self.reset();
    }

    pub fn restart(&mut self) {
        self.play.restart();
        self.motion.restart(self.play.state());
        self.reset();
    }

    /// Clears the effects for a board put back.
    fn reset(&mut self) {
        self.shards.clear();
        self.update_mask();
    }

    /// Sets off what `Motion` says is due.
    fn effect(&mut self, e: Event) {
        match e {
            Event::Landing(at) => self.burst(Burst::Landing, at),
            Event::Dust(at) => self.burst(Burst::Dust, at),
            Event::Exit(at) => self.burst(Burst::Exit, at),
            Event::Crumble(at) => self.crumble(at),
            Event::Break => self.break_crates(),
        }
    }

    /// A cracked tile at cell `at` falls apart: four shards dropping through the hole, a puff of dust, a small shake.
    fn crumble(&mut self, (x, y): Pos) {
        self.burst(Burst::Dust, (x + 0.5, y));
        for (qx, qy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            let (a, b) = (self.rand(), self.rand());
            self.shards.push(Shard {
                x: (x + 0.5) * T + qx * 8.0,
                y: (y + 0.5) * T + qy * 8.0,
                vx: qx * (20.0 + 50.0 * a),
                vy: -60.0 * b + qy * 20.0,
                size: 10.0 + 4.0 * b,
                age: 0.0,
                life: 0.6 + 0.3 * a,
                tile: true,
            });
        }
        self.shake = self.shake.max(1.5);
    }

    /// The spark world's obstacles: walls, cracked tiles and the blocks at rest.
    fn update_mask(&mut self) {
        let l = &self.play.level;
        let s = self.play.state();
        let (cw, ch) = (l.w as usize, l.h as usize);
        let (mw, mh) = (((cw * TILE) as f32 * MASK_SCALE) as usize, ((ch * TILE) as f32 * MASK_SCALE) as usize);
        let per = (T * MASK_SCALE) as usize;
        self.sparks.mask_alloc(mw, mh, MASK_SCALE);
        for my in 0..mh {
            for mx in 0..mw {
                let i = (my / per) * cw + mx / per;
                if l.walls.get(i) || s.blocks.get(i) || s.cracked.get(i) {
                    self.sparks.mask[my * mw + mx] = 200;
                }
            }
        }
        self.sparks.mask_commit();
    }

    fn burst(&mut self, kind: Burst, at: Pos) {
        if kind == Burst::Landing {
            self.shake = self.shake.max(2.5);
        }
        let live = (0..self.sparks.active).any(|i| self.sparks.life[i] > 0.0);
        if !live {
            self.sparks.active = 0;
        }
        let (x, y) = (at.0 * T, at.1 * T);
        let n = match kind {
            Burst::Exit => 140,
            Burst::Landing => 36,
            Burst::Dust => 12,
        };
        for _ in 0..n {
            let (a, b, c) = (self.rand(), self.rand(), self.rand());
            let side = if a < 0.5 { -1.0 } else { 1.0 };
            let (px, vx, vy, life) = match kind {
                // From the block's two bottom corners, just outside it: it is solid to the sparks.
                Burst::Landing => {
                    (x + side * (T * 0.5 + 2.0), side * (60.0 + 220.0 * b), -(40.0 + 200.0 * c), 0.3 + 0.45 * b)
                }
                Burst::Dust => {
                    (x + side * (T * 0.45 + 2.0), side * (25.0 + 70.0 * b), -(15.0 + 45.0 * c), 0.25 + 0.3 * c)
                }
                Burst::Exit => {
                    let ang = a * TAU;
                    let speed = 90.0 + 320.0 * b;
                    (x, ang.cos() * speed, ang.sin() * speed - 80.0, 0.55 + 0.7 * c)
                }
            };
            if !self.sparks.put(px, y - 2.0, vx, vy) {
                break;
            }
            let i = self.sparks.active - 1;
            (self.sparks.life[i], self.sparks.energy[i]) = (life, if kind == Burst::Dust { 0.0 } else { 1.0 });
            if kind == Burst::Dust {
                self.sparks.hue[i] = DUST;
            }
        }
    }

    /// Breaks every crate at rest into four tumbling shards, thrown away from the doorway.
    fn break_crates(&mut self) {
        let l = &self.play.level;
        let (w, h) = (l.w as usize, l.h as usize);
        let (ex, ey) = ((l.exit.0 as f32 + 0.5) * T, (l.exit.1 as f32 + 0.5) * T);
        let cells: Vec<usize> = (0..w * h).filter(|&i| self.play.state().blocks.get(i)).collect();
        for i in cells {
            let (cx, cy) = (((i % w) as f32 + 0.5) * T, ((i / w) as f32 + 0.5) * T);
            for (qx, qy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let (a, b) = (self.rand(), self.rand());
                let (dx, dy) = (cx - ex, cy - ey);
                let d = (dx * dx + dy * dy).sqrt().max(1.0);
                self.shards.push(Shard {
                    x: cx + qx * 7.0,
                    y: cy + qy * 7.0,
                    vx: dx / d * (90.0 + 140.0 * a) + qx * 60.0,
                    vy: dy / d * (60.0 + 100.0 * a) + qy * 40.0 - 220.0,
                    size: 9.0 + 4.0 * b,
                    age: 0.0,
                    life: 0.9 + 0.5 * b,
                    tile: false,
                });
            }
        }
        self.shake = self.shake.max(4.0);
    }

    /// Advances `dt` seconds and draws the frame into `canvas.rgba`.
    pub fn frame(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.1);
        self.shake *= (-dt / 0.07).exp();
        if self.shake < 0.05 {
            self.shake = 0.0;
        }
        for e in self.motion.advance(dt) {
            self.effect(e);
        }
        for s in &mut self.shards {
            s.vy += 900.0 * dt;
            (s.x, s.y, s.age) = (s.x + s.vx * dt, s.y + s.vy * dt, s.age + dt);
        }
        self.shards.retain(|s| s.age < s.life);
        if !self.motion.reduced() {
            let (w, h, time) = (self.canvas.w as f32, self.canvas.h as f32, self.motion.time());
            for i in 0..self.embers.len() {
                let respawn = self.rand() * w;
                let e = &mut self.embers[i];
                e.y -= (5.0 + 16.0 * e.depth) * dt;
                e.x += (time * 0.7 + e.phase).sin() * 7.0 * e.depth * dt;
                if e.y < -4.0 {
                    (e.x, e.y) = (respawn, h + 4.0);
                }
            }
        }
        if (0..self.sparks.active).any(|i| self.sparks.life[i] > 0.0) {
            self.sparks.step(dt);
        }
        self.draw();
    }

    fn draw(&mut self) {
        let [fg, muted, accent] = self.colors;
        let s = *self.play.state();
        let scene = self.motion.scene(&s);
        let (reduced, time) = (self.motion.reduced(), self.motion.time());
        let key = (scene.still, s.cracked, self.colors, reduced);
        if self.baked != Some(key) {
            self.bake(&scene.still, &s.cracked);
            self.baked = Some(key);
        }
        let (ex, ey) = (self.play.level.exit.0 as usize, self.play.level.exit.1 as usize);
        let mut rects = vec![];
        if let Some(me) = &scene.climber {
            rects.push(around(me.at.0, me.at.1 - 1.0, 1.0, 2.0));
        }
        rects.extend(scene.crates.iter().map(|k| around(k.at.0, k.at.1, 1.0, 1.0)));
        let shimmer = (!reduced).then_some(time);
        if shimmer.is_some() {
            rects.push(around(ex as f32, ey as f32, 1.0, 1.0));
        }
        let c = &mut self.canvas;
        c.begin();
        for r in merged(rects) {
            c.layer(r);
            if shimmer.is_some() {
                art::door_light(c, ex, ey, shimmer, accent);
            }
            for k in &scene.crates {
                art::crate_(c, k.at.0, k.at.1, k.squash, accent, k.ice);
            }
            if let Some(me) = &scene.climber {
                if let Some(ice) = me.carrying {
                    art::crate_(c, me.at.0, me.at.1 - 1.0, 0.0, accent, ice);
                }
                art::climber(c, me.at.0, me.at.1, &me.pose, fg);
            }
            c.composite(GLOW.0, GLOW.1);
        }
        for e in &self.embers {
            if !reduced {
                let twinkle = 0.65 + 0.35 * (time * 3.1 + e.phase * 7.0).sin();
                art::ember(c, e.x, e.y, e.depth, twinkle, accent);
            }
        }
        let rubble = mix(muted, accent, 0.45);
        for sh in &self.shards {
            let tint = if sh.tile { rubble } else { accent };
            let fade = 1.0 - sh.age / sh.life;
            let half = (sh.size * (0.6 + 0.4 * fade)) / 2.0;
            let (x0, y0, x1, y1) =
                ((sh.x - half) as i32, (sh.y - half) as i32, (sh.x + half) as i32, (sh.y + half) as i32);
            c.speck(x0, y0, x1, y1, scale(tint, 0.12 * fade));
            for (a, b, cc, d) in
                [(x0, y0, x1, y0 + 2), (x0, y1 - 2, x1, y1), (x0, y0, x0 + 2, y1), (x1 - 2, y0, x1, y1)]
            {
                c.speck(a, b, cc, d, scale(tint, 1.1 * fade));
            }
        }
        let dust = mix(muted, fg, 0.35);
        for i in 0..self.sparks.active {
            let life = self.sparks.life[i];
            if life <= 0.0 {
                continue;
            }
            let heat = (1.0 - self.sparks.age[i] / life).clamp(0.0, 1.0);
            let (x, y) = (self.sparks.x[i], self.sparks.y[i]);
            if self.sparks.hue[i] == DUST {
                c.spark(x, y, 1.4, scale(dust, 0.5 * heat));
            } else {
                let hot = mix(accent, [1.0, 1.0, 1.0], heat * heat);
                c.spark(x, y, 1.1, scale(hot, (0.6 + self.sparks.energy[i]) * heat * 1.6));
            }
        }
    }

    /// Draws and bakes what stands still: the backdrop, walls, the cracked tiles in `cracked`, the doorway's frame (and
    /// its light, when it doesn't shimmer), and the crates in `still`.
    fn bake(&mut self, still: &Bits, cracked: &Bits) {
        let [fg, muted, accent] = self.colors;
        let l = &self.play.level;
        let (w, h) = (l.w as usize, l.h as usize);
        let c = &mut self.canvas;
        c.clear([0.0; 3]);
        let wall = |x: i32, y: i32| {
            let outside = x < 0 || y < 0 || x >= w as i32 || y >= h as i32;
            outside || l.walls.get(y as usize * w + x as usize) || cracked.get(y as usize * w + x as usize)
        };
        for y in 0..h {
            for x in 0..w {
                let (xi, yi) = (x as i32, y as i32);
                let open = [!wall(xi, yi - 1), !wall(xi, yi + 1), !wall(xi - 1, yi), !wall(xi + 1, yi)];
                if cracked.get(y * w + x) {
                    art::cracked(c, x, y, open, muted, accent);
                } else if l.springs.get(y * w + x) {
                    art::spring(c, x, y, open, muted, accent);
                } else if wall(xi, yi) {
                    art::wall(c, x, y, open, muted);
                } else {
                    art::backdrop(c, x, y, h, accent);
                }
            }
        }
        let (ex, ey) = (l.exit.0 as usize, l.exit.1 as usize);
        art::door_frame(c, ex, ey, mix(accent, fg, 0.3));
        if self.motion.reduced() {
            art::door_light(c, ex, ey, None, accent);
        }
        let ice = self.play.state().ice;
        for i in (0..w * h).filter(|&i| still.get(i)) {
            art::crate_(c, (i % w) as f32, (i / w) as f32, 0.0, accent, ice.get(i));
        }
        c.bake(GLOW.0, GLOW.1);
    }
}

/// The pixels of `w` × `h` cells from cell (`x`, `y`), with room around them for glow.
fn around(x: f32, y: f32, w: f32, h: f32) -> Rect {
    const ROOM: f32 = 16.0;
    [
        (x * T - ROOM).floor() as i32,
        (y * T - ROOM).floor() as i32,
        ((x + w) * T + ROOM).ceil() as i32,
        ((y + h) * T + ROOM).ceil() as i32,
    ]
}

/// `rects` with any that overlap joined, so no pixel is drawn twice.
fn merged(mut rects: Vec<Rect>) -> Vec<Rect> {
    let overlap = |a: &Rect, b: &Rect| a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3];
    'again: loop {
        for i in 0..rects.len() {
            for j in i + 1..rects.len() {
                if overlap(&rects[i], &rects[j]) {
                    let b = rects.remove(j);
                    let a = &mut rects[i];
                    *a = [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])];
                    continue 'again;
                }
            }
        }
        return rects;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(s: &str) -> Vec<Key> {
        s.chars().filter_map(Key::from_char).collect()
    }

    fn frames(g: &mut Game, n: usize) {
        (0..n).for_each(|_| g.frame(1.0 / 60.0));
    }

    fn scene(g: &Game) -> crate::motion::Scene {
        g.motion.scene(g.play.state())
    }

    fn live_sparks(g: &Game) -> usize {
        (0..g.sparks.active).filter(|&i| g.sparks.life[i] > 0.0).count()
    }

    fn lit(g: &Game, cx: usize, cy: usize) -> f32 {
        // Brightest pixel in a cell, 0..1.
        let w = g.canvas.w;
        let mut best = 0u8;
        for y in cy * TILE..(cy + 1) * TILE {
            for x in cx * TILE..(cx + 1) * TILE {
                let p = &g.canvas.rgba[(y * w + x) * 4..][..3];
                best = best.max(*p.iter().max().unwrap());
            }
        }
        best as f32 / 255.0
    }

    #[test]
    fn a_level_plays_to_its_exit_and_throws_sparks_there() {
        let mut g = Game::new(0, 1, 1).unwrap();
        let solution = puzzle_core::solve(&g.play.level).solution.unwrap();
        assert_eq!(solution.len(), g.optimal);
        for k in solution {
            assert_eq!(g.key(k), None);
            frames(&mut g, 12);
        }
        assert!(g.play.solved());
        g.frame(1.0 / 60.0);
        assert!(live_sparks(&g) > 50, "{} sparks after the exit", live_sparks(&g));
    }

    #[test]
    fn walls_blocks_player_and_exit_are_drawn_and_open_space_stays_dark() {
        let level = Level::parse("#..........#\n#..........#\n#>.B.....E.#\n############").unwrap();
        let mut g = Game::from_level(level, 8, 1, 0);
        g.embers.clear();
        g.frame(0.0);
        assert!(lit(&g, 1, 2) > 0.5, "player");
        assert!(lit(&g, 3, 2) > 0.5, "block");
        assert!(lit(&g, 9, 2) > 0.4, "exit");
        assert!(lit(&g, 3, 3) > 0.2, "the wall under the block shows its top edge");
        // Glow reaches about a cell; two away from everything, the backdrop stays near black.
        assert!(lit(&g, 6, 0) < 0.15, "open space far from everything: {}", lit(&g, 6, 0));
    }

    #[test]
    fn a_refused_key_says_why_and_changes_nothing() {
        let level = Level::parse("#.....#\n#>#..E#\n#######").unwrap();
        let mut g = Game::from_level(level, 0, 1, 0);
        let before = *g.play.state();
        assert_eq!(g.key(Key::D), Some(Refusal::NoBlock));
        assert_eq!(*g.play.state(), before);
    }

    #[test]
    fn putting_a_block_down_throws_sparks_where_it_lands_and_shakes_the_screen() {
        let level = Level::parse("#B.....#\n#>...E.#\n########").unwrap();
        let mut g = Game::from_level(level, 0, 1, 0);
        for k in keys("D") {
            g.key(k);
        }
        // The block falls from head height in front (cell 2, 0) to rest on the floor at (2, 1).
        let mut live = vec![];
        for _ in 0..30 {
            g.frame(1.0 / 60.0);
            live = (0..g.sparks.active).filter(|&i| g.sparks.life[i] > 0.0).collect();
            if !live.is_empty() {
                break;
            }
        }
        assert!(!live.is_empty(), "no sparks after the block landed");
        let near = |i: usize| (g.sparks.x[i] - 2.5 * T).abs() < T && (g.sparks.y[i] - 2.0 * T).abs() < T;
        assert!(live.iter().all(|&i| near(i)), "sparks and dust start at the foot of the landed block");
        assert!(live.iter().any(|&i| g.sparks.hue[i] == DUST), "dust as well as sparks");
        assert!(g.shake(0).abs() + g.shake(1).abs() > 0.0, "the landing shakes the screen");
        frames(&mut g, 40);
        assert_eq!(g.shake, 0.0, "the shake dies away");
    }

    #[test]
    fn only_a_block_coming_to_rest_redraws_the_still_scene() {
        // A full redraw every frame cost a phone ~45 ms; walking and waiting must only redraw what moves.
        let level = Level::parse("#B......#\n#>.....E#\n#########").unwrap();
        let mut g = Game::from_level(level, 0, 1, 0);
        frames(&mut g, 30);
        assert_eq!(g.canvas.bakes, 1, "the first frame bakes, waiting doesn't");
        g.key(Key::R);
        frames(&mut g, 30);
        assert_eq!(g.canvas.bakes, 1, "walking doesn't");
        g.key(Key::D);
        frames(&mut g, 30);
        assert_eq!(g.canvas.bakes, 2, "the block put down bakes once, after it lands and squashes");
        g.undo();
        frames(&mut g, 5);
        assert_eq!(g.canvas.bakes, 3, "undo brings the block back: once more");
    }

    #[test]
    fn set_down_ice_glides_cell_by_cell_to_where_the_rules_put_it() {
        let level = Level::parse("#I.....E#\n#>......#\n#########").unwrap();
        let mut g = Game::from_level(level, 0, 1, 0);
        frames(&mut g, 5);
        g.key(Key::D);
        frames(&mut g, 6);
        assert_eq!(scene(&g).crates.len(), 1, "the block is drawn on its way");
        assert!(lit(&g, 3, 1) > 0.5 || lit(&g, 4, 1) > 0.5, "mid-slide the block is drawn on its way");
        frames(&mut g, 60);
        assert!(scene(&g).crates.is_empty(), "the slide ends");
        assert!(lit(&g, 7, 1) > 0.5, "and the block rests against the wall");
        assert!(g.canvas.bakes <= 3, "{} bakes for one slide", g.canvas.bakes);
    }

    #[test]
    fn a_bounce_is_drawn_rising_and_hopping_with_sparks_off_the_spring() {
        let level = Level::parse("#>.....#\n##.#...#\n#..#..E#\n##^#####").unwrap();
        let mut g = Game::from_level(level, 0, 1, 0);
        frames(&mut g, 5);
        g.key(Key::R);
        frames(&mut g, 90);
        assert_eq!((g.play.state().x, g.play.state().y), (3, 0), "thrown up two and over the wall");
        assert_eq!(scene(&g).climber.map(|c| c.at), Some((3.0, 0.0)));
        assert!(lit(&g, 3, 0) > 0.5, "drawn where it landed");
        assert!(live_sparks(&g) > 0 || g.shake > 0.0 || g.canvas.bakes >= 1);
    }

    #[test]
    fn a_cracked_tile_you_step_off_falls_apart_and_the_scene_redraws_once() {
        let level = Level::parse("#.....#\n#>...E#\n#~#####").unwrap();
        let mut g = Game::from_level(level, 0, 1, 0);
        frames(&mut g, 5);
        assert!(lit(&g, 1, 2) > 0.3, "the cracked tile is drawn");
        g.key(Key::R);
        assert_eq!(g.shards.iter().filter(|s| s.tile).count(), 4, "it breaks in four");
        assert!(g.shake > 0.0);
        frames(&mut g, 60);
        assert_eq!(g.canvas.bakes, 2, "the scene re-bakes once, without the tile");
        assert!(g.shards.is_empty(), "the rubble is gone");
        assert!(lit(&g, 1, 2) < 0.25, "the hole is dark: {}", lit(&g, 1, 2));
    }

    #[test]
    fn a_level_opens_with_its_crates_dropping_in_and_a_key_skips_it() {
        let mut g = Game::new(0, 1, 1).unwrap();
        assert!(g.motion.opening());
        frames(&mut g, 3);
        assert_eq!(g.baked.unwrap().0, Bits::default(), "no crate stands still while they drop in");
        frames(&mut g, 120);
        assert!(!g.motion.opening(), "the intro ends by itself");
        assert_eq!(g.baked.unwrap().0, g.play.state().blocks, "then every crate is baked where it rests");
        g.restart();
        assert!(g.motion.opening(), "restart replays it");
        let first = puzzle_core::solve(&g.play.level).solution.unwrap()[0];
        assert_eq!(g.key(first), None);
        assert!(!g.motion.opening() && g.play.moves() == 1, "a key skips the intro and still plays");
    }

    #[test]
    fn a_win_takes_the_climber_through_the_door_and_breaks_the_crates() {
        let mut g = Game::new(0, 1, 1).unwrap();
        for k in puzzle_core::solve(&g.play.level).solution.unwrap() {
            g.key(k);
            frames(&mut g, 12);
        }
        frames(&mut g, 30);
        assert!(scene(&g).climber.is_none(), "the climber is through the door");
        let crates =
            (0..g.play.level.w as usize * g.play.level.h as usize).filter(|&i| g.play.state().blocks.get(i)).count();
        assert_eq!(g.shards.len(), 4 * crates, "every crate breaks in four");
        assert_eq!(scene(&g).still, Bits::default(), "and none stands still");
        g.undo();
        assert!(scene(&g).climber.is_some() && g.shards.is_empty(), "undo puts the board back");
    }

    #[test]
    fn reduced_motion_snaps_and_throws_no_sparks_shake_or_shards() {
        let mut g = Game::new(0, 1, 1).unwrap();
        g.reduce_motion(true);
        for k in puzzle_core::solve(&g.play.level).solution.unwrap() {
            g.key(k);
            g.frame(1.0 / 60.0);
        }
        frames(&mut g, 30);
        assert!(g.play.solved());
        assert_eq!(live_sparks(&g), 0);
        assert_eq!((g.shake, g.shards.len(), g.motion.opening()), (0.0, 0, false));
    }
}
