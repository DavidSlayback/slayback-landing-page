//! One level being played on the page: the rules from `puzzle-core`, short tweens so moves never snap, `sim` sparks
//! that bounce off the level's own walls and blocks, and the look and motion of P25 and P26 (`art.rs` draws).
//!
//! Motion, all of it off under reduced motion: the climber walks, climbs, breathes, blinks and leans into turns; what
//! falls stretches and lands with a squash, dust and (a crate) a small shake; embers drift up the backdrop and the
//! doorway shimmers; a level opens with its crates dropping in, and a win takes the climber through the door and
//! breaks the crates apart.

use std::f32::consts::{PI, TAU};

use puzzle_core::{Bits, Facing, Key, Level, Play, Refusal, State, TIERS, generate};
use sim::World;

use crate::art::{self, Pose, T, TILE};
use crate::draw::{Canvas, Rect, Rgb, mix, scale};

/// The spark world's obstacle mask has a cell every 4 pixels.
const MASK_SCALE: f32 = 0.25;
const SPARKS: usize = 600;
const EMBERS: usize = 70;
/// sim tags each spark's hue with its insertion order, so a negative hue is free to mark dust.
const DUST: f32 = -1.0;

const WALK: f32 = 0.08;
const CLIMB: f32 = 0.09;
const LIFT: f32 = 0.09;
/// How long a landing squashes.
const SQUASH: f32 = 0.14;
/// A win: the climber shrinks into the doorway, then the crates break.
const INTO_DOOR: f32 = 0.35;
const GLOW: (usize, f32) = (6, 1.8);

/// A position in cells (fractional while tweening).
type Pos = (f32, f32);

fn lerp(a: Pos, b: Pos, t: f32) -> Pos {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

/// How long a fall of `rows` cells takes: quick, and slower for longer drops so they read as falls.
fn fall_time(rows: f32) -> f32 {
    0.05 + 0.055 * rows.max(0.0).sqrt()
}

/// A two-part path: across (or up) to `mid`, then falling to `to`, eased in like gravity.
#[derive(Clone, Copy)]
struct Path {
    from: Pos,
    mid: Pos,
    to: Pos,
    across: f32,
    fall: f32,
}

impl Path {
    fn still(at: Pos) -> Path {
        Path { from: at, mid: at, to: at, across: 0.0, fall: 0.0 }
    }
    fn drop(from: Pos, to: Pos) -> Path {
        Path { from, mid: from, to, across: 0.0, fall: fall_time(to.1 - from.1) }
    }
    fn total(&self) -> f32 {
        self.across + self.fall
    }
    fn at(&self, t: f32) -> Pos {
        if t < self.across {
            lerp(self.from, self.mid, t / self.across)
        } else if t < self.total() {
            let u = (t - self.across) / self.fall;
            lerp(self.mid, self.to, u * u)
        } else {
            self.to
        }
    }
    /// Stretched while falling (< 0), squashed for a moment on landing (> 0), harder after a longer drop.
    fn squash(&self, t: f32) -> f32 {
        if self.fall <= 0.0 || t < self.across {
            0.0
        } else if t < self.total() {
            -0.3 * (t - self.across) / self.fall
        } else if t < self.total() + SQUASH {
            (PI * (t - self.total()) / SQUASH).sin() * (0.45 + 0.25 * (self.to.1 - self.mid.1)).min(1.0)
        } else {
            0.0
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Burst {
    Landing,
    Dust,
    Exit,
}

/// What is moving since the last key: the player, and a block on its way to or from the player's head.
struct Tween {
    t: f32,
    player: Path,
    /// A block in motion and the cell it ends in (hidden from the still blocks until it has landed and squashed);
    /// `None` for the block being lifted, which ends on the player's head.
    block: Option<(Path, Option<usize>)>,
    /// What happens when, in seconds into the tween: sparks, dust, the win.
    events: Vec<(f32, Burst, Pos)>,
}

impl Tween {
    fn still(at: Pos) -> Tween {
        Tween { t: 0.0, player: Path::still(at), block: None, events: vec![] }
    }
    /// The moving block while it still moves or squashes.
    fn block_moving(&self) -> Option<(Path, Option<usize>)> {
        self.block.filter(|(p, _)| self.t < p.total() + if p.fall > 0.0 { SQUASH } else { 0.0 })
    }
}

/// The level opening: crates drop into place bottom first, column by column, then the climber.
struct Intro {
    t: f32,
    drops: Vec<(f32, Path)>,
    player: (f32, Path),
}

impl Intro {
    fn end(&self) -> f32 {
        let last = self.drops.iter().chain([&self.player]).map(|(d, p)| d + p.total()).fold(0.0, f32::max);
        last + SQUASH
    }
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
    pub reduce_motion: bool,
    tween: Tween,
    sparks: World,
    rng: u32,
    time: f32,
    refused: f32,
    /// A turn: when, and which way, for the lean.
    turned: (f32, f32),
    /// Screen shake, in pixels, decaying.
    shake: f32,
    /// Whether restarting replays the intro (the page's games do; tests' don't).
    intro_on: bool,
    intro: Option<Intro>,
    /// Seconds since the climber reached the doorway.
    win: Option<f32>,
    shards: Vec<Shard>,
    embers: Vec<Ember>,
    /// What the still scene was last baked from: blocks at rest, colours, reduced motion.
    baked: Option<(Bits, [Rgb; 3], bool)>,
}

impl Game {
    /// The page's game: the level for `seed` in `tier`, opening with its intro.
    pub fn new(tier: u8, seed: u64) -> Option<Game> {
        let g = generate(seed, TIERS.get(tier.checked_sub(1)? as usize)?)?;
        let mut game = Game::from_level(g.level, g.solution.len(), tier, seed);
        game.intro_on = true;
        game.start_intro();
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
        let start = level.start;
        let mut game = Game {
            play: Play::new(level),
            tier,
            seed,
            optimal,
            canvas: Canvas::new(w, h),
            colors: [[0.94, 1.0, 0.84], [0.49, 0.6, 0.32], [0.65, 1.0, 0.0]],
            reduce_motion: false,
            tween: Tween::still((start.x as f32, start.y as f32)),
            sparks,
            rng: (seed as u32).wrapping_mul(2_654_435_761) | 1,
            time: 0.0,
            refused: 0.0,
            turned: (-1.0, 0.0),
            shake: 0.0,
            intro_on: false,
            intro: None,
            win: None,
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

    /// The screen shake this frame, in canvas pixels: 0 across, 1 down.
    pub fn shake(&self, axis: u32) -> f32 {
        let wave = if axis == 0 { (self.time * 91.0).sin() } else { (self.time * 73.0 + 1.3).cos() };
        self.shake * wave
    }

    /// Drops the crates in, bottom first, column by column, then the climber.
    pub fn start_intro(&mut self) {
        let (w, h) = (self.play.level.w as usize, self.play.level.h as usize);
        let s = *self.play.state();
        let mut drops: Vec<(f32, Path)> = (0..w * h)
            .filter(|&i| s.blocks.get(i))
            .map(|i| {
                let (x, y) = ((i % w) as f32, (i / w) as f32);
                (x * 0.035 + (h as f32 - 1.0 - y) * 0.06, Path::drop((x, -1.5), (x, y)))
            })
            .collect();
        let last = drops.iter().map(|d| d.0).fold(0.0, f32::max);
        // Keep the whole intro short on a big level.
        let fit = if last > 0.9 { 0.9 / last } else { 1.0 };
        drops.iter_mut().for_each(|d| d.0 *= fit);
        let at = (s.x as f32, s.y as f32);
        let player = (last * fit + 0.12, Path::drop((at.0, -1.5), at));
        self.intro = Some(Intro { t: 0.0, drops, player });
    }

    /// Plays one key: `None` if it played (or the level is already solved), else why it was refused. A key during the
    /// intro skips the rest of it.
    pub fn key(&mut self, key: Key) -> Option<Refusal> {
        self.intro = None;
        let before = *self.play.state();
        let applied = self.play.apply(&format!("{key:?}"));
        if let Some(stop) = applied.stop {
            self.refused = 0.18;
            return Some(stop.why);
        }
        if applied.played == 1 {
            let after = *self.play.state();
            if after.facing != before.facing && (after.x, after.y) == (before.x, before.y) {
                self.turned = (self.time, if after.facing == Facing::Left { -1.0 } else { 1.0 });
            }
            self.animate(&before, &after);
            if before.blocks != after.blocks {
                self.update_mask();
            }
        }
        None
    }

    pub fn undo(&mut self) {
        self.play.undo(1);
        self.snap();
    }

    pub fn restart(&mut self) {
        self.play.restart();
        self.snap();
        if self.intro_on && !self.reduce_motion {
            self.start_intro();
        }
    }

    fn snap(&mut self) {
        let s = *self.play.state();
        self.tween = Tween::still((s.x as f32, s.y as f32));
        self.intro = None;
        self.win = None;
        self.shards.clear();
        self.update_mask();
    }

    /// Sets up the tween from `before` to `after`, starting wherever the player is drawn now.
    fn animate(&mut self, before: &State, after: &State) {
        let now = self.tween.player.at(self.tween.t);
        let (bx, by) = (before.x as f32, before.y as f32);
        let to = (after.x as f32, after.y as f32);
        let player = if after.y < before.y {
            Path { from: now, mid: to, to, across: CLIMB, fall: 0.0 }
        } else if after.x != before.x {
            let mid = (to.0, by);
            Path { from: now, mid, to, across: WALK, fall: if to.1 > by { fall_time(to.1 - by) } else { 0.0 } }
        } else {
            Path::still(to) // a turn: the facing flips at once, and the body leans into it
        };
        let front = bx + if before.facing == Facing::Left { -1.0 } else { 1.0 };
        let w = self.play.level.w as usize;
        let block = match (before.carrying, after.carrying) {
            (false, true) => Some((
                Path { from: (front, by), mid: (bx, by - 1.0), to: (bx, by - 1.0), across: LIFT, fall: 0.0 },
                None,
            )),
            (true, false) => {
                let cell = (0..w * self.play.level.h as usize).find(|&i| after.blocks.get(i) && !before.blocks.get(i));
                cell.map(|i| (Path::drop((front, by - 1.0), ((i % w) as f32, (i / w) as f32)), Some(i)))
            }
            _ => None,
        };
        let mut events = vec![];
        if player.fall > 0.0 {
            events.push((player.total(), Burst::Dust, (to.0 + 0.5, to.1 + 1.0)));
        }
        if let Some((path, Some(_))) = block {
            events.push((path.total(), Burst::Landing, (path.to.0 + 0.5, path.to.1 + 1.0)));
            events.push((path.total(), Burst::Dust, (path.to.0 + 0.5, path.to.1 + 1.0)));
        }
        if self.play.solved() {
            events.push((player.total(), Burst::Exit, (to.0 + 0.5, to.1 + 0.5)));
        }
        self.tween = if self.reduce_motion { Tween::still(to) } else { Tween { t: 0.0, player, block, events } };
    }

    /// The spark world's obstacles: walls and the blocks at rest.
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
                if l.walls.get(i) || s.blocks.get(i) {
                    self.sparks.mask[my * mw + mx] = 200;
                }
            }
        }
        self.sparks.mask_commit();
    }

    fn burst(&mut self, kind: Burst, at: Pos) {
        if self.reduce_motion {
            return;
        }
        match kind {
            Burst::Landing => self.shake = self.shake.max(2.5),
            Burst::Exit => self.win = Some(0.0),
            Burst::Dust => {}
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
                });
            }
        }
        self.shake = self.shake.max(4.0);
    }

    /// Advances `dt` seconds and draws the frame into `canvas.rgba`.
    pub fn frame(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.1);
        self.time += dt;
        self.refused = (self.refused - dt).max(0.0);
        if self.reduce_motion {
            (self.intro, self.shake) = (None, 0.0);
            self.shards.clear();
        }
        self.shake *= (-dt / 0.07).exp();
        if self.shake < 0.05 {
            self.shake = 0.0;
        }
        if let Some(intro) = &mut self.intro {
            intro.t += dt;
            if intro.t >= intro.end() {
                self.intro = None;
            }
        }
        self.tween.t += dt;
        let due: Vec<_> = self.tween.events.iter().copied().filter(|e| e.0 <= self.tween.t).collect();
        self.tween.events.retain(|e| e.0 > self.tween.t);
        for (_, kind, at) in due {
            self.burst(kind, at);
        }
        if let Some(t) = &mut self.win {
            let before = *t;
            *t += dt;
            if before < INTO_DOOR && *t >= INTO_DOOR {
                self.break_crates();
            }
        }
        for s in &mut self.shards {
            s.vy += 900.0 * dt;
            (s.x, s.y, s.age) = (s.x + s.vx * dt, s.y + s.vy * dt, s.age + dt);
        }
        self.shards.retain(|s| s.age < s.life);
        if !self.reduce_motion {
            let (w, h, time) = (self.canvas.w as f32, self.canvas.h as f32, self.time);
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

    /// How the climber stands now.
    fn pose(&self, s: &State) -> Pose {
        let mut p = Pose::facing(s.facing);
        p.refused = self.refused > 0.0;
        if self.reduce_motion {
            return p;
        }
        let (t, path) = (self.tween.t, self.tween.player);
        if t < path.across {
            let u = t / path.across;
            if path.from.1 > path.mid.1 + 0.01 {
                p.climb = (PI * u).sin();
            } else if (path.mid.0 - path.from.0).abs() > 0.01 {
                p.stride = (TAU * u).sin();
            }
        }
        p.squash = path.squash(t);
        p.breath = 0.6 * (self.time * 2.2).sin();
        p.blink = self.time % 3.7 < 0.12;
        let since = self.time - self.turned.0;
        if (0.0..0.4).contains(&since) {
            p.lean = self.turned.1 * (-since / 0.07).exp();
        }
        if let Some(w) = self.win {
            p.gone = (w / INTO_DOOR).min(1.0);
        }
        p
    }

    fn draw(&mut self) {
        let [fg, muted, accent] = self.colors;
        let s = *self.play.state();
        let moving = self.tween.block_moving();
        let broken = self.win.is_some_and(|t| t >= INTO_DOOR);
        let mut still = if self.intro.is_some() || broken { Bits::default() } else { s.blocks };
        if let Some((_, Some(i))) = moving {
            still.set(i, false);
        }
        let key = (still, self.colors, self.reduce_motion);
        if self.baked != Some(key) {
            self.bake(&still);
            self.baked = Some(key);
        }
        let (ex, ey) = (self.play.level.exit.0 as usize, self.play.level.exit.1 as usize);
        let t = self.tween.t;
        let pose = self.pose(&s);
        let gone = pose.gone >= 1.0;
        // The player (or the climber dropping in), a block in flight, the crates of the intro.
        let player_at = match &self.intro {
            Some(intro) if intro.t < intro.player.0 => None,
            Some(intro) => Some(intro.player.1.at(intro.t - intro.player.0)),
            None => Some(self.tween.player.at(t)),
        };
        let drops: Vec<(Pos, f32)> = match &self.intro {
            Some(intro) => intro
                .drops
                .iter()
                .filter(|(d, _)| intro.t >= *d)
                .map(|(d, p)| (p.at(intro.t - d), p.squash(intro.t - d)))
                .collect(),
            None => vec![],
        };
        let mut rects = vec![];
        if let Some((px, py)) = player_at.filter(|_| !gone) {
            rects.push(around(px, py - 1.0, 1.0, 2.0));
        }
        if let Some((path, _)) = moving {
            let (bx, by) = path.at(t);
            rects.push(around(bx, by, 1.0, 1.0));
        }
        rects.extend(drops.iter().map(|((x, y), _)| around(*x, *y, 1.0, 1.0)));
        let shimmer = (!self.reduce_motion).then_some(self.time);
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
            for ((x, y), squash) in &drops {
                art::crate_(c, *x, *y, *squash, accent);
            }
            if let Some((px, py)) = player_at.filter(|_| !gone) {
                match moving {
                    Some((path, _)) => {
                        let (bx, by) = path.at(t);
                        art::crate_(c, bx, by, path.squash(t), accent);
                    }
                    None if s.carrying => art::crate_(c, px, py - 1.0, 0.0, accent),
                    None => {}
                }
                art::climber(c, px, py, &pose, fg);
            }
            c.composite(GLOW.0, GLOW.1);
        }
        for e in &self.embers {
            if !self.reduce_motion {
                let twinkle = 0.65 + 0.35 * (self.time * 3.1 + e.phase * 7.0).sin();
                art::ember(c, e.x, e.y, e.depth, twinkle, accent);
            }
        }
        for sh in &self.shards {
            let fade = 1.0 - sh.age / sh.life;
            let half = (sh.size * (0.6 + 0.4 * fade)) / 2.0;
            let (x0, y0, x1, y1) =
                ((sh.x - half) as i32, (sh.y - half) as i32, (sh.x + half) as i32, (sh.y + half) as i32);
            c.speck(x0, y0, x1, y1, scale(accent, 0.12 * fade));
            for (a, b, cc, d) in
                [(x0, y0, x1, y0 + 2), (x0, y1 - 2, x1, y1), (x0, y0, x0 + 2, y1), (x1 - 2, y0, x1, y1)]
            {
                c.speck(a, b, cc, d, scale(accent, 1.1 * fade));
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

    /// Draws and bakes what stands still: the backdrop, walls, the doorway's frame (and its light, when it doesn't
    /// shimmer), and the crates in `still`.
    fn bake(&mut self, still: &Bits) {
        let [fg, muted, accent] = self.colors;
        let l = &self.play.level;
        let (w, h) = (l.w as usize, l.h as usize);
        let c = &mut self.canvas;
        c.clear([0.0; 3]);
        let wall = |x: i32, y: i32| {
            x < 0 || y < 0 || x >= w as i32 || y >= h as i32 || l.walls.get(y as usize * w + x as usize)
        };
        for y in 0..h {
            for x in 0..w {
                let (xi, yi) = (x as i32, y as i32);
                if wall(xi, yi) {
                    let open = [!wall(xi, yi - 1), !wall(xi, yi + 1), !wall(xi - 1, yi), !wall(xi + 1, yi)];
                    art::wall(c, x, y, open, muted);
                } else {
                    art::backdrop(c, x, y, h, accent);
                }
            }
        }
        let (ex, ey) = (l.exit.0 as usize, l.exit.1 as usize);
        art::door_frame(c, ex, ey, mix(accent, fg, 0.3));
        if self.reduce_motion {
            art::door_light(c, ex, ey, None, accent);
        }
        for i in (0..w * h).filter(|&i| still.get(i)) {
            art::crate_(c, (i % w) as f32, (i / w) as f32, 0.0, accent);
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
        let mut g = Game::new(1, 1).unwrap();
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
    fn a_level_opens_with_its_crates_dropping_in_and_a_key_skips_it() {
        let mut g = Game::new(1, 1).unwrap();
        assert!(g.intro.is_some());
        frames(&mut g, 3);
        assert_eq!(g.baked.unwrap().0, Bits::default(), "no crate stands still while they drop in");
        frames(&mut g, 120);
        assert!(g.intro.is_none(), "the intro ends by itself");
        assert_eq!(g.baked.unwrap().0, g.play.state().blocks, "then every crate is baked where it rests");
        g.restart();
        assert!(g.intro.is_some(), "restart replays it");
        let first = puzzle_core::solve(&g.play.level).solution.unwrap()[0];
        assert_eq!(g.key(first), None);
        assert!(g.intro.is_none() && g.play.moves() == 1, "a key skips the intro and still plays");
    }

    #[test]
    fn a_win_takes_the_climber_through_the_door_and_breaks_the_crates() {
        let mut g = Game::new(1, 1).unwrap();
        for k in puzzle_core::solve(&g.play.level).solution.unwrap() {
            g.key(k);
            frames(&mut g, 12);
        }
        frames(&mut g, 30);
        assert!(g.win.is_some_and(|t| t > INTO_DOOR));
        let crates =
            (0..g.play.level.w as usize * g.play.level.h as usize).filter(|&i| g.play.state().blocks.get(i)).count();
        assert_eq!(g.shards.len(), 4 * crates, "every crate breaks in four");
        assert!(g.pose(g.play.state()).gone >= 1.0, "the climber is through the door");
        g.undo();
        assert!(g.win.is_none() && g.shards.is_empty(), "undo puts the board back");
    }

    #[test]
    fn reduced_motion_snaps_and_throws_no_sparks_shake_or_shards() {
        let mut g = Game::new(1, 1).unwrap();
        g.reduce_motion = true;
        for k in puzzle_core::solve(&g.play.level).solution.unwrap() {
            g.key(k);
            g.frame(1.0 / 60.0);
        }
        frames(&mut g, 30);
        assert!(g.play.solved());
        assert_eq!(live_sparks(&g), 0);
        assert_eq!((g.shake, g.shards.len(), g.intro.is_some()), (0.0, 0, false));
    }
}
