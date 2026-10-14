//! Demo mode (`/play/?demo=1`, or `DEMO=1` natively), driving the real game with synthetic key presses. It goes
//! straight into play (no start overlay) and doesn't pause when the page loses focus, as in the 2.5D game. So far it
//! has two knobs. `stairs=1` (`STAIRS=1`), the one the browser harness uses, is the stairs run: each box gets a
//! staircase of debris up past its rim and the player walks right, up it and out, box after box. `bombs=1`
//! (`BOMBS=1`) makes every enemy that comes through a door a bomb-head. The scripted fight and the other knobs (slow
//! motion, debris, enemy type) are G9's.

use bevy::input::InputSystems;
use bevy::prelude::*;
use game_core::climb::MAX_STEP;
use game_core::debris::solid::Solid;
use game_core::run::rim_metres;

use crate::debris::piece;
use crate::enemy::Waves;
use crate::flow::{PauseOnBlur, Phase};
use crate::input::ReadActions;
use crate::player::SPAWN;
use crate::run::BoxStarted;

/// A demo setting from the page query (`demo=1&stairs=1`) or, natively, the environment (`DEMO=1 STAIRS=1`).
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

/// The demo, if the page or environment asked for it; otherwise nothing.
pub fn plugin(app: &mut App) {
    if !requested() {
        return;
    }
    // After the screens' Startup, which puts a fresh page on the start overlay.
    app.insert_resource(PauseOnBlur(false))
        .add_systems(PostStartup, |mut next: ResMut<NextState<Phase>>| next.set(Phase::Playing));
    if setting("stairs").as_deref() == Some("1") {
        app.add_plugins(stairs);
    }
    if setting("bombs").as_deref() == Some("1") {
        app.add_systems(Startup, |mut waves: ResMut<Waves>| waves.bombs = Some(1.0));
    }
}

/// The stairs run: no enemies, a staircase in each box, and the player walking right.
pub fn stairs(app: &mut App) {
    app.insert_resource(Waves { enabled: false, ..default() })
        .add_systems(Update, build_stairs)
        .add_systems(PreUpdate, walk_right.after(InputSystems).before(ReadActions));
}

/// Each step rises this much (under `MAX_STEP`), metres.
const RISE: f32 = 0.75 * MAX_STEP;
/// Each step's depth along x and width along z, metres.
const TREAD: f32 = 0.55;
const WIDTH: f32 = 1.2;
/// Where the first step starts, right of the spawn, metres.
const FIRST: f32 = 0.8;

/// A staircase of frozen debris to the right of the spawn, up to the rim: blocks from the floor, each a step taller
/// than the last, the top one level with or above the rim.
fn build_stairs(mut commands: Commands, time: Res<Time>, mut started: MessageReader<BoxStarted>) {
    let Some(BoxStarted(spec)) = started.read().last() else { return };
    let steps = (rim_metres(spec) / RISE).ceil() as usize;
    for i in 0..steps {
        let h = RISE * (i + 1) as f32;
        let at = Vec3::new(FIRST + TREAD * (i as f32 + 0.5), h / 2.0, SPAWN.z);
        let block = piece(Solid::Cube([TREAD, h, WIDTH]), 0, at, Quat::IDENTITY, time.elapsed_secs());
        commands.spawn(block).insert(avian3d::prelude::RigidBody::Static);
    }
}

fn walk_right(mut keys: ResMut<ButtonInput<KeyCode>>) {
    keys.release_all();
    keys.press(KeyCode::KeyD);
}
