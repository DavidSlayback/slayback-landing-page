//! Sound effects (Q79), crunchy and real: blows thump, kills crack and fly apart, debris knocks and clatters as it
//! lands (louder the harder, higher the smaller), throws whoosh, doors knock twice before they open, and bombs boom
//! over a rain of rubble. The hits are recordings David picked by ear from CC0 packs (`tools/sfx/import.sh`), the
//! whoosh and the boom synthesised (`tools/sfx/generate.py`), all embedded from `crates/game3d/assets/sfx/`. Muted by default; M (or Select, or a tap
//! low on the screen) on the pause screen toggles it, and the choice is kept under the 2.5D game's key
//! (`slayback-sound`, see [`Store`]).
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
        .add_systems(Update, toggle.in_set(crate::Frame::Show).run_if(in_state(Phase::Paused)));
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

    use super::Sound;
    use crate::bomb::Exploded;
    use crate::carry::Thrown;
    use crate::debris::{CLACK, Clack};
    use crate::enemy::WallDoor;
    use crate::fighter::{Defeated, Hit, Team};

    /// The effects (Q79): recordings David picked by ear from CC0 packs (`tools/sfx/import.sh`; credits in
    /// `assets/sfx/README.md`), in a few takes each so repeats don't sound mechanical, and a whoosh and a blast
    /// synthesised (`tools/sfx/generate.py`).
    #[derive(Resource)]
    struct Effects {
        hit: [Handle<AudioSource>; 3],
        heavy: [Handle<AudioSource>; 3],
        clack: [Handle<AudioSource>; 4],
        crack: [Handle<AudioSource>; 2],
        rubble: Handle<AudioSource>,
        whoosh: Handle<AudioSource>,
        door: Handle<AudioSource>,
        boom: Handle<AudioSource>,
    }

    /// Plays the effects (the window's audio output; not in headless runs).
    pub fn playback(app: &mut App) {
        app.add_systems(Startup, load).add_systems(Update, play.in_set(crate::Frame::Show));
    }

    fn load(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
        let mut add = |bytes: &'static [u8]| sources.add(AudioSource { bytes: Arc::from(bytes) });
        macro_rules! sfx {
            ($name:literal) => {
                add(include_bytes!(concat!("../assets/sfx/", $name, ".wav")))
            };
        }
        commands.insert_resource(Effects {
            hit: [sfx!("hit_0"), sfx!("hit_1"), sfx!("hit_2")],
            heavy: [sfx!("heavy_0"), sfx!("heavy_1"), sfx!("heavy_2")],
            clack: [sfx!("clack_0"), sfx!("clack_1"), sfx!("clack_2"), sfx!("clack_3")],
            crack: [sfx!("crack_0"), sfx!("crack_1")],
            rubble: sfx!("rubble"),
            whoosh: sfx!("whoosh"),
            door: sfx!("door"),
            boom: sfx!("boom"),
        });
    }

    /// At most this many debris knocks start in a frame, the hardest first, and none sooner than [`CLACK_GAP`] after the
    /// last: a heap collapsing rattles rather than machine-guns.
    const CLACKS: usize = 2;
    const CLACK_GAP: f32 = 0.03;

    /// A sound to start: which, how loud (0..=1) and how fast (pitch: 1 as recorded).
    type Cue<'a> = (&'a Handle<AudioSource>, f32, f32);

    #[allow(clippy::too_many_arguments)]
    fn play(
        mut commands: Commands,
        sound: Res<Sound>,
        effects: Res<Effects>,
        real: Res<Time<Real>>,
        mut hits: MessageReader<Hit>,
        mut defeats: MessageReader<Defeated>,
        mut blasts: MessageReader<Exploded>,
        mut clacks: MessageReader<Clack>,
        thrown: Query<(), Added<Thrown>>,
        doors: Query<(Entity, &WallDoor)>,
        mut warned: Local<Vec<Entity>>,
        mut last_clack: Local<f32>,
        mut seed: Local<u32>,
    ) {
        // A little randomness for which take plays and its pitch (xorshift: no dependency for this).
        let mut roll = || {
            let mut x = seed.wrapping_add(0x9E37_79B9) | 1;
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            *seed = x;
            (x % 10_000) as f32 / 10_000.0
        };
        let mut queue: Vec<Cue> = Vec::new();
        // A blow on a body of blocks: a wooden knock, heavier for kicks, throws and smashes. One per frame: a crowd
        // struck at once shouldn't stack ten.
        if let Some(h) = hits.read().last() {
            let (takes, volume) = if h.feel.heavy { (&effects.heavy, 0.85) } else { (&effects.hit, 0.7) };
            let take = &takes[(roll() * takes.len() as f32) as usize % takes.len()];
            queue.push((take, if h.finisher { 1.0 } else { volume }, 0.94 + 0.12 * roll()));
        }
        // A kill: the figure cracks apart and its blocks fly (their landings knock below).
        if defeats.read().any(|d| d.team == Team::Enemy) {
            let take = &effects.crack[(roll() * 2.0) as usize % 2];
            queue.push((take, 0.75, 0.95 + 0.1 * roll()));
            queue.push((&effects.whoosh, 0.45, 0.9 + 0.2 * roll()));
        }
        // Something thrown flies off.
        if !thrown.is_empty() {
            queue.push((&effects.whoosh, 0.5, 1.0 + 0.15 * roll()));
        }
        // Debris landing: the hardest knocks this frame, louder the harder, higher the smaller the block.
        let now = real.elapsed_secs();
        let mut landed: Vec<Clack> = clacks.read().copied().collect();
        if !landed.is_empty() && now - *last_clack >= CLACK_GAP {
            *last_clack = now;
            landed.sort_by(|a, b| b.strength.total_cmp(&a.strength));
            for c in landed.into_iter().take(CLACKS) {
                let take = &effects.clack[(roll() * effects.clack.len() as f32) as usize % effects.clack.len()];
                let volume = ((c.strength - CLACK) / 7.0).clamp(0.12, 1.0) * 0.6;
                let pitch = (0.32 / c.size.max(0.05)).clamp(0.75, 1.35) * (0.94 + 0.12 * roll());
                queue.push((take, volume, pitch));
            }
        }
        // A door starting its warning knocks: the sound tells you where the next enemy comes from.
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
            queue.push((&effects.door, 0.55, 1.0));
        }
        // A bomb going off (once a frame, however many chain).
        if blasts.read().last().is_some() {
            queue.push((&effects.boom, 0.95, 1.0));
            queue.push((&effects.rubble, 0.6, 0.95 + 0.1 * roll()));
        }
        if !sound.on {
            return;
        }
        for (source, volume, speed) in queue {
            let settings = PlaybackSettings { speed, ..PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume)) };
            commands.spawn((AudioPlayer(source.clone()), settings));
        }
    }
}
