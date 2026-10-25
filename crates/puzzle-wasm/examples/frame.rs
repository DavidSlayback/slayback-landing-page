//! What drawing costs on a tier-4 board (640 × 384): a frame, which redraws only what moves over the baked still
//! scene, and a re-bake of that scene, which happens when a block comes to rest, on undo and on restart.
//!
//! `bash scripts/native.sh run -p puzzle-wasm --release --example frame`
//!
//! Native timings run several times faster than wasm on a phone; see `tests/e2e/tools/puzzle-frame.mjs` for the
//! browser.

use std::time::Instant;

use puzzle_core::Key;
use puzzle_wasm::draw::Canvas;
use puzzle_wasm::game::Game;

fn ms(n: usize, mut f: impl FnMut()) -> f64 {
    f();
    let t = Instant::now();
    for _ in 0..n {
        f();
    }
    t.elapsed().as_secs_f64() * 1e3 / n as f64
}

fn main() {
    let mut g = Game::new(4, 1).expect("tier 4 seed 1");
    let (w, h) = (g.canvas.w, g.canvas.h);
    let still = ms(300, || g.frame(1.0 / 60.0));
    let mut turn = 0;
    let moving = ms(300, || {
        // A turn every frame keeps a tween going, as fast play does.
        turn += 1;
        g.key(if turn % 2 == 0 { Key::L } else { Key::R });
        g.frame(1.0 / 60.0);
    });
    let mut c = Canvas::new(w, h);
    let bake = ms(100, || {
        c.clear([0.0; 3]);
        c.bake(5, 1.5)
    });
    println!("{w}x{h}: frame {still:.2} ms still, {moving:.2} ms moving | re-bake {bake:.2} ms");
}
