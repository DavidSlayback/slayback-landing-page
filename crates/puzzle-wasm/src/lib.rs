//! C-ABI exports of *Out of the Box* for `/puzzle` (`site/puzzle/puzzle.js`).
//!
//! The page creates a game for a tier and seed, sends keys, and each animation frame calls `game_frame`, which draws
//! into an RGBA buffer it reads straight out of WASM memory. No wasm-bindgen: the module has no imports. `game_new`
//! allocates and may grow memory, so the page re-takes `memory.buffer` views after it.

// The exports take raw pointers from JS by design (see `game`); this crate is the FFI boundary.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

pub mod art;
pub mod draw;
pub mod game;
pub mod motion;

use game::Game;
use puzzle_core::Key;

fn game<'a>(p: *mut Game) -> &'a mut Game {
    // SAFETY: JS only passes pointers returned by `game_new` and never after `game_free`.
    unsafe { &mut *p }
}

/// A game on the level for `seed` in `world` (`puzzle_core::World`'s numbers) and `tier` (1 to 4), from a later seed
/// if that one makes no level (`game_seed` says which), or null if there is none.
#[unsafe(no_mangle)]
pub extern "C" fn game_new(world: u32, tier: u32, seed: u64) -> *mut Game {
    match Game::new(world as u8, tier as u8, seed) {
        Some(g) => Box::into_raw(Box::new(g)),
        None => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn game_free(p: *mut Game) {
    if !p.is_null() {
        // SAFETY: `p` came from `game_new` (Box::into_raw) and is freed once.
        drop(unsafe { Box::from_raw(p) });
    }
}

/// Plays a key (0 L, 1 R, 2 U, 3 D): 0 if it played or the level is solved, else 1 plus the refusal's index in
/// `puzzle_core::Refusal`.
#[unsafe(no_mangle)]
pub extern "C" fn game_key(p: *mut Game, key: u32) -> u32 {
    let Some(&k) = [Key::L, Key::R, Key::U, Key::D].get(key as usize) else { return 0 };
    game(p).key(k).map_or(0, |r| r as u32 + 1)
}

#[unsafe(no_mangle)]
pub extern "C" fn game_undo(p: *mut Game) {
    game(p).undo();
}

#[unsafe(no_mangle)]
pub extern "C" fn game_restart(p: *mut Game) {
    game(p).restart();
}

/// Advances `dt` seconds, draws, and returns the RGBA pixels (`game_width` × `game_height`).
#[unsafe(no_mangle)]
pub extern "C" fn game_frame(p: *mut Game, dt: f32) -> *const u8 {
    let g = game(p);
    g.frame(dt);
    g.canvas.rgba.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn game_width(p: *mut Game) -> u32 {
    game(p).canvas.w as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn game_height(p: *mut Game) -> u32 {
    game(p).canvas.h as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn game_moves(p: *mut Game) -> u32 {
    game(p).play.moves() as u32
}

/// The length of the level's shortest solution.
#[unsafe(no_mangle)]
pub extern "C" fn game_optimal(p: *mut Game) -> u32 {
    game(p).optimal as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn game_solved(p: *mut Game) -> u32 {
    game(p).play.solved() as u32
}

/// The seed the level came from: the one asked for, or a later one when that one made no level.
#[unsafe(no_mangle)]
pub extern "C" fn game_seed(p: *mut Game) -> u64 {
    game(p).seed
}

/// The level's fingerprint, as in the built-in levels' `crates/puzzle-core/levels/page.json`.
#[unsafe(no_mangle)]
pub extern "C" fn game_fingerprint(p: *mut Game) -> u64 {
    game(p).play.level.fingerprint()
}

/// Sets a scheme colour (0 fg, 1 muted, 2 accent), each channel 0..1.
#[unsafe(no_mangle)]
pub extern "C" fn game_color(p: *mut Game, which: u32, r: f32, g: f32, b: f32) {
    if let Some(c) = game(p).colors.get_mut(which as usize) {
        *c = [r, g, b];
    }
}

/// The screen shake this frame in canvas pixels (0 across, 1 down); the page moves the canvas by it.
#[unsafe(no_mangle)]
pub extern "C" fn game_shake(p: *mut Game, axis: u32) -> f32 {
    game(p).shake(axis)
}

#[unsafe(no_mangle)]
pub extern "C" fn game_reduce_motion(p: *mut Game, on: u32) {
    game(p).reduce_motion(on != 0);
}

#[cfg(test)]
mod tests {
    /// The page sends a world's index in its `WORLDS` list to `game_new`, which reads it as `World`'s number.
    #[test]
    fn the_pages_world_list_is_in_world_number_order() {
        let js = include_str!("../../../site/puzzle/puzzle.js");
        let names: Vec<_> = puzzle_core::World::ALL.iter().map(|w| format!("'{}'", w.name())).collect();
        assert!(js.contains(&format!("const WORLDS = [{}];", names.join(", "))), "puzzle.js WORLDS");
    }
}
