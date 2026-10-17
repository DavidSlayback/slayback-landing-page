//! C-ABI exports of `sim` for the browser hero (`site/hero.js`).
//!
//! JS creates a world, writes text masks into WASM memory through `world_mask_alloc`, steps it, and reads
//! particle state straight out of memory via the returned pointers. No wasm-bindgen: the module has no imports.
//! Any call that allocates (`world_new`, `world_mask_alloc`) may grow memory, so JS must re-take
//! `memory.buffer` views after it.

// The exports take raw pointers from JS by design (see `world`); this crate is the FFI boundary.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use sim::World;

fn world<'a>(p: *mut World) -> &'a mut World {
    // SAFETY: JS only passes pointers returned by `world_new` and never after `world_free`.
    unsafe { &mut *p }
}

#[unsafe(no_mangle)]
pub extern "C" fn world_new(cap: u32, w: f32, h: f32, seed: u32) -> *mut World {
    Box::into_raw(Box::new(World::new(cap as usize, w, h, seed)))
}

#[unsafe(no_mangle)]
pub extern "C" fn world_free(p: *mut World) {
    if !p.is_null() {
        // SAFETY: `p` came from `world_new` (Box::into_raw) and is freed once.
        drop(unsafe { Box::from_raw(p) });
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn world_set(p: *mut World, key: u32, value: f32) {
    world(p).set(key, value);
}

#[unsafe(no_mangle)]
pub extern "C" fn world_shield(p: *mut World, x: f32, y: f32, r: f32) {
    world(p).set_shield(x, y, r);
}

#[unsafe(no_mangle)]
pub extern "C" fn world_shield_hits(p: *mut World) -> u32 {
    world(p).shield_hits
}

/// Emitters as 8 floats each: x, y, angle, power, hue, 0, 0, 0.
#[unsafe(no_mangle)]
pub extern "C" fn world_emitters_ptr(p: *mut World) -> *const f32 {
    world(p).emitter_out().as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn world_emitter_count(p: *mut World) -> u32 {
    world(p).emitters.len() as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn world_mask_alloc(p: *mut World, mw: u32, mh: u32, scale: f32) -> *mut u8 {
    world(p).mask_alloc(mw as usize, mh as usize, scale)
}

#[unsafe(no_mangle)]
pub extern "C" fn world_mask_commit(p: *mut World) {
    world(p).mask_commit();
}

#[unsafe(no_mangle)]
pub extern "C" fn world_sdf_ptr(p: *mut World) -> *const f32 {
    world(p).sdf.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn world_cursor(p: *mut World, x: f32, y: f32, on: u32) {
    world(p).set_cursor(x, y, on != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn world_burst(p: *mut World, x: f32, y: f32, radius: f32, strength: f32) {
    world(p).burst(x, y, radius, strength);
}

#[unsafe(no_mangle)]
pub extern "C" fn world_step(p: *mut World, dt: f32) {
    world(p).step(dt);
}

/// The current field, u and v interleaved per cell (see `world_flow_dims`), refreshed every step.
#[unsafe(no_mangle)]
pub extern "C" fn world_flow_ptr(p: *mut World) -> *const f32 {
    world(p).flow_out.as_ptr()
}

/// The current field's grid: 0 = cells across, 1 = cells down, 2 = a cell's size in CSS px.
#[unsafe(no_mangle)]
pub extern "C" fn world_flow_dims(p: *mut World, which: u32) -> f32 {
    let (w, h, cell) = world(p).flow_dims();
    [w as f32, h as f32, cell][which.min(2) as usize]
}

#[unsafe(no_mangle)]
pub extern "C" fn world_out_ptr(p: *mut World) -> *const f32 {
    world(p).out.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn world_active(p: *mut World) -> u32 {
    world(p).active as u32
}
