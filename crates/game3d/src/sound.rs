//! Sound effects, as in the 2.5D game: its generated retro set (`tools/sfx/generate.py`, embedded from
//! `crates/game3d/assets/sfx/`) for hits, heavy hits, kills, doors and bomb blasts. Muted by default; M (or Select) on the
//! pause screen toggles it, and the choice is kept under the 2.5D game's key (`slayback-sound`, see [`Store`]).
//!
//! The setting and its toggle are gameplay (headless scenarios test them); playing the effects needs the renderer's
//! audio, so [`playback`] is added by the window only.

use bevy::prelude::*;

use crate::flow::Phase;
use crate::input::Actions;
use crate::store::Store;

/// Whether sound is on.
#[derive(Resource, Default)]
pub struct Sound {
    pub on: bool,
}

/// The sound setting's key in the [`Store`], shared with the 2.5D game.
pub const SOUND_KEY: &str = "slayback-sound";

pub fn plugin(app: &mut App) {
    app.init_resource::<Sound>()
        .init_resource::<Store>()
        .add_systems(Startup, |store: Res<Store>, mut sound: ResMut<Sound>| {
            sound.on = store.get(SOUND_KEY).is_some_and(|s| s.trim() == "on");
        })
        .add_systems(Update, toggle.run_if(in_state(Phase::Paused)));
}

fn toggle(actions: Res<Actions>, store: Res<Store>, mut sound: ResMut<Sound>) {
    if actions.verbs.sound {
        sound.on = !sound.on;
        store.set(SOUND_KEY, if sound.on { "on" } else { "off" });
    }
}

#[cfg(feature = "render")]
pub use effects::playback;

#[cfg(feature = "render")]
mod effects {
    use std::sync::Arc;

    use bevy::audio::Volume;
    use bevy::prelude::*;
    use game_core::combat::Move;

    use super::Sound;
    use crate::bomb::Exploded;
    use crate::enemy::WallDoor;
    use crate::fighter::{Defeated, Hit, Team};

    #[derive(Resource)]
    struct Effects {
        hit: Handle<AudioSource>,
        heavy: Handle<AudioSource>,
        shatter: Handle<AudioSource>,
        door: Handle<AudioSource>,
        boom: Handle<AudioSource>,
    }

    /// Plays the effects (the window's audio output; not in headless runs).
    pub fn playback(app: &mut App) {
        app.add_systems(Startup, load).add_systems(Update, play.after(crate::fighter::Combat));
    }

    fn load(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
        let mut add = |bytes: &'static [u8]| sources.add(AudioSource { bytes: Arc::from(bytes) });
        commands.insert_resource(Effects {
            hit: add(include_bytes!("../assets/sfx/hit.wav")),
            heavy: add(include_bytes!("../assets/sfx/heavy.wav")),
            shatter: add(include_bytes!("../assets/sfx/shatter.wav")),
            door: add(include_bytes!("../assets/sfx/door.wav")),
            boom: add(include_bytes!("../assets/sfx/boom.wav")),
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn play(
        mut commands: Commands,
        sound: Res<Sound>,
        effects: Res<Effects>,
        mut hits: MessageReader<Hit>,
        mut defeats: MessageReader<Defeated>,
        mut blasts: MessageReader<Exploded>,
        doors: Query<(Entity, &WallDoor)>,
        mut warned: Local<Vec<Entity>>,
    ) {
        let mut queue: Vec<(&Handle<AudioSource>, f32)> = Vec::new();
        // At most one of each per frame: a crowd going down at once shouldn't stack ten identical clicks.
        if let Some(h) = hits.read().last() {
            let light = matches!(h.mv, Move::Punch | Move::Slash);
            queue.push(if light { (&effects.hit, 0.5) } else { (&effects.heavy, 0.6) });
        }
        // A kill: the figure breaks into its pieces.
        if defeats.read().any(|d| d.team == Team::Enemy) {
            queue.push((&effects.shatter, 0.55));
        }
        // A door starting its warning: the sound tells you where the next enemy comes from.
        let mut door = false;
        for (e, d) in &doors {
            match (d.door.warning(), warned.contains(&e)) {
                (true, false) => {
                    warned.push(e);
                    door = true;
                }
                (false, true) => warned.retain(|&w| w != e),
                _ => {}
            }
        }
        if door {
            queue.push((&effects.door, 0.35));
        }
        // A bomb going off (once a frame, however many chain).
        if blasts.read().last().is_some() {
            queue.push((&effects.boom, 0.8));
        }
        if !sound.on {
            return;
        }
        for (source, volume) in queue {
            commands
                .spawn((AudioPlayer(source.clone()), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume))));
        }
    }
}
