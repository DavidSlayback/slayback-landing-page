//! How fast the environment steps on one core: `bash scripts/native.sh run -p box-env --release --example steps`
//! (`STEPS=5000` to change the count). Random actions, so the box fills and the physics has work to do.

use std::time::Instant;

use box_env::{ACTIONS, Action, Env, EnvConfig};

fn main() {
    let steps: usize = std::env::var("STEPS").ok().and_then(|s| s.parse().ok()).unwrap_or(3000);
    let config = EnvConfig::default();
    let t = Instant::now();
    let mut env = Env::new(config);
    let built = t.elapsed();
    let mut s = 0x2545_F491u32;
    let (mut done, mut episodes) = (0usize, 1usize);
    let t = Instant::now();
    while done < steps {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        if env.step(Action((s % ACTIONS as u32) as u8)).done {
            env.reset(episodes as u32);
            episodes += 1;
        }
        done += 1;
    }
    let secs = t.elapsed().as_secs_f64();
    let frames = done as f64 * config.frame_skip as f64;
    println!(
        "{done} steps ({frames} frames, {:.0} s of play) in {secs:.2} s: {:.0} steps/s, {:.0} frames/s ({:.1}x real time); \
         {episodes} episode(s); first build {:.0} ms",
        frames / 60.0,
        done as f64 / secs,
        frames / secs,
        frames / 60.0 / secs,
        built.as_secs_f64() * 1000.0
    );
}
