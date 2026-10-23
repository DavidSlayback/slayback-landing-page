//! The environment over the real headless game: episodes start in play, repeat exactly from their trace, and the
//! reward sees what happens in them.

use box_env::{ACTIONS, Action, Env, EnvConfig, FLAT, brawler, obs_json, replay};

/// A fixed pseudo-random policy (xorshift), so tests act the same every run.
fn policy(seed: u32) -> impl FnMut() -> Action {
    let mut s = seed.max(1);
    move || {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        Action((s % ACTIONS as u32) as u8)
    }
}

fn play(config: EnvConfig, steps: usize) -> Env {
    let mut env = Env::new(config);
    let mut act = policy(config.seed ^ 0x5EED);
    for _ in 0..steps {
        if env.step(act()).done {
            break;
        }
    }
    env
}

#[test]
fn an_episode_starts_in_play_in_box_one_at_full_health() {
    let env = Env::new(EnvConfig::default());
    let obs = env.last();
    assert_eq!(obs.box_number, 1);
    assert!(obs.player.hp > 0.0 && obs.player.hp == obs.player.max_hp, "{:?}", obs.player);
    assert_eq!(obs.flat().len(), FLAT);
    assert!(obs.flat().iter().all(|x| x.is_finite()));
    let json = obs_json(obs);
    assert_eq!(json["box"], 1);
    assert!(json["pile_heights"]["rows_front_to_back_left_to_right"].as_array().is_some_and(|r| r.len() == 8));
}

#[test]
fn the_mini_box_has_its_own_rim_and_its_traces_replay_too() {
    let config = EnvConfig { seed: 4, rim: Some(box_env::MINI_RIM), ..EnvConfig::default() };
    let mut env = Env::new(config);
    assert_eq!(env.last().rim, box_env::MINI_RIM);
    let mut act = policy(9);
    for _ in 0..300 {
        if env.step(act()).done {
            break;
        }
    }
    assert_eq!(replay(&env.trace()), env.outcome(), "the rim is part of the trace's config");
    assert_eq!(env.reset(5).rim, box_env::MINI_RIM, "a reset keeps it");
}

#[test]
fn a_trace_replays_to_the_same_outcome_bit_for_bit() {
    // 40 s of play: enemies come through the doors, fight, die and pile up.
    let config = EnvConfig { seed: 7, ..EnvConfig::default() };
    let env = play(config, 600);
    let recorded = env.outcome();
    assert!(recorded.steps > 0);
    assert_eq!(replay(&env.trace()), recorded, "the replay matches");
}

/// Plays the scripted brawler (it survives into the thugs' part of box 1) until the run ends or `steps` pass.
fn brawl(seed: u32, steps: usize) -> Env {
    let mut env = Env::new(EnvConfig { seed, ..EnvConfig::default() });
    for _ in 0..steps {
        let a = brawler(env.last());
        if env.step(a).done {
            break;
        }
    }
    env
}

#[test]
fn different_seeds_make_different_runs() {
    // Box 1 sends only fodder for its first 14 s, so seeds part once thugs can come through the doors.
    let first = brawl(1, 1200).outcome();
    assert!((2..5).any(|seed| brawl(seed, 1200).outcome() != first), "every seed played out identically");
}

#[test]
fn the_scripted_brawler_kills_and_is_paid_for_it() {
    let env = brawl(3, 1200);
    let o = env.outcome();
    assert!(o.kills >= 3, "{o:?}");
    assert!(f32::from_bits(o.reward_bits) > 0.0, "kills outweigh the harm taken: {o:?}");
    assert_eq!(replay(&env.trace()), o, "a closed-loop policy's trace replays too");
}

#[test]
fn standing_still_in_a_filling_box_gets_hurt_and_the_reward_says_so() {
    let mut env = Env::new(EnvConfig::default());
    let idle = Action::new(0, 0);
    let (mut total, mut hurt_steps) = (0.0, 0);
    // A minute of standing there.
    for _ in 0..900 {
        let s = env.step(idle);
        total += s.reward;
        if s.reward < -box_env::STEP - 1e-6 {
            hurt_steps += 1;
        }
        if s.done {
            break;
        }
    }
    assert!(hurt_steps > 0, "the enemies landed blows");
    assert!(total < 0.0, "and it cost: {total}");
    assert_eq!(env.outcome().escapes, 0);
}
