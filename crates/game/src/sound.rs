//! Sound effects (D10): generated retro sounds (`tools/sfx/generate.py`, embedded here) for hits, shatters and
//! blasts. Muted by default; M (or Select) on the pause screen toggles them, and the choice is kept (localStorage
//! `slayback-sound`, or natively a file in the temp directory).

use std::sync::Arc;

use bevy::audio::Volume;
use bevy::prelude::*;
use game_core::combat::Move;

use crate::bomb::Exploded;
use crate::debris::Shattered;
use crate::fighter::Hit;
use crate::flow::Phase;
use crate::input::Actions;

/// Whether sound is on.
#[derive(Resource)]
pub struct Sound {
    pub on: bool,
}

#[derive(Resource)]
struct Effects {
    hit: Handle<AudioSource>,
    heavy: Handle<AudioSource>,
    shatter: Handle<AudioSource>,
    boom: Handle<AudioSource>,
}

pub fn plugin(app: &mut App) {
    app.insert_resource(Sound { on: load() })
        .add_systems(Startup, load_effects)
        .add_systems(Update, toggle.run_if(in_state(Phase::Paused)))
        .add_systems(Update, play);
}

fn load_effects(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    let mut add = |bytes: &'static [u8]| sources.add(AudioSource { bytes: Arc::from(bytes) });
    commands.insert_resource(Effects {
        hit: add(include_bytes!("../assets/sfx/hit.wav")),
        heavy: add(include_bytes!("../assets/sfx/heavy.wav")),
        shatter: add(include_bytes!("../assets/sfx/shatter.wav")),
        boom: add(include_bytes!("../assets/sfx/boom.wav")),
    });
}

fn toggle(actions: Res<Actions>, mut sound: ResMut<Sound>) {
    if actions.sound {
        sound.on = !sound.on;
        save(sound.on);
    }
}

fn play(
    mut commands: Commands,
    sound: Res<Sound>,
    effects: Res<Effects>,
    mut hits: MessageReader<Hit>,
    mut shattered: MessageReader<Shattered>,
    mut blasts: MessageReader<Exploded>,
) {
    let mut queue: Vec<(&Handle<AudioSource>, f32)> = Vec::new();
    // At most one of each per frame: a throw through a crowd shouldn't stack ten identical clicks.
    if let Some(h) = hits.read().last() {
        queue.push(if matches!(h.mv, Move::Punch | Move::Slash) { (&effects.hit, 0.5) } else { (&effects.heavy, 0.6) });
    }
    if shattered.read().last().is_some() {
        queue.push((&effects.shatter, 0.55));
    }
    if blasts.read().last().is_some() {
        queue.push((&effects.boom, 0.8));
    }
    if !sound.on {
        return;
    }
    for (source, volume) in queue {
        commands.spawn((AudioPlayer(source.clone()), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume))));
    }
}

const KEY: &str = "slayback-sound";

fn load() -> bool {
    #[cfg(target_arch = "wasm32")]
    let stored =
        web_sys::window().and_then(|w| w.local_storage().ok().flatten()).and_then(|s| s.get_item(KEY).ok().flatten());
    #[cfg(not(target_arch = "wasm32"))]
    let stored = std::fs::read_to_string(std::env::temp_dir().join(KEY)).ok();
    stored.is_some_and(|s| s.trim() == "on")
}

fn save(on: bool) {
    let value = if on { "on" } else { "off" };
    // A lost preference only means sound starts muted again, so failures are ignored.
    #[cfg(target_arch = "wasm32")]
    if let Some(s) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = s.set_item(KEY, value);
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = std::fs::write(std::env::temp_dir().join(KEY), value);
}
