//! Bomb-heads (decision Q32): from box 2 any enemy can carry a bomb for a head. On defeat the bomb is lit: a 1.5 s
//! blinking fuse, then a blast that hurts nearby enemies and the player and scatters debris. The blast never deletes
//! debris (the pile never loses progress outright); it only knocks parts loose. A lit bomb can be grabbed and thrown.

/// Seconds from defeat to blast.
pub const FUSE: f32 = 1.5;
/// Blast radius, world units (about two and a half player heights).
pub const RADIUS: f32 = 140.0;
/// Damage at the centre and at the edge of the blast.
pub const DAMAGE: (f32, f32) = (32.0, 8.0);
/// Launch speed at the centre of the blast, world units/s (falls off to a third at the edge).
pub const PUSH: f32 = 1100.0;

/// A lit fuse.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fuse {
    pub left: f32,
}

impl Default for Fuse {
    fn default() -> Self {
        Self { left: FUSE }
    }
}

impl Fuse {
    /// Burns down; returns true on the frame it goes off.
    pub fn tick(&mut self, dt: f32) -> bool {
        let was = self.left;
        self.left -= dt;
        was > 0.0 && self.left <= 0.0
    }

    /// Whether the bomb's light is on this instant: the blink speeds up as the fuse runs out.
    pub fn blink(&self) -> bool {
        let burned = (FUSE - self.left).max(0.0);
        // 3 Hz rising to 12 Hz: the phase is the integral of a linearly rising frequency.
        let phase = 3.0 * burned + 0.5 * (12.0 - 3.0) / FUSE * burned * burned;
        phase.fract() < 0.5
    }
}

/// Falloff from 1 at the centre to 0 at the edge, or `None` outside the blast.
fn reach(distance: f32) -> Option<f32> {
    (distance <= RADIUS).then(|| 1.0 - distance / RADIUS)
}

/// Damage to someone `distance` from the blast, or `None` if out of range.
pub fn damage(distance: f32) -> Option<f32> {
    reach(distance).map(|k| DAMAGE.1 + (DAMAGE.0 - DAMAGE.1) * k)
}

/// Launch velocity for something at `offset` from the blast (pushed straight away, and a little up), or `None` if out
/// of range.
pub fn push(offset: (f32, f32)) -> Option<(f32, f32)> {
    let d = (offset.0 * offset.0 + offset.1 * offset.1).sqrt();
    let k = reach(d)?;
    let speed = PUSH * (1.0 / 3.0 + 2.0 / 3.0 * k);
    let (nx, ny) = if d > 1e-3 { (offset.0 / d, offset.1 / d) } else { (0.0, 1.0) };
    Some((nx * speed, ny * speed + 0.35 * speed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fuse_burns_for_a_second_and_a_half_then_goes_off_once() {
        let mut f = Fuse::default();
        let mut went_off = Vec::new();
        for i in 0..120 {
            if f.tick(1.0 / 60.0) {
                went_off.push(i);
            }
        }
        assert_eq!(went_off.len(), 1);
        assert!((went_off[0] as f32 / 60.0 - FUSE).abs() < 0.03);
    }

    #[test]
    fn the_blink_speeds_up_as_the_fuse_runs_out() {
        let flips = |from: f32, to: f32| {
            let mut n = 0;
            let mut last = None;
            let mut t = from;
            while t < to {
                let b = Fuse { left: FUSE - t }.blink();
                if last.is_some_and(|l| l != b) {
                    n += 1;
                }
                last = Some(b);
                t += 0.002;
            }
            n
        };
        assert!(flips(1.0, 1.5) > 2 * flips(0.0, 0.5), "faster at the end");
    }

    #[test]
    fn the_blast_hurts_most_at_the_centre_and_nothing_outside_its_radius() {
        assert_eq!(damage(0.0), Some(DAMAGE.0));
        assert!(damage(RADIUS * 0.5).unwrap() < DAMAGE.0);
        assert_eq!(damage(RADIUS), Some(DAMAGE.1));
        assert_eq!(damage(RADIUS + 1.0), None);
    }

    #[test]
    fn the_blast_pushes_things_away_and_up() {
        let (x, y) = push((50.0, 0.0)).unwrap();
        assert!(x > 0.0 && y > 0.0, "right of the blast goes right and up");
        let (x, _) = push((-50.0, 0.0)).unwrap();
        assert!(x < 0.0);
        assert!(push((0.0, 0.0)).unwrap().1 > 0.0, "dead centre goes straight up");
        assert_eq!(push((RADIUS + 1.0, 0.0)), None);
        assert!(push((10.0, 0.0)).unwrap().0 > push((120.0, 0.0)).unwrap().0, "harder near the centre");
    }
}
