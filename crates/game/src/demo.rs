//! A scripted demo (`/play/?demo=1`, or `DEMO=1` natively): fodder stepping up to the player, who punches, kicks,
//! drop-kicks and spin-throws them in a loop. It drives the real game through synthetic key presses, so what it shows is what
//! playing looks like. Uses: checking the look of a fight without a person at the keyboard, recording the
//! touch-device gameplay clip, and loading the box with debris (`debris=1000`) to measure frame time.

use bevy::input::InputSystems;
use bevy::prelude::*;

use crate::ARENA;
use crate::arena::WALL;
use crate::debris::part;
use crate::fighter::Hp;
use crate::flow::{PauseOnBlur, Phase};
use crate::run::RunState;
use game_core::combat::Health;
use crate::bomb::BombHead;
use crate::enemy::{Brain, HALF, Waves, enemy};
use game_core::enemy::Kind;
use game_core::debris::humanoid_parts;
use crate::input::{GRAB_KEY, KICK_KEY, PUNCH_KEY, ReadActions};
use crate::player::Player;

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
    // `debris` drops that many loose parts into the box at the start, to check the frame time of a full pile.
    let debris = setting("debris").and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
    // `enemy` picks who steps up each beat (fodder, thug or ninja) and `bombs=1` gives them bomb heads.
    let foe = match setting("enemy").as_deref() {
        Some("thug") => Kind::Thug,
        Some("ninja") => Kind::Ninja,
        _ => Kind::Fodder,
    };
    let bombs = setting("bombs").as_deref() == Some("1");
    // `timer` (seconds on the box clock) and `hp` (the player's starting health) make a run end on cue, e.g. to check
    // the game-over screen and the best score: `?demo=1&timer=6&hp=20`.
    let timer = setting("timer").and_then(|s| s.parse::<f32>().ok());
    let hp = setting("hp").and_then(|s| s.parse::<f32>().ok());
    app.insert_resource(Waves { enabled: false, ..default() })
        .insert_resource(Foe { kind: foe, bombs })
        .add_systems(Startup, move |mut time: ResMut<Time<Virtual>>| time.set_relative_speed(speed))
        // Straight into play: no start overlay.
        .add_systems(PostStartup, move |mut next: ResMut<NextState<Phase>>, mut run: ResMut<RunState>, mut player: Query<&mut Hp, With<Player>>| {
            next.set(Phase::Playing);
            if let Some(t) = timer {
                run.0.time_left = t;
            }
            if let (Some(h), Ok(mut p)) = (hp, player.single_mut()) {
                p.0.hp = h;
            }
        })
        .add_systems(Startup, move |mut commands: Commands| drop_debris(&mut commands, debris))
        .insert_resource(PauseOnBlur(false))
        // Only while playing: after a death the script must not press "play again" (Space is also the dash).
        .add_systems(PreUpdate, script.after(InputSystems).before(ReadActions).run_if(in_state(Phase::Playing)));
}

/// `n` parts of a broken figure's shapes, in rows across the top of the box.
fn drop_debris(commands: &mut Commands, n: usize) {
    let shapes = humanoid_parts((HALF.x, HALF.y));
    let cols = 40;
    for i in 0..n {
        let (col, row) = ((i % cols) as f32, (i / cols) as f32);
        let x = -ARENA.x / 2.0 + WALL + 20.0 + col * (ARENA.x - 2.0 * WALL - 40.0) / (cols - 1) as f32;
        let y = ARENA.y / 2.0 - WALL - 30.0 - row * 26.0 + (i % 3) as f32 * 4.0;
        commands.spawn(part(shapes[i % shapes.len()].shape, Vec2::new(x, y.max(-ARENA.y / 2.0 + 120.0)), 0.0));
    }
}

const DEMO_FODDER_HP: f32 = 8.0;

/// Who steps up each beat.
#[derive(Resource)]
struct Foe {
    kind: Kind,
    bombs: bool,
}

/// One beat of the loop: what to press, and how far away the next fodder stands.
const BEATS: [(&[KeyCode], f32); 4] =
    [(&[PUNCH_KEY], 48.0), (&[KICK_KEY], 58.0), (&[KeyCode::Space, PUNCH_KEY], 150.0), (&[GRAB_KEY, PUNCH_KEY], 45.0)];
const BEAT: f32 = 1.4;

fn script(
    time: Res<Time>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut commands: Commands,
    player: Query<(&Transform, &avian2d::prelude::ShapeHits), With<Player>>,
    fodder_left: Query<Entity, With<Brain>>,
    foe: Res<Foe>,
    mut clock: Local<(f32, usize, bool)>,
) {
    let (t, beat, spawned) = &mut *clock;
    keys.release_all();
    let Ok((at, ground)) = player.single() else { return };
    // Beats start once the player has landed, so the first fodder isn't placed in mid-air.
    if *t == 0.0 && *beat == 0 && ground.is_empty() {
        return;
    }
    *t += time.delta_secs();
    let (presses, gap) = BEATS[*beat % BEATS.len()];
    // Set up the beat: clear whoever is left and place a fresh fodder at the beat's distance in front of the player
    // (it faces right), so every beat plays out the same way.
    if !*spawned && *t > 0.2 {
        for e in &fodder_left {
            commands.entity(e).despawn();
        }
        // Repeated dashes walk the player right; keep the fodder inside the box.
        let x = (at.translation.x + gap).min(ARENA.x / 2.0 - WALL - HALF.x - 4.0);
        // Weak fodder, so every beat ends with a kill (and the box fills with debris).
        let mut e = commands.spawn(enemy(foe.kind, Vec2::new(x, at.translation.y + 2.0)));
        e.insert(Hp(Health::new(DEMO_FODDER_HP)));
        if foe.bombs {
            e.insert(BombHead);
        }
        *spawned = true;
    }
    // Strike just before the fodder's own punch would come out, then move on.
    if *t > 0.5 && *t - time.delta_secs() <= 0.5 {
        keys.press(presses[0]); // a dash or grab first; the attack follows (drop-kick / spin-throw)
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
