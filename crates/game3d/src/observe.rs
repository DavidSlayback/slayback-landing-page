//! What an agent sees of the game this frame (`game_core::agent::Obs`), read from the world: the RL environment
//! (`crates/box-env`) and the box's MCP server observe through it, and so will a trained policy playing in the page.
//! Positions are `plane` vectors (x right, y away from the camera), as the agent's actions are.

use avian3d::prelude::*;
use bevy::prelude::*;
use game_core::agent::{EnemyObs, GRID, NEAREST, Obs, PlayerObs, cell_centre};
use game_core::doors::Door;

use crate::arena::Arena;
use crate::bomb::BombHead;
use crate::carry::{Carried, Carry};
use crate::debris::{Piece, pile_under};
use crate::enemy::{Brain, WallDoor};
use crate::fighter::Hp;
use crate::player::{Facing, Player};
use crate::run::{RunState, standing_on};
use crate::{plane, plane_on};

/// The observation now. Before the player exists (the first frame) it describes an empty box with a dead player.
pub fn observe(world: &mut World) -> Obs {
    let arena = *world.resource::<Arena>();
    let run = world.resource::<RunState>().0;
    let floor = [arena.floor.x, arena.floor.y];

    // Every query's state first (they need the world mutably), then read them all through one shared borrow.
    let mut piece_q = world.query::<(&Piece, &Transform)>();
    let mut player_q = world.query_filtered::<(&Position, &LinearVelocity, &Facing, &Hp, Has<Carry>), With<Player>>();
    let mut enemy_q = world.query::<(&Brain, &Position, &Hp, Has<BombHead>, Has<Carried>)>();
    let mut door_q = world.query::<&WallDoor>();
    let world: &World = world;

    let pieces: Vec<(&Piece, &Transform)> = piece_q.iter(world).collect();
    let pile = || pieces.iter().copied();

    let player = player_q.iter(world).next().map(|(&Position(at), v, facing, hp, carrying)| {
        let p = plane(at);
        let u = plane(v.0);
        PlayerObs {
            at: [p[0], at.y, p[1]],
            velocity: [u[0], v.0.y, u[1]],
            yaw: facing.0.yaw,
            hp: hp.0.hp,
            max_hp: hp.0.max,
            carrying,
            pile: standing_on(pile(), at),
        }
    });
    let player = player.unwrap_or_default();
    let me = [player.at[0], player.at[2]];

    let mut enemies: Vec<EnemyObs> = enemy_q
        .iter(world)
        .map(|(brain, &Position(at), hp, bomb, carried)| {
            let p = plane(at);
            EnemyObs {
                offset: [p[0] - me[0], p[1] - me[1]],
                rise: at.y - player.at[1],
                kind: brain.0.style().kind,
                hp: hp.0.hp,
                max_hp: hp.0.max,
                telegraph: brain.0.telegraph(),
                bomb,
                carried,
            }
        })
        .collect();
    // Nearest first; ties (rare: two enemies equally far) keep a fixed order by offset, so observations repeat.
    enemies.sort_by(|a, b| {
        let d = |e: &EnemyObs| e.offset[0] * e.offset[0] + e.offset[1] * e.offset[1];
        d(a).total_cmp(&d(b)).then(a.offset[0].total_cmp(&b.offset[0])).then(a.offset[1].total_cmp(&b.offset[1]))
    });
    enemies.truncate(NEAREST);

    // A cell's height is the pile's top anywhere over most of the cell.
    let reach = 0.5 * (floor[0] / GRID[0] as f32).min(floor[1] / GRID[1] as f32);
    let heights =
        (0..GRID[0] * GRID[1]).map(|i| pile_under(pile(), plane_on(cell_centre(i, floor)), reach, f32::MAX)).collect();

    // The doors in a fixed order (round the walls by where they stand), however the world stores them.
    let mut doors: Vec<(Vec3, bool)> = door_q.iter(world).map(|d| (d.face, d.door != Door::Closed)).collect();
    doors.sort_by(|a, b| a.0.x.total_cmp(&b.0.x).then(a.0.z.total_cmp(&b.0.z)));
    let mut open = [false; 4];
    for (slot, (_, busy)) in open.iter_mut().zip(doors) {
        *slot = busy;
    }

    Obs {
        player,
        enemies,
        heights,
        floor,
        rim: arena.rim,
        box_number: run.spec.number,
        time_left: run.time_left,
        timer: run.spec.timer,
        doors: open,
    }
}
