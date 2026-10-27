//! What things look like (P25, neon arcade): the backdrop, brick walls with lit and bevelled faces, crates, the
//! doorway, and the climber in any pose. Positions are in cells; [`TILE`] pixels to a cell.

use puzzle_core::Facing;

use crate::draw::{Canvas, Rgb, mix, scale};

/// Pixels per cell.
pub const TILE: usize = 32;
pub const T: f32 = TILE as f32;

/// The dark behind everything, lifted a little toward the floor, with a faint grid that fades out upward.
pub fn backdrop(c: &mut Canvas, cx: usize, cy: usize, rows: usize, accent: Rgb) {
    let h = (rows * TILE) as f32;
    for y in cy * TILE..(cy + 1) * TILE {
        let depth = y as f32 / h;
        let ground = scale(accent, 0.012 * depth);
        let grid = scale(accent, 0.02 * depth * depth);
        let base = [0.004 + ground[0], 0.005 + ground[1], 0.008 + ground[2]];
        let lined = [base[0] + grid[0], base[1] + grid[1], base[2] + grid[2]];
        for x in cx * TILE..(cx + 1) * TILE {
            c.add(x as i32, y as i32, if x % 16 == 0 || y % 16 == 0 { lined } else { base });
        }
    }
}

/// A wall cell: brick courses in the dim body; along each face open to the air, a lit edge and a fainter bevel line
/// just inside it. `open` is [up, down, left, right].
pub fn wall(c: &mut Canvas, cx: usize, cy: usize, open: [bool; 4], muted: Rgb) {
    let (x0, y0) = ((cx * TILE) as i32, (cy * TILE) as i32);
    let (brick, mortar) = (scale(muted, 0.065), scale(muted, 0.025));
    for y in y0..y0 + TILE as i32 {
        let offset = if (y / 8) % 2 == 0 { 0 } else { 8 };
        for x in x0..x0 + TILE as i32 {
            c.add(x, y, if y % 8 == 0 || (x + offset) % 16 == 0 { mortar } else { brick });
        }
    }
    let (x1, y1) = (x0 + TILE as i32, y0 + TILE as i32);
    let (edge, bevel) = (scale(muted, 0.6), scale(muted, 0.2));
    let [up, down, left, right] = open;
    if up {
        c.fill(x0, y0, x1, y0 + 2, edge);
        c.fill(x0, y0 + 4, x1, y0 + 5, bevel);
    }
    if down {
        c.fill(x0, y1 - 2, x1, y1, edge);
        c.fill(x0, y1 - 5, x1, y1 - 4, bevel);
    }
    if left {
        c.fill(x0, y0, x0 + 2, y1, edge);
        c.fill(x0 + 4, y0, x0 + 5, y1, bevel);
    }
    if right {
        c.fill(x1 - 2, y0, x1, y1, edge);
        c.fill(x1 - 5, y0, x1 - 4, y1, bevel);
    }
}

/// A cracked floor tile (M2): a pale slab with a warm lit edge and a jagged crack, so it reads as floor that won't
/// last. The crack shifts from cell to cell so a row of them doesn't repeat. `open` is [up, down, left, right].
pub fn cracked(c: &mut Canvas, cx: usize, cy: usize, open: [bool; 4], muted: Rgb, accent: Rgb) {
    let (x0, y0) = ((cx * TILE) as i32, (cy * TILE) as i32);
    let (x1, y1) = (x0 + TILE as i32, y0 + TILE as i32);
    c.fill(x0, y0, x1, y1, scale(muted, 0.09));
    let edge = scale(mix(muted, accent, 0.45), 0.65);
    let [up, down, left, right] = open;
    if up {
        c.fill(x0, y0, x1, y0 + 2, edge);
    }
    if down {
        c.fill(x0, y1 - 2, x1, y1, scale(edge, 0.5));
    }
    if left {
        c.fill(x0, y0, x0 + 2, y1, scale(edge, 0.5));
    }
    if right {
        c.fill(x1 - 2, y0, x1, y1, scale(edge, 0.5));
    }
    let (fx, fy, k) = (x0 as f32, y0 as f32, ((cx * 7 + cy * 13) % 5) as f32);
    let crack = [(9.0 + k, 2.0), (14.0 + k * 0.5, 10.0), (11.0 + k, 17.0), (18.0, 24.0), (15.0 + k * 0.6, 31.0)];
    for pair in crack.windows(2) {
        let ((ax, ay), (bx, by)) = (pair[0], pair[1]);
        c.line(fx + ax, fy + ay, fx + bx, fy + by, 1.4, scale(accent, 0.45));
    }
    let (bx, by) = crack[1];
    c.line(fx + bx, fy + by, fx + 23.0, fy + 13.0 + k, 1.1, scale(accent, 0.3));
}

/// A crate at cell (`x`, `y`): a glowing frame, cross braces and corner rivets. `squash` > 0 squashes it flat
/// around its bottom edge (a landing), < 0 stretches it tall.
pub fn crate_(c: &mut Canvas, x: f32, y: f32, squash: f32, accent: Rgb) {
    let (w, h) = (26.0 * (1.0 + 0.2 * squash), 26.0 * (1.0 - 0.18 * squash));
    let (mx, by) = (x * T + T / 2.0, y * T + T - 3.0);
    let (x0, y0, x1, y1) = (mx - w / 2.0, by - h, mx + w / 2.0, by);
    let (ix0, iy0, ix1, iy1) = (x0.round() as i32, y0.round() as i32, x1.round() as i32, y1.round() as i32);
    c.fill(ix0, iy0, ix1, iy1, scale(accent, 0.07));
    c.frame(ix0, iy0, ix1, iy1, 2, scale(accent, 0.95));
    let (bx0, by0, bx1, by1) = (x0 + 4.0, y0 + 4.0, x1 - 4.0, y1 - 4.0);
    c.line(bx0, by0, bx1, by1, 1.6, scale(accent, 0.42));
    c.line(bx1, by0, bx0, by1, 1.6, scale(accent, 0.42));
    for (rx, ry) in [(x0 + 4.0, y0 + 4.0), (x1 - 6.0, y0 + 4.0), (x0 + 4.0, y1 - 6.0), (x1 - 6.0, y1 - 6.0)] {
        c.fill(rx as i32, ry as i32, rx as i32 + 2, ry as i32 + 2, scale(accent, 1.3));
    }
}

/// The doorway's frame at cell (`x`, `y`): two posts and a lintel, no sill.
pub fn door_frame(c: &mut Canvas, x: usize, y: usize, door: Rgb) {
    let (x0, y0) = ((x * TILE) as i32, (y * TILE) as i32);
    c.fill(x0 + 5, y0 + 2, x0 + 27, y0 + 5, scale(door, 1.1));
    c.fill(x0 + 5, y0 + 5, x0 + 8, y0 + 32, scale(door, 1.1));
    c.fill(x0 + 24, y0 + 5, x0 + 27, y0 + 32, scale(door, 1.1));
}

/// The light inside the doorway, in bands that rise with `time` (still when `time` is `None`).
pub fn door_light(c: &mut Canvas, x: usize, y: usize, time: Option<f32>, accent: Rgb) {
    let (x0, y0) = ((x * TILE) as i32, (y * TILE) as i32);
    for row in 5..32 {
        let k = match time {
            Some(t) => 0.2 + 0.11 * ((row as f32) * 0.55 + t * 7.0).sin() + 0.05 * (t * 2.3).sin(),
            None => 0.28,
        };
        c.fill(x0 + 8, y0 + row, x0 + 24, y0 + row + 1, scale(accent, k));
    }
}

/// How the climber stands this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pose {
    pub left: bool,
    /// Legs mid-stride, -1 to 1 (0 standing).
    pub stride: f32,
    /// Reaching up a step, 0 to 1.
    pub climb: f32,
    /// The idle breath, in pixels.
    pub breath: f32,
    pub blink: bool,
    /// Leaning into a turn, -1 (left) to 1.
    pub lean: f32,
    /// Squashed by a landing (> 0) or stretched by a fall (< 0).
    pub squash: f32,
    /// Shrinking into the doorway on a win, 0 to 1.
    pub gone: f32,
    pub refused: bool,
}

impl Pose {
    pub fn facing(facing: Facing) -> Pose {
        Pose { left: facing == Facing::Left, ..Pose::default() }
    }
}

/// The climber at cell (`x`, `y`): a head, a body, arms and legs, all neon, posed by `pose`.
pub fn climber(c: &mut Canvas, x: f32, y: f32, pose: &Pose, fg: Rgb) {
    let body = if pose.refused { [1.0, 0.35, 0.25] } else { fg };
    let k = 1.0 - 0.75 * pose.gone;
    let (sx, sy) = ((1.0 + 0.22 * pose.squash) * k, (1.0 - 0.18 * pose.squash) * k);
    let bright = 1.0 + 1.5 * pose.gone;
    // Local coordinates: x across from the centre line, y up from the feet. Leaning shifts higher parts further.
    let (fx, fy) = (x * T + T / 2.0, y * T + T);
    let at = |lx: f32, ly: f32| (fx + (lx + pose.lean * ly * 0.14) * sx, fy - ly * sy);
    let part = |c: &mut Canvas, (ax, ay): (f32, f32), (bx, by): (f32, f32), w: f32, k: f32| {
        let (p, q) = (at(ax, ay), at(bx, by));
        c.line(p.0, p.1, q.0, q.1, w * sx.max(0.4), scale(body, k * bright));
    };
    let dir = if pose.left { -1.0 } else { 1.0 };
    // Legs: swing with the stride; on a climb the leading leg lifts.
    let swing = pose.stride * 3.5;
    let lift = (pose.stride.abs() * 2.0).max(pose.climb * 5.0);
    part(c, (-2.5 - swing * dir, 9.0), (-2.5 - swing * dir, lift * 0.3), 3.2, 0.9);
    part(c, (2.5 + swing * dir, 9.0), (2.5 + swing * dir, lift), 3.2, 0.9);
    // Body, breathing.
    let (b0, b1) = (at(-5.5, 21.0 + pose.breath), at(5.5, 9.0));
    let (ix0, iy0, ix1, iy1) =
        (b0.0.min(b1.0) as i32, b0.1.min(b1.1) as i32, b0.0.max(b1.0).ceil() as i32, b0.1.max(b1.1).ceil() as i32);
    c.fill(ix0, iy0, ix1, iy1, scale(body, 0.18 * bright));
    c.frame(ix0, iy0, ix1, iy1, 2, scale(body, 1.1 * bright));
    // Arms: hanging, swinging against the legs, or reaching up on a climb.
    let reach = pose.climb;
    let hand = |side: f32| (side * (7.0 + reach), 12.0 + reach * 16.0 + side * swing * 0.6);
    part(c, (-6.5, 19.0), hand(-1.0), 2.4, 0.85);
    part(c, (6.5, 19.0), hand(1.0), 2.4, 0.85);
    // Head and eye.
    let head = at(0.0, 26.5 + pose.breath);
    c.disc(head.0, head.1, 5.6 * sx.min(sy).max(0.3), scale(body, 1.2 * bright));
    let eye = at(3.0 * dir, 27.0 + pose.breath);
    if pose.blink {
        c.line(eye.0 - 1.5, eye.1, eye.0 + 1.5, eye.1, 1.2, scale(body, -1.0));
    } else {
        c.disc(eye.0, eye.1, 1.5, scale(body, -1.0));
    }
}

/// An ember of the living backdrop: a dim, soft dot, brighter and bigger the nearer it is (`depth` near 1).
pub fn ember(c: &mut Canvas, x: f32, y: f32, depth: f32, twinkle: f32, accent: Rgb) {
    let hot = mix(accent, [1.0, 0.9, 0.7], 0.3 * depth);
    c.spark(x, y, 0.5 + 0.9 * depth, scale(hot, (0.08 + 0.3 * depth) * twinkle));
}
