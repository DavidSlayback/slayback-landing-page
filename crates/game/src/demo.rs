//! A scripted demo (`/play/?demo=1`, or `DEMO=1` natively): fodder stepping up to the player, who punches, kicks and
//! drop-kicks them in a loop. It drives the real game through synthetic key presses, so what it shows is what
//! playing looks like. Uses: checking the look of a fight without a person at the keyboard, and recording the
//! touch-device gameplay clip.

use bevy::input::InputSystems;
use bevy::prelude::*;

use crate::ARENA;
use crate::arena::WALL;
use crate::enemy::{Brain, HALF, Waves, fodder};
use crate::player::{KICK_KEY, PUNCH_KEY, Player};

/// A demo setting from the page query (`demo=1&slow=0.25`) or, natively, the environment (`DEMO=1 SLOW=0.25`).
fn setting(key: &str) -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        let query = web_sys::window()?.location().search().ok()?;
        query.trim_start_matches('?').split('&').find_map(|kv| kv.strip_prefix(&format!("{key}="))).map(str::to_owned)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var(key.to_uppercase()).ok()
    }
}

/// Whether the page or environment asked for the demo.
pub fn requested() -> bool {
    setting("demo").as_deref() == Some("1")
}

pub fn plugin(app: &mut App) {
    // `slow` plays the demo in slow motion (e.g. 0.25), so short effects can be captured frame by frame.
    let speed = setting("slow").and_then(|s| s.parse::<f32>().ok()).filter(|s| *s > 0.0).unwrap_or(1.0);
    app.insert_resource(Waves { enabled: false, ..default() })
        .add_systems(Startup, move |mut time: ResMut<Time<Virtual>>| time.set_relative_speed(speed))
        .add_systems(PreUpdate, script.after(InputSystems));
}

/// One beat of the loop: what to press, and how far away the next fodder stands.
const BEATS: [(&[KeyCode], f32); 3] = [(&[PUNCH_KEY], 48.0), (&[KICK_KEY], 58.0), (&[KeyCode::Space, PUNCH_KEY], 150.0)];
const BEAT: f32 = 1.4;

fn script(
    time: Res<Time>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut commands: Commands,
    player: Query<&Transform, With<Player>>,
    fodder_left: Query<(), With<Brain>>,
    mut clock: Local<(f32, usize, bool)>,
) {
    let (t, beat, spawned) = &mut *clock;
    keys.release_all();
    *t += time.delta_secs();
    let Ok(at) = player.single() else { return };
    let (presses, gap) = BEATS[*beat % BEATS.len()];
    // Set up the beat: a fresh fodder at the right distance in front of the player (it faces right).
    if !*spawned && *t > 0.2 {
        if fodder_left.is_empty() {
            // Repeated dashes walk the player right; keep the fodder inside the box.
            let x = (at.translation.x + gap).min(ARENA.x / 2.0 - WALL - HALF.x - 4.0);
            commands.spawn(fodder(Vec2::new(x, at.translation.y + 2.0)));
        }
        *spawned = true;
    }
    // Strike just before the fodder's own punch would come out, then move on.
    if *t > 0.5 && *t - time.delta_secs() <= 0.5 {
        keys.press(presses[0]); // for a drop-kick, the dash; the attack follows mid-dash
    }
    if presses.len() == 2 && *t > 0.55 && *t - time.delta_secs() <= 0.55 {
        keys.press(presses[1]);
    }
    if *t > BEAT {
        *t = 0.0;
        *beat += 1;
        *spawned = false;
    }
}
