//! The A* solver (`docs/planning/puzzle-spec.md`, "Solver and generator"): the bench's oracle, and what sets each
//! generated level's tier and move budget.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use crate::{Key, Level, State, step};

/// What a search found, and how hard it worked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Search {
    /// A shortest key sequence from the start to the exit, or `None` if the exit can't be reached.
    pub solution: Option<Vec<Key>>,
    /// States taken off the frontier and expanded.
    pub expanded: usize,
}

const KEYS: [Key; 4] = [Key::L, Key::R, Key::U, Key::D];

/// A lower bound on the moves left: a move shifts you at most one column, and only a climb raises you, by one row.
/// Turns and falls cost nothing here, so it never overestimates, and it changes by at most one per move, so the first
/// time A* expands a state it has reached it by a shortest path.
fn estimate(level: &Level, s: &State) -> u32 {
    let dx = (s.x as i32 - level.exit.0 as i32).unsigned_abs();
    let rise = (s.y as i32 - level.exit.1 as i32).max(0) as u32;
    dx.max(rise)
}

pub fn solve(level: &Level) -> Search {
    // Every state reached so far: the state, the node it came from and the key that got here, plus its distance.
    struct Node {
        state: State,
        parent: usize,
        key: Key,
        g: u32,
    }
    let start = level.start;
    let mut nodes = vec![Node { state: start, parent: usize::MAX, key: Key::L, g: 0 }];
    let mut best: HashMap<State, usize> = HashMap::from([(start, 0)]);
    // Ties on f go to the deeper node, then the older one, so a search is the same on every platform.
    let mut open = BinaryHeap::from([Reverse((estimate(level, &start), Reverse(0u32), 0usize))]);
    let mut expanded = 0;
    while let Some(Reverse((_, Reverse(g), i))) = open.pop() {
        if g > nodes[i].g {
            continue; // reached more cheaply since this entry was pushed
        }
        let s = nodes[i].state;
        if level.solved(&s) {
            let mut keys = vec![];
            let mut at = i;
            while at != 0 {
                keys.push(nodes[at].key);
                at = nodes[at].parent;
            }
            keys.reverse();
            return Search { solution: Some(keys), expanded };
        }
        expanded += 1;
        for key in KEYS {
            let Ok(next) = step(level, &s, key) else { continue };
            let g = g + 1;
            match best.get(&next) {
                Some(&j) if nodes[j].g <= g => continue,
                Some(&j) => {
                    nodes[j] = Node { state: next, parent: i, key, g };
                    open.push(Reverse((g + estimate(level, &next), Reverse(g), j)));
                }
                None => {
                    let j = nodes.len();
                    nodes.push(Node { state: next, parent: i, key, g });
                    best.insert(next, j);
                    open.push(Reverse((g + estimate(level, &next), Reverse(g), j)));
                }
            }
        }
    }
    Search { solution: None, expanded }
}
