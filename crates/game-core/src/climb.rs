//! Climbing the pile in the 3D game (spec #58, decisions Q51 and Q59): how tall a ledge the player steps up without a
//! jump, how steep a slope it walks up, and how high the pile stands under a point, which is what the escape test
//! measures. Metres, y up; world x and z on the floor.

use crate::debris::solid::Solid;

/// Tallest ledge the player steps up onto, metres: under a quarter of its height, and as tall as the biggest debris
/// piece (the fodder's torso, 0.32 × 0.4 × 0.5) standing upright. Every piece lying on the floor is a step, while two
/// stacked usually aren't, so a pile has to be built up as a ramp.
pub const MAX_STEP: f32 = 0.4;
/// Steepest slope the player walks up, degrees; anything steeper acts as a wall.
pub const MAX_SLOPE: f32 = 50.0;

/// Whether a surface whose unit normal has this upward part can be walked on.
pub fn walkable(normal_y: f32) -> bool {
    normal_y >= MAX_SLOPE.to_radians().cos()
}

/// A piece lying in the world: its solid, its centre, and its rotation as a unit quaternion `[x, y, z, w]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub solid: Solid,
    pub centre: [f32; 3],
    pub rotation: [f32; 4],
}

/// Points sampled round the rim of a footprint, besides its centre.
const RING: usize = 8;

/// How high the pile stands under a footprint: the highest piece surface, at or below `top`, directly under any of
/// the footprint's sample points (its centre `at` = world `[x, z]`, and eight points `radius` out). 0 (the floor)
/// when no piece is under it. Surfaces above `top` don't count, so a piece over the player's head isn't "under" them;
/// pass the player's feet (plus a small tolerance) as `top`.
pub fn pile_height(pieces: &[Placed], at: [f32; 2], radius: f32, top: f32) -> f32 {
    let mut points = [at; RING + 1];
    for (i, p) in points.iter_mut().skip(1).enumerate() {
        let a = i as f32 / RING as f32 * std::f32::consts::TAU;
        *p = [at[0] + radius * a.cos(), at[1] + radius * a.sin()];
    }
    let mut height: f32 = 0.0;
    for piece in pieces {
        // Too far off to the side to be under the footprint.
        let reach = radius + bound(piece.solid);
        if (piece.centre[0] - at[0]).hypot(piece.centre[2] - at[1]) > reach {
            continue;
        }
        for &[x, z] in &points {
            if let Some(h) = surface_under(piece, x, z, top) {
                height = height.max(h);
            }
        }
    }
    height
}

/// Distance from a solid's centre to its furthest point.
fn bound(solid: Solid) -> f32 {
    match solid {
        Solid::Cube([x, y, z]) => (x * x + y * y + z * z).sqrt() / 2.0,
        Solid::Ball(r) => r,
    }
}

/// The highest point of `piece` on the vertical line through `x, z` that is at or below `top`, if the line meets it
/// there.
fn surface_under(piece: &Placed, x: f32, z: f32, top: f32) -> Option<f32> {
    let [cx, cy, cz] = piece.centre;
    match piece.solid {
        Solid::Ball(r) => {
            let d2 = (x - cx).powi(2) + (z - cz).powi(2);
            let half = (r * r - d2).sqrt();
            (d2 <= r * r && cy - half <= top).then(|| (cy + half).min(top))
        }
        Solid::Cube(size) => {
            // The line `y = top - t` (t >= 0, going down), in the cube's own frame: a slab test on each axis.
            let inverse = conjugate(piece.rotation);
            let origin = rotate(inverse, [x - cx, top - cy, z - cz]);
            let dir = rotate(inverse, [0.0, -1.0, 0.0]);
            let (mut enter, mut exit) = (f32::NEG_INFINITY, f32::INFINITY);
            for k in 0..3 {
                let half = size[k] / 2.0;
                if dir[k].abs() < 1e-7 {
                    if origin[k].abs() > half {
                        return None;
                    }
                    continue;
                }
                let (a, b) = ((-half - origin[k]) / dir[k], (half - origin[k]) / dir[k]);
                enter = enter.max(a.min(b));
                exit = exit.min(a.max(b));
            }
            // The cube spans t in enter..=exit; below `top` means t >= 0.
            (enter <= exit && exit >= 0.0).then(|| top - enter.max(0.0))
        }
    }
}

fn conjugate([x, y, z, w]: [f32; 4]) -> [f32; 4] {
    [-x, -y, -z, w]
}

/// `v` rotated by the unit quaternion `q`.
fn rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let u = [q[0], q[1], q[2]];
    let cross =
        |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let t = cross(u, v).map(|c| 2.0 * c);
    let ut = cross(u, t);
    [v[0] + q[3] * t[0] + ut[0], v[1] + q[3] * t[1] + ut[1], v[2] + q[3] * t[2] + ut[2]]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::debris::solid::FODDER;

    const LEVEL: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

    fn cube(size: [f32; 3], centre: [f32; 3]) -> Placed {
        Placed { solid: Solid::Cube(size), centre, rotation: LEVEL }
    }

    /// A turn by `a` radians about the z axis.
    fn about_z(a: f32) -> [f32; 4] {
        [0.0, 0.0, (a / 2.0).sin(), (a / 2.0).cos()]
    }

    const HIGH: f32 = 100.0;

    #[test]
    fn with_no_pieces_the_pile_is_the_floor() {
        assert_eq!(pile_height(&[], [0.0, 0.0], 0.3, HIGH), 0.0);
    }

    #[test]
    fn a_block_under_the_point_raises_the_pile_to_its_top_and_one_beside_it_doesnt() {
        let block = [cube([1.0, 0.5, 1.0], [0.0, 0.25, 0.0])];
        assert!((pile_height(&block, [0.0, 0.0], 0.3, HIGH) - 0.5).abs() < 1e-5);
        assert_eq!(pile_height(&block, [2.0, 0.0], 0.3, HIGH), 0.0, "beside it, the floor");
        assert_eq!(pile_height(&block, [0.0, -2.0], 0.3, HIGH), 0.0);
    }

    #[test]
    fn stacked_pieces_count_to_the_top_of_the_stack() {
        let stack = [cube([1.0, 0.5, 1.0], [0.0, 0.25, 0.0]), cube([0.4, 0.3, 0.4], [0.1, 0.65, -0.1])];
        assert!((pile_height(&stack, [0.1, -0.1], 0.1, HIGH) - 0.8).abs() < 1e-5);
    }

    #[test]
    fn a_piece_at_the_edge_of_the_footprint_still_counts() {
        let block = [cube([0.2, 0.3, 0.2], [0.35, 0.15, 0.0])];
        assert!((pile_height(&block, [0.0, 0.0], 0.3, HIGH) - 0.3).abs() < 1e-5, "under the footprint's rim");
        assert_eq!(pile_height(&block, [0.0, 0.0], 0.1, HIGH), 0.0, "outside a smaller footprint");
    }

    #[test]
    fn pieces_above_the_top_dont_count_but_one_reaching_into_it_does() {
        let overhead = [cube([1.0, 0.2, 1.0], [0.0, 3.0, 0.0])];
        assert_eq!(pile_height(&overhead, [0.0, 0.0], 0.2, 1.0), 0.0, "a piece over the head is not under the feet");
        let block = [cube([1.0, 0.5, 1.0], [0.0, 0.25, 0.0])];
        assert!((pile_height(&block, [0.0, 0.0], 0.2, 0.45) - 0.45).abs() < 1e-5, "a top a little above the feet");
    }

    #[test]
    fn a_tilted_block_is_as_high_as_its_corner_under_the_point() {
        // A unit cube turned 45 degrees about z stands on an edge: its ridge is half a diagonal above the centre.
        let ridge = [Placed {
            solid: Solid::Cube([1.0, 1.0, 1.0]),
            centre: [0.0, 1.0, 0.0],
            rotation: about_z(std::f32::consts::FRAC_PI_4),
        }];
        let h = pile_height(&ridge, [0.0, 0.0], 0.0, HIGH);
        assert!((h - (1.0 + std::f32::consts::FRAC_1_SQRT_2)).abs() < 1e-4, "ridge at {h}");
        // A quarter of the way along the slope, the top is lower by that much.
        let h = pile_height(&ridge, [0.25, 0.0], 0.0, HIGH);
        assert!((h - (1.0 + std::f32::consts::FRAC_1_SQRT_2 - 0.25)).abs() < 1e-4, "on the slope at {h}");
    }

    #[test]
    fn a_ball_is_highest_over_its_centre() {
        let ball = [Placed { solid: Solid::Ball(0.2), centre: [1.0, 0.2, 1.0], rotation: LEVEL }];
        assert!((pile_height(&ball, [1.0, 1.0], 0.0, HIGH) - 0.4).abs() < 1e-5);
        let off = pile_height(&ball, [1.12, 1.0], 0.0, HIGH);
        assert!((off - (0.2 + 0.16)).abs() < 1e-4, "on its shoulder: {off}");
        assert_eq!(pile_height(&ball, [1.3, 1.0], 0.0, HIGH), 0.0);
    }

    #[test]
    fn any_debris_piece_lying_on_the_floor_is_a_step_but_two_stacked_torsos_are_not() {
        let smallest = |p: &crate::debris::solid::Piece| match p.solid {
            Solid::Cube(s) => s.iter().fold(f32::MAX, |m, x| m.min(*x)),
            Solid::Ball(r) => 2.0 * r,
        };
        for p in &FODDER {
            assert!(smallest(p) <= MAX_STEP, "{p:?} lying flat is a step");
        }
        assert!(2.0 * smallest(&FODDER[1]) > MAX_STEP, "a torso on a torso is a wall");
    }

    #[test]
    fn slopes_up_to_fifty_degrees_are_walkable() {
        assert!(walkable(1.0));
        assert!(walkable(45f32.to_radians().cos()));
        assert!(!walkable(55f32.to_radians().cos()));
        assert!(!walkable(0.0), "a wall");
    }
}
