//! The observation as JSON, for agents that read text (the box's MCP server): the same facts as the flat encoding,
//! in metres and seconds with names, rounded to centimetres so a model isn't fed noise digits.

use game_core::agent::{GRID, Obs};
use serde_json::{Value, json};

fn r(x: f32) -> f64 {
    (x as f64 * 100.0).round() / 100.0
}

/// `obs` as JSON (see the module docs). Positions are plane coordinates: x right, y away from the camera.
pub fn obs_json(obs: &Obs) -> Value {
    let p = &obs.player;
    let enemies: Vec<Value> = obs
        .enemies
        .iter()
        .map(|e| {
            json!({
                "kind": e.kind.style().name,
                "offset": [r(e.offset[0]), r(e.offset[1])],
                "distance": r((e.offset[0] * e.offset[0] + e.offset[1] * e.offset[1]).sqrt()),
                "rise": r(e.rise),
                "hp": r(e.hp),
                "max_hp": r(e.max_hp),
                "winding_up": r(e.telegraph),
                "bomb_head": e.bomb,
                "held_by_you": e.carried,
            })
        })
        .collect();
    let rows: Vec<Vec<f64>> = obs.heights.chunks(GRID[0]).map(|row| row.iter().map(|h| r(*h)).collect()).collect();
    json!({
        "box": obs.box_number,
        "rim_height": r(obs.rim),
        "time_left": r(obs.time_left),
        "floor_size": [r(obs.floor[0]), r(obs.floor[1])],
        "you": {
            "position": [r(p.at[0]), r(p.at[2])],
            "height": r(p.at[1]),
            "standing_on_pile": r(p.pile),
            "velocity": [r(p.velocity[0]), r(p.velocity[2])],
            "facing_radians": r(p.yaw),
            "hp": r(p.hp),
            "max_hp": r(p.max_hp),
            "holding": p.carrying,
        },
        "enemies_nearest_first": enemies,
        "pile_heights": {
            "cells": [GRID[0], GRID[1]],
            "rows_front_to_back_left_to_right": rows,
        },
        "doors_opening": obs.doors,
    })
}
