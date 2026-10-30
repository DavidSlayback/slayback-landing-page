//! Tidying a generated level so it reads as a pit, not rubble. The climb toggles wall cells and piles in blocks
//! wherever that keeps the level hard, so it leaves floating walls, buried blocks and sealed holes. [`tidy`] takes out
//! everything the puzzle doesn't need, keeping its difficulty; [`seal`] only fills what the player can never reach, at
//! once, for levels generated on the page.

use crate::generate::score;
use crate::{Key, Level, Tier, World};

/// `level` (with `solution`, a shortest one) with what its puzzle doesn't need taken out: every wall cell, tile and
/// block, top row first and each row left to right, whose removal leaves the level what its world and tier want and
/// its shortest solution as long; then [`seal`]ed. A wall is tried only where the player could stand beside it:
/// opening one deeper in the ground would only make a hole for `seal` to fill. Returns the tidied level and its own
/// shortest solution. Tier 4 takes seconds to tens of seconds natively.
pub fn tidy(level: &Level, solution: Vec<Key>, world: World, tier: &Tier) -> (Level, Vec<Key>) {
    let moves = solution.len();
    let mut best = (level.clone(), solution);
    let (w, h) = (level.w as usize, level.h as usize);
    for y in 0..h - 1 {
        for x in 1..w - 1 {
            let mut cells = rows(&best.0);
            match cells[y][x] {
                b'#' | b'^' | b'~' if !beside(&reach(&cells), w, h, x, y) => continue,
                b'#' | b'^' | b'~' | b'B' | b'I' => cells[y][x] = b'.',
                _ => continue,
            }
            // A cell holding something up can't go: the level would have something floating.
            let Ok(next) = parse(&cells) else { continue };
            if let Some((true, shorter)) = score(&next, world, tier)
                && shorter.len() == moves
            {
                best = (next, shorter);
            }
        }
    }
    (seal(&best.0), best.1)
}

/// `level` with every cell the player can never get to filled with wall, blocks and tiles in it included. It plays
/// exactly as before.
pub fn seal(level: &Level) -> Level {
    let mut cells = rows(level);
    let w = level.w as usize;
    let reached = reach(&cells);
    for (y, row) in cells.iter_mut().enumerate() {
        for (x, c) in row.iter_mut().enumerate() {
            if !reached[y * w + x] && matches!(*c, b'.' | b'B' | b'I' | b'~') {
                *c = b'#';
            }
        }
    }
    parse(&cells).expect("filling cells no one reaches leaves nothing floating")
}

/// A level's text form, a row of bytes per grid row.
fn rows(level: &Level) -> Vec<Vec<u8>> {
    level.to_string().lines().map(|l| l.bytes().collect()).collect()
}

fn parse(cells: &[Vec<u8>]) -> Result<Level, crate::ParseError> {
    let text: Vec<String> = cells.iter().map(|r| String::from_utf8_lossy(r).into_owned()).collect();
    Level::parse(&text.join("\n"))
}

/// Which cells the player could ever get to: everything joined to the player through cells that aren't wall or
/// spring. A cracked tile can break and a block can move, so neither closes the way.
fn reach(cells: &[Vec<u8>]) -> Vec<bool> {
    let (w, h) = (cells[0].len(), cells.len());
    let mut seen = vec![false; w * h];
    let start = (0..w * h).find(|&i| matches!(cells[i / w][i % w], b'<' | b'>')).expect("a player");
    let mut todo = vec![start];
    seen[start] = true;
    while let Some(i) = todo.pop() {
        for n in around(w, h, i % w, i / w) {
            if !seen[n] && !matches!(cells[n / w][n % w], b'#' | b'^') {
                seen[n] = true;
                todo.push(n);
            }
        }
    }
    seen
}

/// Whether cell (x, y) touches a reached cell.
fn beside(reached: &[bool], w: usize, h: usize, x: usize, y: usize) -> bool {
    around(w, h, x, y).any(|n| reached[n])
}

/// The indices of the up to four cells beside (x, y).
fn around(w: usize, h: usize, x: usize, y: usize) -> impl Iterator<Item = usize> {
    [(x.wrapping_sub(1), y), (x + 1, y), (x, y.wrapping_sub(1)), (x, y + 1)]
        .into_iter()
        .filter(move |&(nx, ny)| nx < w && ny < h)
        .map(move |(nx, ny)| ny * w + nx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TIERS, solve};

    #[test]
    fn seal_fills_a_pocket_no_one_reaches_and_nothing_else() {
        let level = Level::parse("#....#\n#>.#.#\n####E#\n#B####\n######").unwrap();
        assert_eq!(seal(&level).to_string(), "#....#\n#>.#.#\n####E#\n######\n######");
    }

    #[test]
    fn tidying_keeps_the_shortest_solution_as_long_and_the_level_what_its_tier_wants() {
        let tier = &TIERS[1];
        let g = crate::generate(2, World::Classic, tier).unwrap();
        let (tidied, solution) = tidy(&g.level, g.solution.clone(), World::Classic, tier);
        assert_eq!(solution.len(), g.solution.len());
        assert_eq!(solve(&tidied).solution.map(|s| s.len()), Some(solution.len()));
        assert_eq!(score(&tidied, World::Classic, tier).map(|(fits, _)| fits), Some(true));
        let count = |l: &Level, cs: &[char]| l.to_string().chars().filter(|c| cs.contains(c)).count();
        assert!(count(&tidied, &['B', 'I']) < count(&g.level, &['B', 'I']), "fewer blocks:\n{}\n\n{tidied}", g.level);
    }
}
