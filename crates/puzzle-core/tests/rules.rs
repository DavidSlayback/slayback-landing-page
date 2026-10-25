//! The rules table of `docs/planning/puzzle-spec.md`, one test per row and per refusal, written as grids before and
//! after.

use puzzle_core::*;

/// Trims each line so grids can be indented in the source.
fn grid(text: &str) -> String {
    text.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")
}

fn level(text: &str) -> Level {
    Level::parse(&grid(text)).unwrap_or_else(|e| panic!("{e:?} in\n{}", grid(text)))
}

#[test]
fn text_form_round_trips() {
    let text = grid(
        "
        #......#
        #..B..E#
        #>.##..#
        ########",
    );
    let l = Level::parse(&text).unwrap();
    assert_eq!(l.to_string(), text);
    assert_eq!((l.start.x, l.start.y, l.start.facing, l.start.carrying), (1, 2, Facing::Right, false));
    assert_eq!(l.exit, (6, 1));
}

#[test]
fn a_block_above_the_player_is_carried() {
    let l = level(
        "
        #B....#
        #<...E#
        #######",
    );
    assert!(l.start.carrying);
    assert_eq!(l.start.facing, Facing::Left);
    assert_eq!(l.to_string(), grid("#B....#\n#<...E#\n#######"));
}

#[test]
fn a_block_on_the_exit_is_drawn_lowercase() {
    let text = grid(
        "
        #>..b#
        ######",
    );
    assert_eq!(level(&text).to_string(), text);
    // Carried through the exit cell too (found by the random-play test).
    let carried = level("#.b.#\n#.>.#\n#####");
    assert!(carried.start.carrying);
    assert_eq!(carried.to_string(), grid("#.b.#\n#.>.#\n#####"));
}

#[test]
fn malformed_text_is_refused() {
    use ParseError::*;
    for (bad, want) in [
        ("#>..#\n#####", Exits(0)),
        ("#>.E<#\n######", Players(2)),
        ("#..E#\n#####", Players(0)),
        ("#B...#\n#...E#\n#>####", Floating { x: 1, y: 0 }),
        ("#..>E#\n###.##", Floating { x: 3, y: 0 }),
        ("#>..E#\n####", Ragged { row: 1 }),
        ("#>..E#\n#..x.#\n######", Unknown { ch: 'x', x: 3, y: 1 }),
    ] {
        assert_eq!(Level::parse(bad), Err(want), "in\n{bad}");
    }
    let wide = format!("{}\n#>E{}#\n{}", "#".repeat(22), ".".repeat(17), "#".repeat(22));
    assert_eq!(Level::parse(&wide), Err(TooBig { w: 22, h: 3 }));
}

fn keys(s: &str) -> Vec<Key> {
    s.chars().map(|c| Key::from_char(c).unwrap_or_else(|| panic!("not a key: {c}"))).collect()
}

/// The grid after playing `ks` from `before`; panics on a refusal.
fn after(before: &str, ks: &str) -> String {
    let l = level(before);
    let mut s = l.start;
    for (i, k) in keys(ks).into_iter().enumerate() {
        s = step(&l, &s, k).unwrap_or_else(|r| panic!("key {i} ({k:?}) refused: {r:?}"));
    }
    l.show(&s)
}

/// The refusal of the last key in `ks`, the earlier ones having played.
fn refused(before: &str, ks: &str) -> Refusal {
    let l = level(before);
    let ks = keys(ks);
    let (last, earlier) = ks.split_last().unwrap();
    let s = earlier.iter().fold(l.start, |s, &k| step(&l, &s, k).unwrap());
    step(&l, &s, *last).expect_err("the last key played")
}

#[test]
fn walking_the_way_you_face_steps_one_cell() {
    assert_eq!(after("#>..E#\n######", "R"), grid("#.>.E#\n######"));
}

#[test]
fn pressing_the_other_way_turns_in_place() {
    assert_eq!(after("#.>.E#\n######", "L"), grid("#.<.E#\n######"));
    assert_eq!(after("#.>.E#\n######", "LL"), grid("#<..E#\n######"));
}

#[test]
fn walking_off_a_ledge_falls_to_the_floor() {
    assert_eq!(after("#>...#\n##...#\n##..E#\n######", "R"), grid("#....#\n##...#\n##>.E#\n######"));
}

#[test]
fn a_carried_block_falls_with_you() {
    assert_eq!(after("#B...#\n#>...#\n##..E#\n######", "R"), grid("#....#\n#.B..#\n##>.E#\n######"));
}

#[test]
fn walking_into_a_wall_or_block_is_refused() {
    assert_eq!(refused("#>#.E#\n######", "R"), Refusal::Blocked);
    assert_eq!(refused("#>B.E#\n######", "R"), Refusal::Blocked);
    assert_eq!(refused("<..E#\n#####", "L"), Refusal::Blocked); // the grid's edge
}

#[test]
fn walking_under_a_low_ceiling_while_carrying_is_refused() {
    assert_eq!(refused("#B##.#\n#>..E#\n######", "R"), Refusal::CarryBlocked);
}

#[test]
fn climbing_steps_up_onto_the_cell_in_front() {
    assert_eq!(after("#....E#\n#>B...#\n#######", "U"), grid("#.>..E#\n#.B...#\n#######"));
    assert_eq!(after("#....E#\n#.#.<.#\n#######", "LU"), grid("#.<..E#\n#.#...#\n#######"));
}

#[test]
fn climbing_takes_the_carried_block_along() {
    assert_eq!(after("#.....#\n#B..E.#\n#>#...#\n#######", "U"), grid("#.B...#\n#.>.E.#\n#.#...#\n#######"));
}

#[test]
fn climbing_with_nothing_in_front_is_refused() {
    assert_eq!(refused("#>..E#\n######", "U"), Refusal::NothingToClimb);
}

#[test]
fn climbing_without_headroom_is_refused() {
    assert_eq!(refused("#.#.E#\n#>#..#\n######", "U"), Refusal::NoHeadroom); // above the step
    assert_eq!(refused("##..E#\n#>#..#\n######", "U"), Refusal::NoHeadroom); // above you
}

#[test]
fn climbing_where_the_carried_block_would_hit_is_refused() {
    assert_eq!(refused("#.#...#\n#B..E.#\n#>#...#\n#######", "U"), Refusal::CarryBlocked);
}

#[test]
fn down_lifts_the_block_in_front_onto_your_head() {
    let l = level("#.....#\n#>B.E.#\n#######");
    let s = step(&l, &l.start, Key::D).unwrap();
    assert!(s.carrying);
    assert_eq!(l.show(&s), grid("#B....#\n#>..E.#\n#######"));
}

#[test]
fn down_while_carrying_puts_the_block_in_front_and_it_falls() {
    assert_eq!(after("#B....#\n#>..E.#\n#######", "D"), grid("#.....#\n#>B.E.#\n#######")); // onto the floor
    assert_eq!(after("#B....#\n#>B.E.#\n#######", "D"), grid("#.B...#\n#>B.E.#\n#######")); // onto a block
    assert_eq!(after("#B....#\n#>..E.#\n##.####", "D"), grid("#.....#\n#>..E.#\n##B####")); // down a pit
}

#[test]
fn lifting_with_no_block_in_front_is_refused() {
    assert_eq!(refused("#>..E#\n######", "D"), Refusal::NoBlock);
    assert_eq!(refused("#>#.E#\n######", "D"), Refusal::NoBlock); // walls stay put
}

#[test]
fn lifting_a_block_with_something_on_it_is_refused() {
    assert_eq!(refused("#.B.E#\n#>B..#\n######", "D"), Refusal::Covered);
}

#[test]
fn lifting_under_a_low_ceiling_is_refused() {
    assert_eq!(refused("##...#\n#>B.E#\n######", "D"), Refusal::NoHeadroom);
}

#[test]
fn putting_down_where_head_height_in_front_is_filled_is_refused() {
    assert_eq!(refused("#B#..#\n#>#.E#\n######", "D"), Refusal::NoRoom);
}

fn play(text: &str) -> Play {
    Play::new(level(text))
}

#[test]
fn standing_on_the_exit_solves_and_later_keys_are_ignored() {
    let mut p = play("#>.E#\n#####");
    let a = p.apply("RRLL");
    assert_eq!(a, Applied { played: 2, stop: None, solved: true });
    assert!(p.solved());
    assert_eq!(p.moves(), 2);
    assert_eq!(p.apply("L"), Applied { played: 0, stop: None, solved: true });
}

#[test]
fn a_move_string_stops_at_the_first_refusal_and_says_why() {
    let mut p = play("#>#.E#\n######");
    let a = p.apply("LRRL");
    assert_eq!(a, Applied { played: 2, stop: Some(Stop { index: 2, key: 'R', why: Refusal::Blocked }), solved: false });
    assert_eq!(p.moves(), 2, "a refused key is not a move");
    assert_eq!(p.show(), grid("#>#.E#\n######"));
}

#[test]
fn a_letter_that_is_not_a_key_stops_the_string() {
    let mut p = play("#>..E#\n######");
    let a = p.apply("Rx");
    assert_eq!(a, Applied { played: 1, stop: Some(Stop { index: 1, key: 'x', why: Refusal::NotAKey }), solved: false });
}

#[test]
fn undo_takes_back_moves_and_restart_clears_them() {
    let start = grid("#>...E#\n#######");
    let mut p = play(&start);
    p.apply("RRR");
    assert_eq!(p.undo(1), 1);
    assert_eq!((p.moves(), p.show()), (2, after(&start, "RR")));
    assert_eq!(p.undo(5), 2, "undo stops at the start");
    assert_eq!((p.moves(), p.show()), (0, start.clone()));
    p.apply("RR");
    p.restart();
    assert_eq!((p.moves(), p.show()), (0, start));
}

/// xorshift64*: enough randomness for property tests without a dependency.
struct Rng(u64);
impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 33) as usize % n
    }
}

/// A random settled level: a walled pit with scattered wall cells, blocks dropped down their columns, the player and
/// the exit dropped onto whatever is under them.
fn random_level(rng: &mut Rng) -> Level {
    let (w, h) = (6 + rng.below(MAX_W - 5), 4 + rng.below(MAX_H - 3));
    let mut g = vec![vec!['.'; w]; h];
    for (y, row) in g.iter_mut().enumerate() {
        for (x, c) in row.iter_mut().enumerate() {
            if x == 0 || x == w - 1 || y == h - 1 || rng.below(7) == 0 {
                *c = '#';
            }
        }
    }
    let drop = |g: &mut Vec<Vec<char>>, x: usize, c: char| {
        let Some(mut y) = (0..h).find(|&y| g[y][x] == '.') else { return };
        while y + 1 < h && g[y + 1][x] == '.' {
            y += 1;
        }
        g[y][x] = c;
    };
    for _ in 0..rng.below(w * h / 4) {
        drop(&mut g, 1 + rng.below(w - 2), 'B');
    }
    drop(&mut g, 1 + rng.below(w - 2), 'E');
    drop(&mut g, 1 + rng.below(w - 2), if rng.below(2) == 0 { '<' } else { '>' });
    let text = g.iter().map(|r| r.iter().collect::<String>()).collect::<Vec<_>>().join("\n");
    // A full column can swallow the player or the exit; such a level is skipped by the caller.
    Level::parse(&text).unwrap_or_else(|_| Level::parse("#>E#\n####").unwrap())
}

#[test]
fn random_play_never_leaves_anything_floating_and_every_state_reads_back() {
    let mut rng = Rng(0x5eed_0fb0_c5ed);
    for _ in 0..500 {
        let l = random_level(&mut rng);
        let mut s = l.start;
        for _ in 0..200 {
            let k = [Key::L, Key::R, Key::U, Key::D][rng.below(4)];
            if let Ok(n) = step(&l, &s, k) {
                s = n;
            }
            if l.solved(&s) {
                break;
            }
            let text = l.show(&s);
            let back = Level::parse(&text)
                .unwrap_or_else(|e| panic!("{e:?} after {k:?}, exit {:?}, {s:?} in\n{text}", l.exit));
            assert_eq!((back.start, back.walls, back.exit), (s, l.walls, l.exit), "in\n{text}");
        }
    }
}
