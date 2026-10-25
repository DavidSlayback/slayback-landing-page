//! One level being played on the page: the rules from `puzzle-core`, short tweens so moves never snap, `sim` sparks
//! that bounce off the level's own walls and blocks, and the neon drawing (P18).

use puzzle_core::{Bits, Facing, Key, Level, Play, Refusal, State, TIERS, generate};
use sim::World;

use crate::draw::{Canvas, Rect, Rgb, mix, scale};

/// Pixels per cell.
pub const TILE: usize = 32;
const T: f32 = TILE as f32;
/// The spark world's obstacle mask has a cell every 4 pixels.
const MASK_SCALE: f32 = 0.25;
const SPARKS: usize = 600;

const WALK: f32 = 0.08;
const CLIMB: f32 = 0.09;
const LIFT: f32 = 0.09;

/// A position in cells (fractional while tweening).
type Pos = (f32, f32);

fn lerp(a: Pos, b: Pos, t: f32) -> Pos {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

/// How long a fall of `rows` cells takes: quick, and slower for longer drops so they read as falls.
fn fall_time(rows: f32) -> f32 {
    0.05 + 0.055 * rows.sqrt()
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
}

#[derive(Clone, Copy, PartialEq)]
enum Burst {
    Landing,
    Exit,
}

/// What is moving since the last key: the player, and a block on its way to or from the player's head.
struct Tween {
    t: f32,
    player: Path,
    /// A block in motion and the cell it ends in (hidden from the still blocks until it arrives); `None` for the
    /// block being lifted, which ends on the player's head.
    block: Option<(Path, Option<usize>)>,
    /// Sparks to throw when the tween ends, at a point in cells.
    then: Vec<(Burst, Pos)>,
}

impl Tween {
    fn done(&self) -> bool {
        self.t >= self.player.total().max(self.block.map_or(0.0, |b| b.0.total()))
    }
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
    /// What the still scene was last baked from: blocks at rest, colours, reduced motion.
    baked: Option<(Bits, [Rgb; 3], bool)>,
}

impl Game {
    pub fn new(tier: u8, seed: u64) -> Option<Game> {
        let g = generate(seed, TIERS.get(tier.checked_sub(1)? as usize)?)?;
        Some(Game::from_level(g.level, g.solution.len(), tier, seed))
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
            tween: Tween { t: 0.0, player: Path::still((start.x as f32, start.y as f32)), block: None, then: vec![] },
            sparks,
            rng: (seed as u32).wrapping_mul(2_654_435_761) | 1,
            time: 0.0,
            refused: 0.0,
            baked: None,
        };
        game.update_mask();
        game
    }

    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Plays one key: `None` if it played (or the level is already solved), else why it was refused.
    pub fn key(&mut self, key: Key) -> Option<Refusal> {
        let before = *self.play.state();
        let applied = self.play.apply(&format!("{key:?}"));
        if let Some(stop) = applied.stop {
            self.refused = 0.18;
            return Some(stop.why);
        }
        if applied.played == 1 {
            let after = *self.play.state();
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
    }

    fn snap(&mut self) {
        let s = *self.play.state();
        self.tween = Tween { t: 0.0, player: Path::still((s.x as f32, s.y as f32)), block: None, then: vec![] };
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
            Path::still(to) // a turn: the facing flips at once
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
                cell.map(|i| {
                    let rest = ((i % w) as f32, (i / w) as f32);
                    let from = (front, by - 1.0);
                    (Path { from, mid: from, to: rest, across: 0.0, fall: fall_time(rest.1 - from.1) }, Some(i))
                })
            }
            _ => None,
        };
        let mut then = vec![];
        if let Some((path, Some(_))) = block {
            then.push((Burst::Landing, (path.to.0 + 0.5, path.to.1 + 1.0)));
        }
        if self.play.solved() {
            then.push((Burst::Exit, (to.0 + 0.5, to.1 + 0.5)));
        }
        if self.reduce_motion {
            self.tween = Tween { t: 0.0, player: Path::still(to), block: None, then: vec![] };
        } else {
            self.tween = Tween { t: 0.0, player, block, then };
        }
    }

    /// The spark world's obstacles: walls and the blocks at rest.
    fn update_mask(&mut self) {
        let l = &self.play.level;
        let s = self.play.state();
        let (cw, ch) = (l.w as usize, l.h as usize);
        let (mw, mh) = ((cw * TILE) as f32 * MASK_SCALE, (ch * TILE) as f32 * MASK_SCALE);
        let (mw, mh) = (mw as usize, mh as usize);
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
        let live = (0..self.sparks.active).any(|i| self.sparks.life[i] > 0.0);
        if !live {
            self.sparks.active = 0;
        }
        let (x, y) = (at.0 * T, at.1 * T);
        let n = if kind == Burst::Exit { 140 } else { 36 };
        for _ in 0..n {
            let (a, b, c) = (self.rand(), self.rand(), self.rand());
            let (px, vx, vy, life) = match kind {
                Burst::Landing => {
                    // From the block's two bottom corners, just outside it: it is solid to the sparks.
                    let side = if a < 0.5 { -1.0 } else { 1.0 };
                    (x + side * (T * 0.5 + 2.0), side * (60.0 + 220.0 * b), -(40.0 + 200.0 * c), 0.3 + 0.45 * b)
                }
                Burst::Exit => {
                    let ang = a * std::f32::consts::TAU;
                    let speed = 90.0 + 320.0 * b;
                    (x, ang.cos() * speed, ang.sin() * speed - 80.0, 0.55 + 0.7 * c)
                }
            };
            if !self.sparks.put(px, y - 2.0, vx, vy) {
                break;
            }
            let i = self.sparks.active - 1;
            (self.sparks.life[i], self.sparks.energy[i]) = (life, 1.0);
        }
    }

    /// Advances `dt` seconds and draws the frame into `canvas.rgba`.
    pub fn frame(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.1);
        self.time += dt;
        self.refused = (self.refused - dt).max(0.0);
        self.tween.t += dt;
        if self.tween.done() && !self.tween.then.is_empty() {
            for (kind, at) in std::mem::take(&mut self.tween.then) {
                self.burst(kind, at);
            }
        }
        if (0..self.sparks.active).any(|i| self.sparks.life[i] > 0.0) {
            self.sparks.step(dt);
        }
        self.draw();
    }

    fn draw(&mut self) {
        let [fg, _, accent] = self.colors;
        let s = *self.play.state();
        let moving = self.tween.block.filter(|_| !self.tween.done());
        let mut still = s.blocks;
        if let Some((_, Some(i))) = moving {
            still.set(i, false);
        }
        let key = (still, self.colors, self.reduce_motion);
        if self.baked != Some(key) {
            self.bake(&still);
            self.baked = Some(key);
        }
        let (ex, ey) = (self.play.level.exit.0 as f32, self.play.level.exit.1 as f32);
        let t = self.tween.t;
        let (px, py) = self.tween.player.at(t);
        // Around the player (and the cell above, for a carried block), a block in flight, and the pulsing doorway.
        let mut rects = vec![around(px, py - 1.0, 1.0, 2.0)];
        if let Some((path, _)) = moving {
            let (bx, by) = path.at(t);
            rects.push(around(bx, by, 1.0, 1.0));
        }
        let pulse = (!self.reduce_motion).then(|| 0.75 + 0.25 * (self.time * 3.0).sin());
        if pulse.is_some() {
            rects.push(around(ex, ey, 1.0, 1.0));
        }
        let c = &mut self.canvas;
        c.begin();
        for r in merged(rects) {
            c.layer(r);
            if let Some(p) = pulse {
                door_light(c, ex, ey, accent, p);
            }
            match moving {
                Some((path, _)) => {
                    let (bx, by) = path.at(t);
                    block(c, bx, by, accent);
                }
                None if s.carrying => block(c, px, py - 1.0, accent),
                None => {}
            }
            player(c, px, py, s.facing, fg, self.refused > 0.0);
            c.composite(5, 1.5);
        }
        for i in 0..self.sparks.active {
            let life = self.sparks.life[i];
            if life <= 0.0 {
                continue;
            }
            let heat = (1.0 - self.sparks.age[i] / life).clamp(0.0, 1.0);
            let hot = mix(accent, [1.0, 1.0, 1.0], heat * heat);
            c.spark(self.sparks.x[i], self.sparks.y[i], 1.1, scale(hot, (0.6 + self.sparks.energy[i]) * heat * 1.6));
        }
    }

    /// Draws and bakes what stands still: walls, the doorway's frame (and its light, when it doesn't pulse), and the
    /// blocks in `still`.
    fn bake(&mut self, still: &Bits) {
        let [fg, muted, accent] = self.colors;
        let l = &self.play.level;
        let (w, h) = (l.w as usize, l.h as usize);
        let c = &mut self.canvas;
        c.clear([0.004, 0.005, 0.007]);
        let wall = |x: i32, y: i32| {
            x < 0 || y < 0 || x >= w as i32 || y >= h as i32 || l.walls.get(y as usize * w + x as usize)
        };
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                let (x0, y0) = (x * TILE as i32, y * TILE as i32);
                let (x1, y1) = (x0 + TILE as i32, y0 + TILE as i32);
                if wall(x, y) {
                    // Walls read as one mass: a dim body, lit only along the edges that face open space.
                    c.fill(x0, y0, x1, y1, scale(muted, 0.07));
                    let edge = scale(muted, 0.55);
                    if !wall(x, y - 1) {
                        c.fill(x0, y0, x1, y0 + 2, edge);
                    }
                    if !wall(x, y + 1) {
                        c.fill(x0, y1 - 2, x1, y1, edge);
                    }
                    if !wall(x - 1, y) {
                        c.fill(x0, y0, x0 + 2, y1, edge);
                    }
                    if !wall(x + 1, y) {
                        c.fill(x1 - 2, y0, x1, y1, edge);
                    }
                }
            }
        }
        // The exit: an open doorway (posts and a lintel, no sill) full of light, which pulses unless motion is reduced.
        let door = mix(accent, fg, 0.3);
        let (ex, ey) = (l.exit.0 as i32 * TILE as i32, l.exit.1 as i32 * TILE as i32);
        c.fill(ex + 5, ey + 2, ex + 27, ey + 5, scale(door, 1.1));
        c.fill(ex + 5, ey + 5, ex + 8, ey + 32, scale(door, 1.1));
        c.fill(ex + 24, ey + 5, ex + 27, ey + 32, scale(door, 1.1));
        if self.reduce_motion {
            door_light(c, l.exit.0 as f32, l.exit.1 as f32, accent, 1.0);
        }
        for i in (0..w * h).filter(|&i| still.get(i)) {
            block(c, (i % w) as f32, (i / w) as f32, accent);
        }
        c.bake(5, 1.5);
    }
}

/// The light inside the doorway at cell (`x`, `y`), at `pulse` strength.
fn door_light(c: &mut Canvas, x: f32, y: f32, accent: Rgb, pulse: f32) {
    let (x0, y0) = ((x * T) as i32, (y * T) as i32);
    c.fill(x0 + 8, y0 + 5, x0 + 24, y0 + 32, scale(accent, 0.28 * pulse));
}

/// The pixels of `w` × `h` cells from cell (`x`, `y`), with room around them for glow.
fn around(x: f32, y: f32, w: f32, h: f32) -> Rect {
    const ROOM: f32 = 14.0;
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

/// A block: an outlined square with a smaller one inside, so it reads apart from walls by shape as well as colour.
fn block(c: &mut Canvas, x: f32, y: f32, accent: Rgb) {
    let (x0, y0) = ((x * T).round() as i32, (y * T).round() as i32);
    let t = TILE as i32;
    c.fill(x0 + 3, y0 + 3, x0 + t - 3, y0 + t - 3, scale(accent, 0.07));
    c.frame(x0 + 3, y0 + 3, x0 + t - 3, y0 + t - 3, 2, scale(accent, 0.95));
    c.frame(x0 + 11, y0 + 11, x0 + t - 11, y0 + t - 11, 2, scale(accent, 0.5));
}

/// The player: a figure (a round head over an outlined body) with a dark eye on the side it faces. Flashes warm when a
/// key is refused.
fn player(c: &mut Canvas, x: f32, y: f32, facing: Facing, fg: Rgb, refused: bool) {
    let (x0, y0) = ((x * T).round() as i32, (y * T).round() as i32);
    let (fx, fy) = (x0 as f32, y0 as f32);
    let body = if refused { [1.0, 0.35, 0.25] } else { fg };
    c.disc(fx + 16.0, fy + 8.5, 6.0, scale(body, 1.2));
    let eye = if facing == Facing::Left { 13.0 } else { 19.0 };
    c.disc(fx + eye, fy + 8.0, 1.6, scale(body, -1.0));
    c.fill(x0 + 10, y0 + 16, x0 + 22, y0 + 31, scale(body, 0.16));
    c.frame(x0 + 10, y0 + 16, x0 + 22, y0 + 31, 2, scale(body, 1.1));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(s: &str) -> Vec<Key> {
        s.chars().filter_map(Key::from_char).collect()
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
            for _ in 0..12 {
                g.frame(1.0 / 60.0);
            }
        }
        assert!(g.play.solved());
        g.frame(1.0 / 60.0);
        let live = (0..g.sparks.active).filter(|&i| g.sparks.life[i] > 0.0).count();
        assert!(live > 50, "{live} sparks after the exit");
    }

    #[test]
    fn walls_blocks_player_and_exit_are_drawn_and_open_space_stays_dark() {
        let level = Level::parse("#.....#\n#>.B.E#\n#######").unwrap();
        let mut g = Game::from_level(level, 4, 1, 0);
        g.frame(0.0);
        assert!(lit(&g, 1, 1) > 0.5, "player");
        assert!(lit(&g, 3, 1) > 0.5, "block");
        assert!(lit(&g, 5, 1) > 0.4, "exit");
        assert!(lit(&g, 3, 2) > 0.2, "the wall under the block shows its top edge");
        assert!(lit(&g, 2, 0) < 0.15, "open space far from everything");
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
    fn putting_a_block_down_throws_sparks_where_it_lands() {
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
        let far: Vec<_> = live.iter().filter(|&&i| !near(i)).map(|&i| (g.sparks.x[i], g.sparks.y[i])).collect();
        assert!(
            far.is_empty(),
            "sparks start at the foot of the landed block: {} of {} not: {far:?}",
            far.len(),
            live.len()
        );
    }

    #[test]
    fn only_a_block_coming_to_rest_redraws_the_still_scene() {
        // A full redraw every frame cost a phone ~45 ms; walking and waiting must only redraw what moves.
        let level = Level::parse("#B......#\n#>.....E#\n#########").unwrap();
        let mut g = Game::from_level(level, 0, 1, 0);
        let frames = |g: &mut Game, n| (0..n).for_each(|_| g.frame(1.0 / 60.0));
        frames(&mut g, 30);
        assert_eq!(g.canvas.bakes, 1, "the first frame bakes, waiting doesn't");
        g.key(Key::R);
        frames(&mut g, 30);
        assert_eq!(g.canvas.bakes, 1, "walking doesn't");
        g.key(Key::D);
        frames(&mut g, 30);
        assert_eq!(g.canvas.bakes, 2, "the block put down bakes once, when it lands");
        g.undo();
        frames(&mut g, 5);
        assert_eq!(g.canvas.bakes, 3, "undo brings the block back: once more");
    }

    #[test]
    fn reduced_motion_snaps_and_throws_no_sparks() {
        let mut g = Game::new(1, 1).unwrap();
        g.reduce_motion = true;
        for k in puzzle_core::solve(&g.play.level).solution.unwrap() {
            g.key(k);
            g.frame(1.0 / 60.0);
        }
        assert!(g.play.solved());
        assert_eq!((0..g.sparks.active).filter(|&i| g.sparks.life[i] > 0.0).count(), 0);
    }
}
