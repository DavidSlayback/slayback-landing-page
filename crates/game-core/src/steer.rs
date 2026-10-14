//! Enemy movement on the 3D floor (decision Q63): steering, not pathfinding. Each enemy seeks the player, keeps apart
//! from the others, and turns away from walls it comes close to, so a crowd spreads out round the player instead of
//! stacking into one body. Thugs plough straight in (the game steers them with no crowd to keep apart from), and a
//! ninja the player is facing [`circle`]s round to its back rather than standing in front.
//!
//! Plane vectors as in [`crate::plane`]; distances in metres.

/// Neighbours closer than this push each other apart, centre to centre (about two fighters' widths).
pub const SEPARATION: f32 = 1.4;
/// Walls closer than this push an enemy back toward the middle.
pub const WALL_MARGIN: f32 = 0.8;
const SEPARATE_WEIGHT: f32 = 1.8;
const WALL_WEIGHT: f32 = 1.2;
/// A flanking enemy circles the player about this far out, centre to centre: past a ninja's striking range (1.4 m)
/// and the player's punch, so it goes round rather than through.
pub const ORBIT: f32 = 2.0;

fn length(v: [f32; 2]) -> f32 {
    v[0].hypot(v[1])
}

fn unit(v: [f32; 2]) -> [f32; 2] {
    let l = length(v);
    if l < 1e-4 { [0.0, 0.0] } else { [v[0] / l, v[1] / l] }
}

/// Which way an enemy at `me` walks (unit length, or zero to stand): toward `goal` when `toward` is positive, away
/// from it when negative (backing off), pushed apart from `others` (the other enemies' centres) and in from the walls
/// of a floor `half` wide each way from the middle.
pub fn steer(me: [f32; 2], goal: [f32; 2], toward: f32, others: &[[f32; 2]], half: [f32; 2]) -> [f32; 2] {
    let seek = unit([goal[0] - me[0], goal[1] - me[1]]);
    keep_clear([seek[0] * toward.signum(), seek[1] * toward.signum()], me, others, half)
}

/// Which way a flanking enemy at `me` walks (unit length) to get behind a player at `goal` facing `facing` (a unit
/// plane vector): round the player toward its back the shorter way, drawn in or out to [`ORBIT`], and kept apart and
/// off the walls as in [`steer`].
pub fn circle(me: [f32; 2], goal: [f32; 2], facing: [f32; 2], others: &[[f32; 2]], half: [f32; 2]) -> [f32; 2] {
    let out = [me[0] - goal[0], me[1] - goal[1]];
    let dist = length(out);
    let back = [-facing[0], -facing[1]];
    let r = if dist < 1e-4 { back } else { unit(out) };
    // Anticlockwise unless the back is clockwise from here (exactly in front, anticlockwise).
    let turn = if back[0] * r[1] - back[1] * r[0] > 0.0 { -1.0 } else { 1.0 };
    let pull = ((ORBIT - dist) / ORBIT).clamp(-1.0, 1.0);
    keep_clear([-r[1] * turn + r[0] * pull, r[0] * turn + r[1] * pull], me, others, half)
}

/// `v` pushed apart from `others` and in from the walls, as a unit vector (or zero).
fn keep_clear(mut v: [f32; 2], me: [f32; 2], others: &[[f32; 2]], half: [f32; 2]) -> [f32; 2] {
    for o in others {
        let d = [me[0] - o[0], me[1] - o[1]];
        let dist = length(d);
        if dist > 1e-4 && dist < SEPARATION {
            let push = SEPARATE_WEIGHT * (1.0 - dist / SEPARATION) / dist;
            v = [v[0] + d[0] * push, v[1] + d[1] * push];
        }
    }
    for axis in 0..2 {
        let inside = half[axis] - WALL_MARGIN;
        let over = me[axis].abs() - inside;
        if over > 0.0 {
            v[axis] -= me[axis].signum() * WALL_WEIGHT * (over / WALL_MARGIN).min(1.0);
        }
    }
    unit(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HALF: [f32; 2] = [7.5, 5.0];

    #[test]
    fn alone_in_the_open_it_heads_straight_for_the_goal_or_straight_away() {
        let v = steer([0.0, 0.0], [3.0, 4.0], 1.0, &[], HALF);
        assert!((v[0] - 0.6).abs() < 1e-5 && (v[1] - 0.8).abs() < 1e-5, "{v:?}");
        let away = steer([0.0, 0.0], [3.0, 4.0], -1.0, &[], HALF);
        assert!((away[0] + 0.6).abs() < 1e-5 && (away[1] + 0.8).abs() < 1e-5, "{away:?}");
        assert_eq!(steer([1.0, 1.0], [1.0, 1.0], 1.0, &[], HALF), [0.0, 0.0], "already there");
    }

    #[test]
    fn two_side_by_side_heading_the_same_way_fan_apart() {
        let goal = [5.0, 0.0];
        let (a, b) = ([0.0, 0.3], [0.0, -0.3]);
        let va = steer(a, goal, 1.0, &[b], HALF);
        let vb = steer(b, goal, 1.0, &[a], HALF);
        assert!(va[1] > 0.1 && vb[1] < -0.1, "spread apart: {va:?} {vb:?}");
        assert!(va[0] > 0.3 && vb[0] > 0.3, "while still closing in: {va:?} {vb:?}");
    }

    #[test]
    fn near_a_wall_it_turns_back_toward_the_middle() {
        // Hugging the right wall, the goal straight along it: it peels off the wall as it goes.
        let v = steer([HALF[0] - 0.2, -2.0], [HALF[0] - 0.2, 3.0], 1.0, &[], HALF);
        assert!(v[0] < -0.3 && v[1] > 0.3, "{v:?}");
        // Far from the walls they don't matter.
        let v = steer([0.0, 0.0], [0.0, 3.0], 1.0, &[], HALF);
        assert!(v[0].abs() < 1e-6);
    }

    #[test]
    fn a_crowd_closing_on_the_player_spreads_round_it_and_stays_on_the_floor() {
        // Five enemies start in a clump and walk at 3 m/s until within 1.2 m of the player, for five seconds.
        let player = [2.0, 0.0];
        let mut crowd: Vec<[f32; 2]> = (0..5).map(|i| [-4.0 + 0.1 * i as f32, 0.05 * i as f32]).collect();
        let dt = 1.0 / 60.0;
        for _ in 0..300 {
            let now = crowd.clone();
            for (i, me) in crowd.iter_mut().enumerate() {
                let others: Vec<_> = now.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, p)| *p).collect();
                let near = length([player[0] - me[0], player[1] - me[1]]) < 1.2;
                let v = steer(*me, player, 1.0, &others, HALF);
                let speed = if near { 0.0 } else { 3.0 };
                *me = [me[0] + v[0] * speed * dt, me[1] + v[1] * speed * dt];
            }
        }
        for (i, a) in crowd.iter().enumerate() {
            assert!(a[0].abs() < HALF[0] && a[1].abs() < HALF[1], "on the floor: {a:?}");
            assert!(length([player[0] - a[0], player[1] - a[1]]) < 2.5, "closed in on the player: {a:?}");
            for b in &crowd[i + 1..] {
                let d = length([a[0] - b[0], a[1] - b[1]]);
                assert!(d > 0.7, "kept apart: {a:?} {b:?} only {d} apart");
            }
        }
        // And they surround the player rather than queueing on one side.
        let bearings: Vec<f32> = crowd.iter().map(|a| (a[1] - player[1]).atan2(a[0] - player[0])).collect();
        let spread =
            bearings.iter().fold(f32::MIN, |m, b| m.max(*b)) - bearings.iter().fold(f32::MAX, |m, b| m.min(*b));
        assert!(spread > 1.0, "fanned out round the player: {bearings:?}");
    }

    /// Walks a flanker at `speed` round a player at the origin facing +x for `seconds`; returns its track.
    fn flank(from: [f32; 2], speed: f32, seconds: f32) -> Vec<[f32; 2]> {
        let (dt, mut me) = (1.0 / 60.0, from);
        let mut track = vec![me];
        for _ in 0..(seconds / dt) as usize {
            let v = circle(me, [0.0, 0.0], [1.0, 0.0], &[], HALF);
            me = [me[0] + v[0] * speed * dt, me[1] + v[1] * speed * dt];
            track.push(me);
        }
        track
    }

    #[test]
    fn a_flanker_in_front_circles_round_to_the_players_back_the_shorter_way_without_cutting_through() {
        // In front and a little anticlockwise (screen up): it goes round that way, at a ninja's 7.5 m/s.
        let track = flank([3.0, 0.4], 7.5, 1.2);
        let behind = track.iter().position(|p| p[0] < -0.5).expect("got round behind");
        assert!(behind as f32 / 60.0 < 1.0, "within a second: {behind} frames");
        for p in &track[..=behind] {
            assert!(p[1] > 0.0, "the shorter way, anticlockwise: {p:?}");
            assert!(length(*p) > 1.6, "kept its distance, not through the player: {p:?}");
        }
        // Clockwise from the front, it goes round clockwise.
        let track = flank([3.0, -0.4], 7.5, 1.2);
        assert!(track.iter().any(|p| p[0] < -0.5) && track.iter().all(|p| p[1] < 0.3), "{:?}", track.last());
    }

    #[test]
    fn a_flanker_holds_its_orbit_and_keeps_clear_of_walls_and_others() {
        // Far out it closes in; close in it backs out; either way it ends near the orbit.
        for from in [[5.0, 1.0], [0.6, 0.6]] {
            let end = *flank(from, 3.0, 3.0).last().unwrap();
            assert!((length(end) - ORBIT).abs() < 0.3, "from {from:?} ended {} out", length(end));
        }
        // Circling into a wall, it turns off it.
        let v = circle([0.0, HALF[1] - 0.1], [0.0, HALF[1] - 2.0], [0.0, -1.0], &[], HALF);
        assert!(v[1] < 0.0, "{v:?}");
        // And it gives way to another enemy in its path.
        let alone = circle([2.0, 0.0], [0.0, 0.0], [1.0, 0.0], &[], HALF);
        let crowded = circle([2.0, 0.0], [0.0, 0.0], [1.0, 0.0], &[[2.0, 0.5]], HALF);
        assert!(crowded[1] < alone[1], "{alone:?} {crowded:?}");
    }
}
