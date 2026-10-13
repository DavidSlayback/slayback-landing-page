//! Debris: a defeated enemy falls apart into its own parts, which persist as solid, climbable terrain.
//!
//! Budget and freeze rule come from the B1 spike (docs/research/game-research.md, "debris pile in Chrome"): a part
//! freezes (becomes static) 3 s after it spawns, or as soon as the physics puts it to sleep; at most 6 parts per enemy
//! (the figure's own primitives) and 1000 in the box.

/// Parts one enemy breaks into (the figure's torso, head and four limbs).
pub const MAX_PARTS_PER_ENEMY: usize = 6;
/// Parts the box holds; past this the oldest go.
pub const MAX_PARTS: usize = 1000;
/// Seconds after spawning when a part freezes regardless of motion.
pub const FREEZE_AFTER: f32 = 3.0;

/// A part's collision shape, in the enemy figure's local space (world units, +y up, centre of the body at 0,0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Box { w: f32, h: f32 },
    Ball { r: f32 },
}

/// One piece of a broken figure: its shape and where it sat in the figure.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Part {
    pub shape: Shape,
    pub offset: (f32, f32),
}

/// The parts of a humanoid figure whose body is `half` (half-width, half-height): torso, head, arms and legs,
/// laid out like the rendered figure (2.45 rig units tall, feet at the bottom of the body).
pub fn humanoid_parts(half: (f32, f32)) -> Vec<Part> {
    let s = half.1 * 2.0 / 2.45; // rig unit -> world units
    let feet = -half.1 + 0.775 * s; // rig origin in world space
    let at = |x: f32, y: f32| (x * s, feet + y * s);
    let limb = Shape::Box { w: 0.22 * s, h: 0.75 * s };
    let parts = vec![
        Part { shape: Shape::Box { w: 0.7 * s, h: 1.0 * s }, offset: at(0.0, 0.5) },
        Part { shape: Shape::Ball { r: 0.32 * s }, offset: at(0.0, 1.35) },
        Part { shape: limb, offset: at(-0.48, 0.45) },
        Part { shape: limb, offset: at(0.48, 0.45) },
        Part { shape: limb, offset: at(-0.18, -0.4) },
        Part { shape: limb, offset: at(0.18, -0.4) },
    ];
    debug_assert!(parts.len() <= MAX_PARTS_PER_ENEMY);
    parts
}

/// Whether a part should freeze now.
pub fn should_freeze(age: f32, asleep: bool) -> bool {
    asleep || age >= FREEZE_AFTER
}

/// How many of the oldest parts to remove so the box holds at most [`MAX_PARTS`].
pub fn over_cap(count: usize) -> usize {
    count.saturating_sub(MAX_PARTS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_figure_breaks_into_at_most_the_capped_number_of_parts_that_fit_inside_its_body() {
        let half = (15.0, 26.0);
        let parts = humanoid_parts(half);
        assert_eq!(parts.len(), MAX_PARTS_PER_ENEMY);
        for p in &parts {
            let (hw, hh) = match p.shape {
                Shape::Box { w, h } => (w / 2.0, h / 2.0),
                Shape::Ball { r } => (r, r),
            };
            assert!(hw > 0.0 && hh > 0.0);
            // Within the body's height, with a little slack for arms and head
            assert!(p.offset.1 - hh >= -half.1 - 0.5 && p.offset.1 + hh <= half.1 + 2.0, "{p:?} sticks out of the body");
        }
    }

    #[test]
    fn parts_freeze_when_asleep_or_once_old_enough() {
        assert!(!should_freeze(0.5, false));
        assert!(should_freeze(0.5, true));
        assert!(should_freeze(FREEZE_AFTER, false));
    }

    #[test]
    fn the_box_sheds_only_the_parts_beyond_its_cap() {
        assert_eq!(over_cap(10), 0);
        assert_eq!(over_cap(MAX_PARTS), 0);
        assert_eq!(over_cap(MAX_PARTS + 6), 6);
    }
}
