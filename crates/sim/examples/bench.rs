//! Native timing for one 12k-grain frame, split by phase. `cargo run --release --example bench`
use sim::World;
use std::time::Instant;

fn main() {
    let mut w = World::new(12000, 1280.0, 720.0, 1);
    w.params.spawn_rate = 1e6;
    let (mw, mh) = (640, 360);
    w.mask_alloc(mw, mh, 0.5);
    for y in 120..200 { for x in 120..520 { if (x / 30) % 2 == 0 { w.mask[y * mw + x] = 255; } } }
    w.mask_commit();
    for _ in 0..240 { w.step(1.0 / 60.0); }
    let n = 120;
    let t = Instant::now();
    for _ in 0..n { w.step(1.0 / 60.0); }
    println!("step {:.2} ms", t.elapsed().as_secs_f64() * 1e3 / n as f64);
    for (name, ms) in w.profile(n) { println!("  {name:<10} {ms:.2} ms"); }
}
