//! How a key played out (`Play::key`): every block's way through each step of a move, recorded where the rules move
//! it, which the browser draws instead of guessing.

mod common;

use common::*;
use puzzle_core::*;

/// The blocks of `s` by cell with whether each is ice, the carried one in the cell above you.
fn blocks(l: &Level, s: &State) -> Vec<(usize, bool)> {
    let w = l.w as usize;
    let mut out: Vec<_> = (0..w * l.h as usize).filter(|&c| s.blocks.get(c)).map(|c| (c, s.ice.get(c))).collect();
    if s.carrying {
        out.push((s.y as usize * w + s.x as usize - w, s.carrying_ice));
    }
    out.sort();
    out
}

/// The rounds of one key, as `(from, to)` per block, sorted.
fn paths(before: &str, key: Key) -> Vec<Vec<(usize, usize)>> {
    let mut play = Play::new(level(before));
    let rounds = play.key(key).unwrap_or_else(|why| panic!("{key:?} refused: {why:?}"));
    rounds
        .iter()
        .map(|r| {
            let mut ways: Vec<_> = r.moved.iter().map(|m| (m.from, m.to)).collect();
            ways.sort();
            ways
        })
        .collect()
}

/// Any random level with some crates made ice, some floor made cracked and some floor made springs.
fn random_mixed(rng: &mut Rng) -> Option<Level> {
    let text: String = random_level(rng)
        .to_string()
        .lines()
        .enumerate()
        .flat_map(|(y, row)| {
            let n = row.len();
            let mut row: Vec<char> = row.chars().collect();
            for (x, c) in row.iter_mut().enumerate() {
                let inner = x > 0 && x + 1 < n && y > 0;
                *c = match *c {
                    'B' if rng.below(2) == 0 => 'I',
                    '#' if inner && rng.below(4) == 0 => '~',
                    '#' if inner && rng.below(4) == 0 => '^',
                    c => c,
                };
            }
            row.into_iter().chain(['\n'])
        })
        .collect();
    Level::parse(text.trim_end()).ok()
}

#[test]
fn every_round_moves_each_block_from_where_the_last_left_it_to_where_the_state_has_it() {
    let mut rng = Rng(0x0006_0ed5_10ce_5eed);
    let mut rounds_seen = 0;
    for _ in 0..400 {
        let Some(l) = random_mixed(&mut rng) else { continue };
        let w = l.w as usize;
        let mut play = Play::new(l.clone());
        for _ in 0..150 {
            let before = *play.state();
            let Ok(rounds) = play.key([Key::L, Key::R, Key::U, Key::D][rng.below(4)]) else { continue };
            if rounds.is_empty() {
                break; // solved
            }
            let mut first: Vec<_> = rounds[0].moved.iter().map(|m| (m.from, m.ice)).collect();
            first.sort();
            assert_eq!(first, blocks(&l, &before), "the first round leaves from the state before the key");
            for (i, r) in rounds.iter().enumerate() {
                let shown = l.show(&r.state);
                if i > 0 {
                    let was: Vec<_> = rounds[i - 1].moved.iter().map(|m| (m.to, m.ice)).collect();
                    let now: Vec<_> = r.moved.iter().map(|m| (m.from, m.ice)).collect();
                    assert_eq!(
                        now, was,
                        "each block, in the same place in the list, leaves from where it was, in\n{shown}"
                    );
                }
                let mut to: Vec<_> = r.moved.iter().map(|m| (m.to, m.ice)).collect();
                to.sort();
                assert_eq!(to, blocks(&l, &r.state), "every block arrives where the state has it, in\n{shown}");
                for m in &r.moved {
                    let (dx, dy) = ((m.to % w).abs_diff(m.from % w), (m.to / w).abs_diff(m.from / w));
                    assert!(dx <= 1 && dy <= 1, "a block moves at most one cell each way per round: {m:?} in\n{shown}");
                }
                rounds_seen += 1;
            }
            if play.solved() {
                break;
            }
        }
    }
    assert!(rounds_seen > 10_000, "the random play should exercise many rounds, saw {rounds_seen}");
}

#[test]
fn a_refused_key_plays_nothing_and_a_solved_level_gives_no_rounds() {
    let mut play = Play::new(level("#.....E#\n#>.....#\n########"));
    assert_eq!(play.key(Key::U), Err(Refusal::NothingToClimb));
    assert_eq!(play.moves(), 0);
    let mut solved = Play::new(level("#>E#\n####"));
    assert_eq!(solved.key(Key::R).map(|r| r.len()), Ok(1));
    assert_eq!((solved.key(Key::R), solved.moves()), (Ok(vec![]), 1));
}

#[test]
fn a_turn_is_one_round_where_nothing_moves() {
    let mut play = Play::new(level("#.....E#\n#>B....#\n########"));
    let rounds = play.key(Key::L).unwrap();
    assert_eq!(rounds.len(), 1);
    assert_eq!(rounds[0].state.facing, Facing::Left);
    assert!(rounds[0].moved.iter().all(|m| m.from == m.to));
}

#[test]
fn a_lift_takes_the_block_from_in_front_to_above_you() {
    // 8 wide: you at 9, the crate at 10, above you 1.
    assert_eq!(paths("#.....E#\n#>B....#\n########", Key::D), vec![vec![(10, 1)]]);
}

#[test]
fn ice_set_down_slides_a_cell_a_round() {
    // The ice goes from above you (1) to the cell in front (2); the next round it falls a row and slides a cell (to
    // 11, both in one round: falling comes before sliding), then slides a cell a round to the wall.
    let rounds = paths("#I....E#\n#>.....#\n########", Key::D);
    assert_eq!(rounds[0], vec![(1, 2)]);
    let ways: Vec<_> = rounds[1..].iter().map(|r| r[0]).collect();
    assert_eq!(ways, [(2, 11), (11, 12), (12, 13), (13, 14)]);
}

#[test]
fn what_rides_on_passed_ice_moves_with_it() {
    // Set-down ice slides into the resting ice at 14 and stops at 13; that ice slides on, the crate on it (4) riding.
    let rounds = paths("#I..B....#\n#>..I....#\n#########E", Key::D);
    let rides = rounds.iter().any(|r| r.contains(&(14, 15)) && r.contains(&(4, 5)));
    assert!(rides, "the rider and its ice move together: {rounds:?}");
}

#[test]
fn a_crate_set_down_on_a_spring_rises_then_hops() {
    // Set down at 2, the crate falls onto the spring (11), rises a row and hops a cell in one round (3), and falls.
    let rounds = paths("#B.....E#\n#>......#\n##^######", Key::D);
    assert_eq!(rounds, [vec![(1, 2)], vec![(2, 11)], vec![(11, 3)], vec![(3, 12)]]);
}
