//! What an agent (an RL policy, an LLM through `box-mcp`) sees of the game and how it acts, as plain data: the
//! discrete action table and the observation with its flat encoding. `game3d::observe` fills an [`Obs`] from the game;
//! `crates/box-env` steps the game with these actions (ML track, `docs/planning/ml-track.md`).

use crate::controls::Verbs;
use crate::enemy::Kind;

/// Move directions: none, then eight floor directions 45° apart, starting along +x (right) and turning toward +y (away
/// from the camera). All floor positions and directions here are `plane` vectors: x right, y away from the camera.
pub const DIRS: usize = 9;
/// The verbs an action may press: none, dash, punch, kick, grab.
pub const VERBS: usize = 5;
/// Every action: each direction with each verb.
pub const ACTIONS: usize = DIRS * VERBS;

/// One of the [`ACTIONS`] discrete actions: direction `index / VERBS`, verb `index % VERBS`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Action(pub u8);

impl Action {
    pub fn new(dir: usize, verb: usize) -> Action {
        assert!(dir < DIRS && verb < VERBS, "action out of range: dir {dir}, verb {verb}");
        Action((dir * VERBS + verb) as u8)
    }

    /// The floor direction (a `plane` vector, unit or zero) and the verbs pressed.
    pub fn decode(self) -> ([f32; 2], Verbs) {
        let i = self.0 as usize % ACTIONS;
        let (dir, verb) = (i / VERBS, i % VERBS);
        let floor = if dir == 0 {
            [0.0, 0.0]
        } else {
            let a = (dir - 1) as f32 * std::f32::consts::FRAC_PI_4;
            [a.cos(), a.sin()]
        };
        let verbs = Verbs { dash: verb == 1, punch: verb == 2, kick: verb == 3, grab: verb == 4, ..Verbs::default() };
        (floor, verbs)
    }
}

/// Enemies an observation describes, nearest first; the rest are left out.
pub const NEAREST: usize = 6;
/// The pile heightmap's cells across the floor's width (x) and depth (y).
pub const GRID: [usize; 2] = [12, 8];

/// The player, as an observation carries it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerObs {
    /// Plane x, height of the capsule's centre above the floor, plane y; metres from the floor's centre.
    pub at: [f32; 3],
    pub velocity: [f32; 3],
    /// Facing, radians (0 along +x, turning toward +y).
    pub yaw: f32,
    pub hp: f32,
    pub max_hp: f32,
    /// Holding something (an enemy, a piece, a bomb).
    pub carrying: bool,
    /// The top of the pile under the player, metres above the floor: what the escape test measures against the rim.
    pub pile: f32,
}

/// One enemy, as an observation carries it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnemyObs {
    /// Plane offset from the player, metres.
    pub offset: [f32; 2],
    /// Height of its centre above the player's, metres (thrown, or on the pile).
    pub rise: f32,
    pub kind: Kind,
    pub hp: f32,
    pub max_hp: f32,
    /// How far through its strike's wind-up, 0..1 (0: not winding up).
    pub telegraph: f32,
    pub bomb: bool,
    /// Held by the player.
    pub carried: bool,
}

/// Everything an agent sees in one frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Obs {
    pub player: PlayerObs,
    /// At most [`NEAREST`], nearest first.
    pub enemies: Vec<EnemyObs>,
    /// The pile's top in each of the [`GRID`] cells across the floor, metres; row-major, x fastest, from -x -y (the
    /// front left corner).
    pub heights: Vec<f32>,
    /// The floor's width (x) and depth (y), metres.
    pub floor: [f32; 2],
    pub rim: f32,
    /// 1-based.
    pub box_number: usize,
    pub time_left: f32,
    /// The box's full clock, seconds.
    pub timer: f32,
    /// Whether each of the four doors is warning or letting someone in.
    pub doors: [bool; 4],
}

/// Floats in [`Obs::flat`].
pub const FLAT: usize = PLAYER_FLAT + NEAREST * ENEMY_FLAT + GRID[0] * GRID[1] + 4 + BOX_FLAT;
const PLAYER_FLAT: usize = 11;
const ENEMY_FLAT: usize = 10;
const BOX_FLAT: usize = 3;
/// Speeds are divided by this to bring them near 1 (a dash is a little over the run).
const SPEED: f32 = crate::plane::RUN_SPEED;
/// Box numbers are divided by this (the rims stop rising at box 4, the clocks at box 5).
const BOXES: f32 = 5.0;

impl Obs {
    /// A fixed-length vector for a policy network, [`FLAT`] long, every entry roughly in -1..1 (heights in rims, so
    /// 1 is the escape line; positions in half-floors; speeds in run speeds). Absent enemies are all zeros, with
    /// their "present" entry 0.
    pub fn flat(&self) -> Vec<f32> {
        let half = [self.floor[0] / 2.0, self.floor[1] / 2.0];
        let rim = self.rim.max(1e-3);
        let p = &self.player;
        let mut v = Vec::with_capacity(FLAT);
        v.extend([
            p.at[0] / half[0],
            p.at[1] / rim,
            p.at[2] / half[1],
            p.velocity[0] / SPEED,
            p.velocity[1] / SPEED,
            p.velocity[2] / SPEED,
            p.yaw.cos(),
            p.yaw.sin(),
            p.hp / p.max_hp.max(1e-3),
            p.carrying as u8 as f32,
            p.pile / rim,
        ]);
        for i in 0..NEAREST {
            match self.enemies.get(i) {
                Some(e) => v.extend([
                    1.0,
                    e.offset[0] / half[0],
                    e.offset[1] / half[1],
                    e.rise / rim,
                    (e.kind == Kind::Thug) as u8 as f32,
                    (e.kind == Kind::Ninja) as u8 as f32,
                    e.hp / e.max_hp.max(1e-3),
                    e.telegraph,
                    e.bomb as u8 as f32,
                    e.carried as u8 as f32,
                ]),
                None => v.extend([0.0; ENEMY_FLAT]),
            }
        }
        v.extend(self.heights.iter().map(|h| h / rim));
        v.resize(PLAYER_FLAT + NEAREST * ENEMY_FLAT + GRID[0] * GRID[1], 0.0);
        v.extend(self.doors.map(|d| d as u8 as f32));
        v.extend([
            self.box_number as f32 / BOXES,
            self.time_left / self.timer.max(1e-3),
            self.rim / crate::run::MAX_RIM,
        ]);
        debug_assert_eq!(v.len(), FLAT);
        v
    }
}

/// The plane centre of heightmap cell `i` (row-major, x fastest) on a floor `floor` wide and deep, centred on the
/// origin.
pub fn cell_centre(i: usize, floor: [f32; 2]) -> [f32; 2] {
    let (cx, cz) = (i % GRID[0], i / GRID[0]);
    [
        -floor[0] / 2.0 + (cx as f32 + 0.5) * floor[0] / GRID[0] as f32,
        -floor[1] / 2.0 + (cz as f32 + 0.5) * floor[1] / GRID[1] as f32,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs() -> Obs {
        Obs {
            player: PlayerObs { at: [1.0, 0.9, -0.5], max_hp: 100.0, hp: 50.0, ..PlayerObs::default() },
            enemies: vec![EnemyObs {
                offset: [2.0, 0.0],
                rise: 0.0,
                kind: Kind::Thug,
                hp: 30.0,
                max_hp: 60.0,
                telegraph: 0.5,
                bomb: true,
                carried: false,
            }],
            heights: vec![0.75; GRID[0] * GRID[1]],
            floor: [12.0, 8.0],
            rim: 1.5,
            box_number: 1,
            time_left: 45.0,
            timer: 90.0,
            doors: [false, true, false, false],
        }
    }

    #[test]
    fn every_action_decodes_to_its_direction_and_verb() {
        let mut seen = std::collections::HashSet::new();
        for dir in 0..DIRS {
            for verb in 0..VERBS {
                let (floor, verbs) = Action::new(dir, verb).decode();
                let len = (floor[0] * floor[0] + floor[1] * floor[1]).sqrt();
                assert!(if dir == 0 { len == 0.0 } else { (len - 1.0).abs() < 1e-6 }, "dir {dir}: {floor:?}");
                let pressed = [verbs.dash, verbs.punch, verbs.kick, verbs.grab];
                assert_eq!(pressed.iter().filter(|p| **p).count(), (verb != 0) as usize, "verb {verb}");
                assert!(!verbs.pause && !verbs.begin && !verbs.sound, "an agent never pauses or toggles sound");
                seen.insert(((floor[0] * 100.0).round() as i32, (floor[1] * 100.0).round() as i32, pressed));
            }
        }
        assert_eq!(seen.len(), ACTIONS, "all distinct");
        let (east, _) = Action::new(1, 0).decode();
        let (away, _) = Action::new(3, 0).decode();
        assert!(east[0] > 0.99 && away[1] > 0.99, "direction 1 is +x, direction 3 is +y");
    }

    #[test]
    fn the_flat_encoding_has_a_fixed_length_and_scales_to_rims_and_half_floors() {
        let v = obs().flat();
        assert_eq!(v.len(), FLAT);
        assert!((v[0] - 1.0 / 6.0).abs() < 1e-6 && (v[2] + 0.5 / 4.0).abs() < 1e-6, "positions in half-floors");
        assert!((v[8] - 0.5).abs() < 1e-6, "health as a share");
        let e = PLAYER_FLAT;
        assert_eq!(&v[e..e + ENEMY_FLAT], &[1.0, 2.0 / 6.0, 0.0, 0.0, 1.0, 0.0, 0.5, 0.5, 1.0, 0.0]);
        assert!(v[e + ENEMY_FLAT..e + 2 * ENEMY_FLAT].iter().all(|x| *x == 0.0), "an absent enemy is zeros");
        let h = PLAYER_FLAT + NEAREST * ENEMY_FLAT;
        assert!(v[h..h + GRID[0] * GRID[1]].iter().all(|x| (x - 0.5).abs() < 1e-6), "heights in rims");
        assert_eq!(&v[FLAT - 7..FLAT - 3], &[0.0, 1.0, 0.0, 0.0], "doors");
        assert!((v[FLAT - 2] - 0.5).abs() < 1e-6, "the clock as a share");
    }

    #[test]
    fn cells_tile_the_floor_from_its_front_left_corner() {
        let floor = [12.0, 8.0];
        assert_eq!(cell_centre(0, floor), [-5.5, -3.5]);
        assert_eq!(cell_centre(GRID[0] * GRID[1] - 1, floor), [5.5, 3.5]);
    }
}
