//! `box-vec [--n N] [--seed S] [--frame-skip F] [--max-steps T] [--rim M]`: N environments for a trainer in another
//! language (`ml/boxenv.py`), over stdin and stdout in a plain binary protocol, little-endian throughout. Each episode
//! that ends is reset at once with the next seed (`seed`, `seed + 1`, ...), so the stream never stops.
//!
//! - On start it writes a header, `n`, `flat` (floats per observation) and `actions` (u32 each), then the first
//!   observations, `n * flat` f32.
//! - Then, per step, it reads `n` action bytes and writes:
//!   - `obs`, `n * flat` f32 (after a reset, the new episode's first);
//!   - `reward`, `n` f32;
//!   - `done` and `truncated`, `n` u8 each;
//!   - for each env whose episode ended: its `score` (the progress score), as `n` f32;
//!   - `escapes`, `kills` and `length` (steps), `n` u32 each (all 0 for an env still in its episode).
//!
//! End of input ends it.

use std::io::{BufReader, BufWriter, Read, Write};

use box_env::{ACTIONS, Action, Env, EnvConfig, FLAT};

fn main() {
    let (mut n, mut config) = (8usize, EnvConfig::default());
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let v = args.next().unwrap_or_else(|| fail(&format!("{flag} needs a value")));
        match flag.as_str() {
            "--n" => n = num(&v),
            "--seed" => config.seed = num(&v),
            "--frame-skip" => config.frame_skip = num(&v),
            "--max-steps" => config.max_steps = num(&v),
            "--rim" => config.rim = Some(num(&v)),
            _ => fail(&format!("unknown argument {flag}")),
        }
    }
    let mut next_seed = config.seed;
    let mut envs: Vec<Env> = (0..n)
        .map(|_| {
            let env = Env::new(EnvConfig { seed: next_seed, ..config });
            next_seed += 1;
            env
        })
        .collect();

    let mut input = BufReader::new(std::io::stdin().lock());
    let mut out = BufWriter::new(std::io::stdout().lock());
    let put = |out: &mut BufWriter<_>, bytes: &[u8]| out.write_all(bytes).unwrap_or_else(|e| fail(&e.to_string()));
    for v in [n as u32, FLAT as u32, ACTIONS as u32] {
        put(&mut out, &v.to_le_bytes());
    }
    for env in &envs {
        for x in env.last().flat() {
            put(&mut out, &x.to_le_bytes());
        }
    }
    out.flush().ok();

    let mut actions = vec![0u8; n];
    let (mut obs, mut reward) = (Vec::with_capacity(n * FLAT), vec![0f32; n]);
    let (mut done, mut truncated) = (vec![0u8; n], vec![0u8; n]);
    let (mut score, mut escapes, mut kills, mut length) = (vec![0f32; n], vec![0u32; n], vec![0u32; n], vec![0u32; n]);
    while input.read_exact(&mut actions).is_ok() {
        obs.clear();
        for (i, env) in envs.iter_mut().enumerate() {
            let step = env.step(Action(actions[i] % ACTIONS as u8));
            reward[i] = step.reward;
            done[i] = step.done as u8;
            truncated[i] = step.truncated as u8;
            (score[i], escapes[i], kills[i], length[i]) = (0.0, 0, 0, 0);
            if step.done {
                let o = env.outcome();
                (score[i], escapes[i], kills[i], length[i]) =
                    (f32::from_bits(o.reward_bits), o.escapes, o.kills, o.steps);
                env.reset(next_seed);
                next_seed += 1;
            }
            obs.extend(env.last().flat());
        }
        for x in &obs {
            put(&mut out, &x.to_le_bytes());
        }
        for x in &reward {
            put(&mut out, &x.to_le_bytes());
        }
        put(&mut out, &done);
        put(&mut out, &truncated);
        for x in &score {
            put(&mut out, &x.to_le_bytes());
        }
        for column in [&escapes, &kills, &length] {
            for x in column {
                put(&mut out, &x.to_le_bytes());
            }
        }
        if out.flush().is_err() {
            break;
        }
    }
}

fn num<T: std::str::FromStr>(s: &str) -> T {
    s.parse().unwrap_or_else(|_| fail(&format!("not a number: {s}")))
}

fn fail(message: &str) -> ! {
    eprintln!("box-vec: {message}");
    std::process::exit(2)
}
