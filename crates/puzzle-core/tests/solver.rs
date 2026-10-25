//! The solver against levels whose shortest solution is worked out by hand, and against a plain breadth-first search.

mod common;

use common::*;
use puzzle_core::*;

fn keys(level: &Level) -> Option<String> {
    let found = solve(level).solution?;
    Some(found.iter().map(|k| format!("{k:?}")).collect())
}

#[test]
fn walks_straight_to_the_exit() {
    assert_eq!(keys(&level("#>..E#\n######")).as_deref(), Some("RRR"));
}

#[test]
fn turning_round_costs_a_move() {
    assert_eq!(keys(&level("#E..<#\n######")).as_deref(), Some("LLL"));
    assert_eq!(keys(&level("#E..>#\n######")).as_deref(), Some("LLLL"));
}

#[test]
fn carries_a_block_to_make_a_step_up_a_wall() {
    // Lift the block, carry it one cell, put it against the wall, climb it, climb the wall, walk in: 7 moves.
    let l = level(
        "
        #.....E#
        #....###
        #>.B.###
        ########",
    );
    let found = keys(&l).expect("solvable");
    assert_eq!(found.len(), 7, "{found}");
    assert!(Play::new(l).apply(&found).solved, "{found} doesn't reach the exit");
}

#[test]
fn finds_no_way_up_a_wall_too_high_to_climb() {
    let l = level(
        "
        #....E#
        #...###
        #>..###
        #######",
    );
    assert_eq!(solve(&l).solution, None);
}

/// The length of a shortest solution by plain breadth-first search, or `None`. It shares nothing with the solver but
/// `step`, so it is an independent check of the solver's estimate: an estimate that ever overshot could make A* return
/// a longer path than this.
fn bfs(level: &Level) -> Option<usize> {
    use std::collections::{HashSet, VecDeque};
    let mut seen = HashSet::from([level.start]);
    let mut queue = VecDeque::from([(level.start, 0)]);
    while let Some((s, d)) = queue.pop_front() {
        if level.solved(&s) {
            return Some(d);
        }
        for k in [Key::L, Key::R, Key::U, Key::D] {
            if let Ok(n) = step(level, &s, k)
                && seen.insert(n)
            {
                queue.push_back((n, d + 1));
            }
        }
    }
    None
}

#[test]
fn matches_breadth_first_search_on_random_levels() {
    let mut rng = Rng(0x0b2_a57a);
    let (mut solvable, mut longest) = (0, 0);
    for _ in 0..300 {
        let l = random_level_within(&mut rng, 10, 7);
        let found = solve(&l).solution;
        assert_eq!(found.as_ref().map(Vec::len), bfs(&l), "in\n{l}");
        if let Some(ks) = found {
            let text: String = ks.iter().map(|k| format!("{k:?}")).collect();
            assert!(Play::new(l).apply(&text).solved, "{text} doesn't reach the exit");
            solvable += 1;
            longest = longest.max(ks.len());
        }
    }
    // The levels must exercise the solver, not just be walk-ups or dead ends.
    assert!(solvable >= 60 && longest >= 15, "{solvable} solvable, longest {longest}");
}
