//! Diagnostic: mean speed of a heavy pile after settling, at several frame rates and substep counts.
use hero_sim::{Params, World};
fn main() {
    for fps in [60.0f32, 30.0] {
        let mut w = World::new(600, 300.0, 200.0, 7);
        w.set_params(Params { gravity: 900.0, wind: 0.0, turb: 0.0, drag: 0.0, spawn_rate: 0.0, ..Params::default() });
        for k in 0..600 { w.put(20.0 + (k % 40) as f32 * 6.5, 10.0 + (k / 40) as f32 * 6.0, 0.0, 0.0); }
        for _ in 0..(8.0 * fps) as usize { w.step(1.0 / fps); }
        let mean = (0..w.active).map(|i| w.vx[i].hypot(w.vy[i])).sum::<f32>() / w.active as f32;
        let top = (0..w.active).map(|i| w.y[i]).fold(f32::MAX, f32::min);
        println!("{fps} fps: mean speed {mean:.1} px/s, pile top y {top:.1}");
    }
}
