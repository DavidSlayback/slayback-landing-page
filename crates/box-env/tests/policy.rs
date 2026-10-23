//! The Rust forward pass gives the logits the Python trainers' reference does (`ml/policy.py`), for the fixture
//! `ml/make_fixture.py` wrote; and a policy plays a run.

use box_env::policy::Policy;
use box_env::{Env, EnvConfig};

#[test]
fn rust_and_python_agree_on_a_policys_logits() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let policy = Policy::load(&dir.join("policy.bin")).expect("the fixture loads");
    let want: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("policy.json")).unwrap()).unwrap();
    for (obs, logits) in want["obs"].as_array().unwrap().iter().zip(want["logits"].as_array().unwrap()) {
        let x: Vec<f32> = obs.as_array().unwrap().iter().map(|v| v.as_f64().unwrap() as f32).collect();
        let got = policy.logits(&x);
        for (g, w) in got.iter().zip(logits.as_array().unwrap()) {
            let w = w.as_f64().unwrap() as f32;
            assert!((g - w).abs() < 1e-4 * (1.0 + w.abs()), "{g} vs {w}");
        }
    }
}

#[test]
fn a_policy_plays_a_run_and_its_trace_replays() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let policy = Policy::load(&dir.join("policy.bin")).unwrap();
    let mut env = Env::new(EnvConfig { seed: 3, max_steps: 300, rim: Some(box_env::MINI_RIM), ..EnvConfig::default() });
    while !env.done() {
        let a = policy.act(env.last());
        env.step(a);
    }
    assert_eq!(box_env::replay(&env.trace()), env.outcome());
}

#[test]
fn a_file_that_isnt_a_policy_is_refused() {
    assert!(Policy::from_bytes(b"nope").is_err());
    assert!(Policy::from_bytes(b"BOXP\x02\0\0\0\0\0\0\0").is_err(), "a later version");
}
