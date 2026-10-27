//! The A* solver (`docs/planning/puzzle-spec.md`, "Solver and generator"): the bench's oracle, and what sets each
//! generated level's tier and move budget.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::hash::{BuildHasherDefault, Hasher};

use crate::{Key, Level, State, step};

/// The Fx hash (as in rustc): a rotate, xor and multiply per word. States are hashed millions of times per search and
/// nothing adversarial reaches this map, so SipHash's protection only cost time (M3: the search ran 1.35× slower once
/// states grew ice fields). The hasher can't change a result: the map is only looked up, never walked.
#[derive(Default)]
struct Fx(u64);

impl Hasher for Fx {
    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut word = [0u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.write_u64(u64::from_le_bytes(word));
        }
    }
    fn write_u64(&mut self, n: u64) {
        self.0 = (self.0.rotate_left(5) ^ n).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
    fn write_u8(&mut self, n: u8) {
        self.write_u64(n as u64);
    }
    fn write_usize(&mut self, n: usize) {
        self.write_u64(n as u64);
    }
    fn finish(&self) -> u64 {
        self.0
    }
}

type States<V> = HashMap<State, V, BuildHasherDefault<Fx>>;

/// What a search found, and how hard it worked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Search {
    /// A shortest key sequence from the start to the exit, or `None` if the exit can't be reached.
    pub solution: Option<Vec<Key>>,
    /// States taken off the frontier and expanded.
    pub expanded: usize,
    /// The search stopped at its cap: no solution was found, but none was ruled out either.
    pub gave_up: bool,
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

/// A shortest solution, however long the search takes. Proving a big level unsolvable can take minutes and gigabytes
/// (`docs/planning/puzzle-spec.md`, "Measured"); [`solve_within`] bounds it.
pub fn solve(level: &Level) -> Search {
    solve_within(level, usize::MAX)
}

/// As [`solve`], but expanding at most `cap` states. A solution it finds is still a shortest one.
pub fn solve_within(level: &Level, cap: usize) -> Search {
    // Every state reached so far: the state, the node it came from and the key that got here, plus its distance.
    struct Node {
        state: State,
        parent: usize,
        key: Key,
        g: u32,
    }
    let start = level.start;
    let mut nodes = vec![Node { state: start, parent: usize::MAX, key: Key::L, g: 0 }];
    let mut best: States<usize> = States::default();
    best.insert(start, 0);
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
            return Search { solution: Some(keys), expanded, gave_up: false };
        }
        if expanded == cap {
            return Search { solution: None, expanded, gave_up: true };
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
    Search { solution: None, expanded, gave_up: false }
}
