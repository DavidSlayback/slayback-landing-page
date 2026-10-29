//! Every timeline of a level being played, in one place: where the climber and each moving crate are drawn now, how the
//! climber stands, and what falls due as time passes (a landing, dust, the exit, the crates breaking). `Game` keeps the
//! effects those set off (sparks, shards, embers, the shake) and draws.
//!
//! The timelines: the intro, crates dropping in; a key's tween (a walk, a climb, a lift, a set-down) or, when the move
//! plays out over several rounds (ice sliding, a bounce), its trace from the rules' own rounds; the win, into the door
//! and then the crates break; the lean into a turn; and the flash of a refused key. Reduced motion is decided here once:
//! every answer is the end state and nothing falls due, so nothing moves, flies or shakes.

use std::f32::consts::{PI, TAU};

use puzzle_core::{Bits, Facing, Level, Play, Refusal, Round, State};

use crate::art::Pose;

const WALK: f32 = 0.08;
const CLIMB: f32 = 0.09;
const LIFT: f32 = 0.09;
/// How long a landing squashes.
const SQUASH: f32 = 0.14;
/// A win: the climber shrinks into the doorway, then the crates break.
const INTO_DOOR: f32 = 0.35;
/// How long a refused key flashes the climber.
const REFUSED: f32 = 0.18;
/// A trace: seconds for the key's own action (a walk, a lift), then for each round after it.
const FIRST: f32 = 0.09;
const ROUND: f32 = 0.055;

/// A position in cells (fractional while moving).
pub type Pos = (f32, f32);

fn lerp(a: Pos, b: Pos, t: f32) -> Pos {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

/// How long a fall of `rows` cells takes: quick, and slower for longer drops so they read as falls.
fn fall_time(rows: f32) -> f32 {
    0.05 + 0.055 * rows.max(0.0).sqrt()
}

/// What falls due as motion plays out, at a point in cells.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// A block lands, or a bounce leaves its spring: sparks from its foot and a shake.
    Landing(Pos),
    /// Something comes to rest: a puff of dust at its foot.
    Dust(Pos),
    /// The climber reaches the doorway, from its middle: the win starts.
    Exit(Pos),
    /// A cracked tile the move broke, at its cell, falls apart (at once, with the key).
    Crumble(Pos),
    /// The climber is through the door: every crate at rest breaks apart.
    Break,
}

/// A crate drawn where it is now rather than baked with the still scene.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crate {
    pub at: Pos,
    /// Squashed by a landing (> 0) or stretched by a fall (< 0).
    pub squash: f32,
    pub ice: bool,
}

/// The climber as drawn now: its feet's cell, its pose, and the block on its head (whether ice) when it carries one
/// that isn't among the moving crates.
#[derive(Clone, Copy, Debug)]
pub struct Climber {
    pub at: Pos,
    pub pose: Pose,
    pub carrying: Option<bool>,
}

/// Everything that moves, at this moment.
#[derive(Clone, Debug)]
pub struct Scene {
    /// The crates standing still, baked with the backdrop.
    pub still: Bits,
    /// The crates drawn where they are now, in drawing order.
    pub crates: Vec<Crate>,
    /// None before the climber drops in and once it is through the door.
    pub climber: Option<Climber>,
}

/// A two-part path: across (or up) to `mid`, then falling to `to`, eased in like gravity.
#[derive(Clone, Copy, Debug)]
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

/// A block between two rounds: from cell, to cell, and whether it is ice.
type Move = (usize, usize, bool);

/// A move that plays out over several rounds: the rules' state after each round and every block's way there
/// (`puzzle_core::Play::key`), so blocks glide instead of snapping.
#[derive(Debug)]
struct Trace {
    /// The state before the key, after it acts, then after each round.
    ticks: Vec<State>,
    /// What moved between each tick and the next.
    moves: Vec<Vec<Move>>,
}

impl Trace {
    fn new(before: State, rounds: &[Round]) -> Trace {
        let ticks = std::iter::once(before).chain(rounds.iter().map(|r| r.state)).collect();
        let moves = rounds.iter().map(|r| r.moved.iter().map(|m| (m.from, m.to, m.ice)).collect()).collect();
        Trace { ticks, moves }
    }

    /// Whether anything moves after the key's own action in a way a tween can't draw: a block sliding or bouncing, or
    /// you bouncing.
    fn needed(&self, w: usize) -> bool {
        let block = self.moves.iter().skip(1).flatten().any(|&(from, to, _)| from != to && to != from + w);
        let me = self.ticks.windows(2).skip(1).any(|p| p[1].y < p[0].y || p[1].x != p[0].x);
        block || me
    }

    /// Whether you go up in step `k` without having gone up in the step before: a bounce leaving its spring.
    fn launches(&self, k: usize) -> bool {
        let up = |k: usize| self.ticks[k + 1].y < self.ticks[k].y;
        k > 0 && up(k) && !up(k - 1)
    }

    /// Whether you go up in step `k`.
    fn rising(&self, k: usize) -> bool {
        self.ticks[k + 1].y < self.ticks[k].y
    }

    fn end(&self) -> f32 {
        FIRST + ROUND * (self.ticks.len() as f32 - 2.0).max(0.0)
    }

    /// Which step of the trace `t` falls in, and how far through it (0 to 1).
    fn at(&self, t: f32) -> (usize, f32) {
        if t < FIRST {
            return (0, t / FIRST);
        }
        let k = 1 + ((t - FIRST) / ROUND) as usize;
        if k >= self.moves.len() {
            return (self.moves.len() - 1, 1.0);
        }
        (k, (t - FIRST - ROUND * (k - 1) as f32) / ROUND)
    }

    /// The blocks that fall in step `k` and stay put in step `k + 1`, by the cell they land in, for landing sparks.
    fn lands(&self, k: usize, w: usize) -> Vec<usize> {
        let fell = |m: &Move| m.1 == m.0 + w;
        self.moves[k]
            .iter()
            .filter(|m| fell(m))
            .filter(|m| self.moves.get(k + 1).is_none_or(|next| next.iter().any(|n| n.0 == m.1 && n.1 == m.1)))
            .map(|m| m.1)
            .collect()
    }
}

/// The level opening: crates drop into place bottom first, column by column, then the climber. Each drop waits its
/// delay in seconds.
#[derive(Debug)]
struct Intro {
    drops: Vec<(f32, Path)>,
    player: (f32, Path),
}

impl Intro {
    fn end(&self) -> f32 {
        let last = self.drops.iter().chain([&self.player]).map(|(d, p)| d + p.total()).fold(0.0, f32::max);
        last + SQUASH
    }
}

#[derive(Debug)]
pub struct Motion {
    w: usize,
    h: usize,
    reduce: bool,
    /// Whether restarting replays the intro (the page's games do; tests' don't).
    replay: bool,
    /// Seconds since the game began: the breath, the blink, the lean.
    time: f32,
    /// Seconds since the last key, intro, undo or restart: every path below runs on it.
    t: f32,
    /// What is left of a refused key's flash.
    refused: f32,
    /// A turn: when, and which way, for the lean.
    turned: (f32, f32),
    player: Path,
    /// A block in motion and the cell it ends in (hidden from the still blocks until it has landed and squashed);
    /// `None` for the block being lifted, which ends on the player's head.
    block: Option<(Path, Option<usize>)>,
    /// What falls due when, in seconds on `t`.
    events: Vec<(f32, Event)>,
    /// A move drawn round by round from the rules' own rounds instead of the tween.
    trace: Option<Trace>,
    intro: Option<Intro>,
    /// Seconds since the climber reached the doorway.
    win: Option<f32>,
}

impl Motion {
    /// Still, with the climber where `level` starts.
    pub fn new(level: &Level) -> Motion {
        let s = level.start;
        Motion {
            w: level.w as usize,
            h: level.h as usize,
            reduce: false,
            replay: false,
            time: 0.0,
            t: 0.0,
            refused: 0.0,
            turned: (-1.0, 0.0),
            player: Path::still((s.x as f32, s.y as f32)),
            block: None,
            events: vec![],
            trace: None,
            intro: None,
            win: None,
        }
    }

    /// Reduced motion, decided once, before the first frame: from then on every answer is the end state and nothing
    /// falls due. A refused key still flashes; that is colour, not motion.
    pub fn reduce(&mut self, on: bool) {
        self.reduce = on;
        if on {
            self.intro = None;
            self.events.clear();
        }
    }

    pub fn reduced(&self) -> bool {
        self.reduce
    }

    /// Seconds since the game began, for what idles: the breath, the doorway's shimmer, the embers.
    pub fn time(&self) -> f32 {
        self.time
    }

    /// Whether the level is still opening with its intro.
    pub fn opening(&self) -> bool {
        self.intro.is_some()
    }

    /// Drops the crates of `s` in, bottom first, column by column, then the climber; restarting replays it.
    pub fn start_intro(&mut self, s: &State) {
        self.replay = true;
        if self.reduce {
            return;
        }
        let (w, h) = (self.w, self.h);
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
        self.t = 0.0;
        self.intro = Some(Intro { drops, player });
    }

    /// Everything at rest in `s`, at once: after an undo.
    pub fn snap(&mut self, s: &State) {
        self.t = 0.0;
        self.player = Path::still((s.x as f32, s.y as f32));
        self.block = None;
        self.events.clear();
        self.intro = None;
        self.win = None;
        self.trace = None;
    }

    /// Back to the start, `s`, replaying the intro if the game opened with one.
    pub fn restart(&mut self, s: &State) {
        self.snap(s);
        if self.replay {
            self.start_intro(s);
        }
    }

    /// A key, as `play` played it from `before`: the rounds it played out in, or why it was refused. It skips the rest
    /// of the intro and starts what the move looks like; the cracked tiles it broke fall due at once.
    pub fn key(&mut self, play: &Play, before: &State, played: Result<&[Round], &Refusal>) -> Vec<Event> {
        self.intro = None;
        let rounds = match played {
            Ok(rounds) if !rounds.is_empty() => rounds,
            Ok(_) => return vec![], // already solved: nothing played
            Err(_) => {
                self.refused = REFUSED;
                return vec![];
            }
        };
        let after = *play.state();
        if after.facing != before.facing && (after.x, after.y) == (before.x, before.y) {
            self.turned = (self.time, if after.facing == Facing::Left { -1.0 } else { 1.0 });
        }
        let trace = Trace::new(*before, rounds);
        self.trace = None;
        if trace.needed(self.w) && !self.reduce {
            self.follow(trace, play.solved());
        } else {
            self.animate(before, &after, rounds, play.solved());
        }
        if self.reduce {
            return vec![];
        }
        let w = self.w;
        (0..w * self.h)
            .filter(|&i| before.cracked.get(i) && !after.cracked.get(i))
            .map(|i| Event::Crumble(((i % w) as f32, (i / w) as f32)))
            .collect()
    }

    /// Draws the move from its trace: blocks glide round by round, landings throw sparks, sliding ice throws dust where
    /// it stops.
    fn follow(&mut self, trace: Trace, solved: bool) {
        let w = self.w;
        let mut events = vec![];
        let end = |k: usize| FIRST + ROUND * k as f32;
        let at = |c: usize| ((c % w) as f32 + 0.5, (c / w) as f32 + 1.0);
        for k in 0..trace.moves.len() {
            if trace.launches(k) {
                let s = &trace.ticks[k];
                events.push((end(k - 1), Event::Landing((s.x as f32 + 0.5, s.y as f32 + 1.0))));
            }
            for c in trace.lands(k, w) {
                events.push((end(k), Event::Landing(at(c))));
            }
            let stops = trace.moves[k]
                .iter()
                .filter(|m| m.0 / w == m.1 / w && m.0 != m.1)
                .filter(|m| trace.moves.get(k + 1).is_none_or(|next| next.iter().any(|n| n.0 == m.1 && n.1 == m.1)));
            events.extend(stops.map(|m| (end(k), Event::Dust(at(m.1)))).collect::<Vec<_>>());
        }
        let s = trace.ticks.last().unwrap();
        if solved {
            events.push((trace.end(), Event::Exit((s.x as f32 + 0.5, s.y as f32 + 0.5))));
        }
        (self.t, self.player, self.block, self.events) = (0.0, Path::still((s.x as f32, s.y as f32)), None, events);
        self.trace = Some(trace);
    }

    /// Sets up the tween from `before` to `after` (`rounds` is how the key played out), starting wherever the player
    /// is drawn now.
    fn animate(&mut self, before: &State, after: &State, rounds: &[Round], solved: bool) {
        let now = self.player.at(self.t);
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
        let w = self.w;
        let block = match (before.carrying, after.carrying) {
            (false, true) => Some((
                Path { from: (front, by), mid: (bx, by - 1.0), to: (bx, by - 1.0), across: LIFT, fall: 0.0 },
                None,
            )),
            (true, false) => {
                // The carried block, followed through the rounds to where it comes to rest.
                let head = before.y as usize * w + before.x as usize - w;
                let k = rounds[0].moved.iter().position(|m| m.from == head);
                let cell = k.and_then(|k| rounds.last().map(|r| r.moved[k].to));
                cell.map(|i| (Path::drop((front, by - 1.0), ((i % w) as f32, (i / w) as f32)), Some(i)))
            }
            _ => None,
        };
        let mut events = vec![];
        if player.fall > 0.0 {
            events.push((player.total(), Event::Dust((to.0 + 0.5, to.1 + 1.0))));
        }
        if let Some((path, Some(_))) = block {
            events.push((path.total(), Event::Landing((path.to.0 + 0.5, path.to.1 + 1.0))));
            events.push((path.total(), Event::Dust((path.to.0 + 0.5, path.to.1 + 1.0))));
        }
        if solved {
            events.push((player.total(), Event::Exit((to.0 + 0.5, to.1 + 0.5))));
        }
        self.t = 0.0;
        (self.player, self.block, self.events) =
            if self.reduce { (Path::still(to), None, vec![]) } else { (player, block, events) };
    }

    /// Advances `dt` seconds: what falls due in them, in order. A win's crates break once the climber is through the
    /// door.
    pub fn advance(&mut self, dt: f32) -> Vec<Event> {
        self.time += dt;
        self.refused = (self.refused - dt).max(0.0);
        self.t += dt;
        if self.intro.as_ref().is_some_and(|intro| self.t >= intro.end()) {
            self.intro = None;
        }
        if self.trace.as_ref().is_some_and(|tr| self.t >= tr.end() + SQUASH) {
            self.trace = None;
        }
        let t = self.t;
        let mut due: Vec<Event> = self.events.iter().filter(|e| e.0 <= t).map(|e| e.1).collect();
        self.events.retain(|e| e.0 > t);
        if due.iter().any(|e| matches!(e, Event::Exit(_))) {
            self.win = Some(0.0);
        }
        if let Some(w) = &mut self.win {
            let before = *w;
            *w += dt;
            if before < INTO_DOOR && *w >= INTO_DOOR {
                due.push(Event::Break);
            }
        }
        due
    }

    /// The moving block while it still moves or squashes.
    fn block_moving(&self) -> Option<(Path, Option<usize>)> {
        self.block.filter(|(p, _)| self.t < p.total() + if p.fall > 0.0 { SQUASH } else { 0.0 })
    }

    /// How the climber stands now, facing as in `s`.
    fn pose(&self, s: &State) -> Pose {
        let mut p = Pose::facing(s.facing);
        p.refused = self.refused > 0.0;
        if self.reduce {
            return p;
        }
        let (t, path) = (self.t, self.player);
        p.squash = path.squash(t);
        if let Some(tr) = &self.trace {
            let (k, u) = tr.at(t);
            if tr.rising(k) {
                p.squash = -0.3; // stretched on the way up a bounce
            } else if tr.ticks[k].x != tr.ticks[k + 1].x && u < 1.0 {
                p.stride = (TAU * u).sin();
            }
        } else if t < path.across {
            let u = t / path.across;
            if path.from.1 > path.mid.1 + 0.01 {
                p.climb = (PI * u).sin();
            } else if (path.mid.0 - path.from.0).abs() > 0.01 {
                p.stride = (TAU * u).sin();
            }
        }
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

    /// What is where now, with `s` the rules' state.
    pub fn scene(&self, s: &State) -> Scene {
        let (w, t) = (self.w, self.t);
        let moving = self.block_moving();
        let broken = self.win.is_some_and(|w| w >= INTO_DOOR);
        let mut still = if self.intro.is_some() || broken { Bits::default() } else { s.blocks };
        if let Some((_, Some(i))) = moving {
            still.set(i, false);
        }
        let mut crates = vec![];
        let mut traced = None;
        if let Some(tr) = &self.trace {
            // What never moves during the trace stays baked; the rest is drawn where the trace has it now.
            still = tr.ticks.iter().fold(still, |a, t| a.zip_and(t.blocks));
            let (k, u) = tr.at(t);
            let cell = |c: usize| ((c % w) as f32, (c / w) as f32);
            for &(from, to, ice) in &tr.moves[k] {
                if from != to || !still.get(to) {
                    crates.push(Crate { at: lerp(cell(from), cell(to), u), squash: 0.0, ice });
                }
            }
            let (a, b) = (&tr.ticks[k], &tr.ticks[k + 1]);
            traced = Some(lerp((a.x as f32, a.y as f32), (b.x as f32, b.y as f32), u));
        }
        let at = match &self.intro {
            Some(intro) if t < intro.player.0 => None,
            Some(intro) => Some(intro.player.1.at(t - intro.player.0)),
            None => Some(traced.unwrap_or_else(|| self.player.at(t))),
        };
        if let Some(intro) = &self.intro {
            for (d, p) in intro.drops.iter().filter(|(d, _)| t >= *d) {
                let ((x, y), squash) = (p.at(t - d), p.squash(t - d));
                crates.push(Crate { at: (x, y), squash, ice: s.ice.get(y.round().max(0.0) as usize * w + x as usize) });
            }
        }
        if let Some((path, cell)) = moving {
            let ice = cell.map_or(s.carrying_ice, |i| s.ice.get(i));
            crates.push(Crate { at: path.at(t), squash: path.squash(t), ice });
        }
        let pose = self.pose(s);
        let carrying = (traced.is_none() && moving.is_none() && s.carrying).then_some(s.carrying_ice);
        let climber = at.filter(|_| pose.gone < 1.0).map(|at| Climber { at, pose, carrying });
        Scene { still, crates, climber }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use puzzle_core::Key;

    const DT: f32 = 1.0 / 60.0;

    /// A level, its rules and its motion, still.
    fn open(text: &str) -> (Play, Motion) {
        let level = Level::parse(text).unwrap();
        let motion = Motion::new(&level);
        (Play::new(level), motion)
    }

    fn press(play: &mut Play, motion: &mut Motion, key: Key) -> Vec<Event> {
        let before = *play.state();
        let played = play.key(key);
        motion.key(play, &before, played.as_deref())
    }

    /// Every event that falls due in `secs`, with when.
    fn run(motion: &mut Motion, secs: f32) -> Vec<(f32, Event)> {
        let mut out = vec![];
        for n in 1..=(secs / DT) as usize {
            out.extend(motion.advance(DT).into_iter().map(|e| (n as f32 * DT, e)));
        }
        out
    }

    #[test]
    fn a_landing_falls_due_at_the_blocks_foot_when_its_drop_ends() {
        let (mut play, mut m) = open("#B.....#\n#>...E.#\n########");
        assert!(press(&mut play, &mut m, Key::D).is_empty());
        // Put down from head height in front, (2, 0), it drops a row to rest on the floor at (2, 1).
        let drop = fall_time(1.0);
        let s = *play.state();
        assert_eq!(m.scene(&s).crates.len(), 1, "on its way, the block is drawn moving");
        assert!(!m.scene(&s).still.get(10), "and not baked");
        let events = run(&mut m, 1.0);
        let landing = events.iter().find(|e| matches!(e.1, Event::Landing(_))).unwrap();
        assert_eq!(landing.1, Event::Landing((2.5, 2.0)), "at the foot of the landed block");
        assert!(landing.0 >= drop && landing.0 < drop + DT, "the frame its drop ends: {} for {drop}", landing.0);
        assert!(events.contains(&(landing.0, Event::Dust((2.5, 2.0)))), "with dust");
        assert!(m.scene(&s).crates.is_empty() && m.scene(&s).still.get(10), "then it rests, baked");
    }

    #[test]
    fn a_key_skips_the_intro() {
        let (mut play, mut m) = open("#.......#\n#.B.B...#\n#>B.B..E#\n#########");
        let s = *play.state();
        m.start_intro(&s);
        assert!(m.opening());
        assert_eq!(m.scene(&s).still, Bits::default(), "no crate stands still while they drop in");
        assert!(m.scene(&s).climber.is_none(), "the climber drops in last");
        run(&mut m, 0.1);
        assert!(m.opening());
        press(&mut play, &mut m, Key::L);
        let s = *play.state();
        assert!(!m.opening());
        let scene = m.scene(&s);
        assert!(scene.crates.is_empty() && scene.still == s.blocks, "every crate where it rests");
        assert_eq!(scene.climber.map(|c| c.at), Some((1.0, 2.0)));
    }

    #[test]
    fn the_intro_ends_by_itself_and_restart_replays_it() {
        let (play, mut m) = open("#.......#\n#.B.B...#\n#>B.B..E#\n#########");
        let s = *play.state();
        m.start_intro(&s);
        assert!(run(&mut m, 2.0).is_empty(), "nothing falls due in an intro");
        assert!(!m.opening());
        assert_eq!(m.scene(&s).still, s.blocks);
        m.restart(&s);
        assert!(m.opening());
    }

    #[test]
    fn under_reduced_motion_every_answer_is_the_end_state() {
        // A set-down ice block slides, a walk off a ledge falls, a cracked tile breaks and the climber wins.
        let (mut play, mut m) = open("#I.......#\n#>.......#\n#~###..E.#\n##########");
        m.reduce(true);
        let s = *play.state();
        m.start_intro(&s);
        assert!(!m.opening(), "no intro");
        for key in [Key::D, Key::R, Key::R, Key::R, Key::R, Key::R, Key::R] {
            assert!(press(&mut play, &mut m, key).is_empty(), "nothing falls due, not even a tile breaking");
            let s = *play.state();
            let scene = m.scene(&s);
            assert!(scene.crates.is_empty() && scene.still == s.blocks, "every crate where the rules put it");
            let me = scene.climber.unwrap();
            assert_eq!(me.at, (s.x as f32, s.y as f32), "the climber where the rules put it");
            let p = me.pose;
            assert_eq!((p.stride, p.climb, p.squash, p.lean, p.breath, p.gone), (0.0, 0.0, 0.0, 0.0, 0.0, 0.0));
            assert!(run(&mut m, 0.5).is_empty());
        }
        assert!(play.solved());
        assert!(m.scene(play.state()).climber.is_some(), "a win leaves the climber in the doorway");
    }

    #[test]
    fn a_refused_key_flashes_the_climber_even_under_reduced_motion() {
        for reduce in [false, true] {
            let (mut play, mut m) = open("#.....#\n#>#..E#\n#######");
            m.reduce(reduce);
            assert!(press(&mut play, &mut m, Key::D).is_empty());
            assert!(m.scene(play.state()).climber.unwrap().pose.refused);
            run(&mut m, REFUSED + DT);
            assert!(!m.scene(play.state()).climber.unwrap().pose.refused, "and it passes");
        }
    }

    /// Plays `key` on `text` to the end: the dust that says where the block came to rest, and the final scene.
    fn set_down(text: &str) -> (bool, Event, Scene, State) {
        let (mut play, mut m) = open(text);
        press(&mut play, &mut m, Key::D);
        let traced = m.trace.is_some();
        let dust = run(&mut m, 2.0).into_iter().rev().find(|e| matches!(e.1, Event::Dust(_))).unwrap().1;
        let s = *play.state();
        (traced, dust, m.scene(&s), s)
    }

    #[test]
    fn a_trace_and_a_tween_answer_the_same_questions() {
        // A crate set down drops one row (a tween); ice set down drops and slides to the wall (a trace).
        let (crate_traced, crate_dust, crate_end, crate_s) = set_down("#B.....E#\n#>......#\n#########");
        let (ice_traced, ice_dust, ice_end, ice_s) = set_down("#I.....E#\n#>......#\n#########");
        assert!(!crate_traced && ice_traced);
        assert_eq!(crate_dust, Event::Dust((2.5, 2.0)), "dust at the foot of where the rules put it");
        assert_eq!(ice_dust, Event::Dust((7.5, 2.0)), "dust at the foot of where the rules put it");
        for (end, s) in [(crate_end, crate_s), (ice_end, ice_s)] {
            assert!(end.crates.is_empty() && end.still == s.blocks, "at rest where the rules put it");
            assert_eq!(end.climber.map(|c| c.at), Some((1.0, 1.0)));
        }
    }

    #[test]
    fn set_down_ice_is_drawn_round_by_round() {
        let (mut play, mut m) = open("#I.....E#\n#>......#\n#########");
        press(&mut play, &mut m, Key::D);
        // Before, set down, a round that drops it and slides it a cell, then four more cells to the wall.
        assert_eq!(m.trace.as_ref().unwrap().ticks.len(), 7);
        run(&mut m, 0.1);
        let at = m.scene(play.state()).crates[0].at;
        assert!(at.0 > 2.0 && at.0 < 7.0, "mid-slide: {at:?}");
        run(&mut m, 1.0);
        assert!(m.trace.is_none(), "the trace ends");
    }

    #[test]
    fn a_bounce_is_drawn_from_its_trace_with_a_landing_off_the_spring() {
        let (mut play, mut m) = open("#>.....#\n##.#...#\n#..#..E#\n##^#####");
        press(&mut play, &mut m, Key::R);
        assert!(m.trace.is_some());
        let events = run(&mut m, 1.0);
        assert!(events.iter().any(|e| matches!(e.1, Event::Landing(_))), "{events:?}");
        assert_eq!(m.scene(play.state()).climber.map(|c| c.at), Some((3.0, 0.0)));
    }

    #[test]
    fn the_climber_stretches_on_the_way_up_a_bounce() {
        let (mut play, mut m) = open("#>.....#\n##.#...#\n#..#..E#\n##^#####");
        press(&mut play, &mut m, Key::R);
        let mut stretched = false;
        for _ in 0..60 {
            m.advance(DT);
            let pose = m.scene(play.state()).climber.unwrap().pose;
            stretched |= pose.squash < 0.0;
        }
        assert!(stretched, "the climber never stretched while the bounce rose");
    }

    #[test]
    fn a_win_breaks_the_crates_once_the_climber_is_through_the_door() {
        let (mut play, mut m) = open("#>E#\n####");
        press(&mut play, &mut m, Key::R);
        let events = run(&mut m, 1.0);
        let kinds: Vec<_> = events.iter().map(|e| e.1).collect();
        assert_eq!(kinds, [Event::Exit((2.5, 0.5)), Event::Break]);
        assert!(events[1].0 - events[0].0 >= INTO_DOOR - DT, "into the door first");
        assert!(m.scene(play.state()).climber.is_none(), "gone");
        m.snap(play.state());
        assert!(m.scene(play.state()).climber.is_some(), "a snap brings it back");
    }

    #[test]
    fn a_turn_leans_and_the_lean_dies_away() {
        let (mut play, mut m) = open("#>..E#\n######");
        run(&mut m, 0.5);
        press(&mut play, &mut m, Key::L);
        let lean = |m: &Motion, play: &Play| m.scene(play.state()).climber.unwrap().pose.lean;
        assert!(lean(&m, &play) < -0.9, "into the turn, to the left");
        run(&mut m, 0.5);
        assert_eq!(lean(&m, &play), 0.0);
    }
}
