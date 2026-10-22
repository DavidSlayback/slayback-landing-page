//! Escape the box in 3D (spec #58): an open-topped box seen from above at a 3/4 angle, on the engine-free `game-core`,
//! and served at `/play` by `bash scripts/build.sh`. It replaced the 2.5D game in G10.
//!
//! The library holds the gameplay ([`plugin`]) and, separately, its look ([`look::plugin`]) and camera
//! ([`camera::plugin`]); `main.rs` adds the window and renderer. Keeping them apart lets tests build a headless app
//! from the real game systems (see `tests/scenarios.rs`). Units are metres; y is up, and the camera looks toward -z.

pub mod arena;
pub mod bomb;
pub mod camera;
pub mod carry;
pub mod comfort;
pub mod debris;
pub mod demo;
pub mod enemy;
pub mod fighter;
#[cfg(feature = "render")]
pub mod figure;
#[cfg(feature = "render")]
pub mod flat;
pub mod flow;
pub mod headless;
pub mod hint;
#[cfg(feature = "render")]
pub mod hud;
#[cfg(feature = "render")]
pub mod impact;
pub mod input;
#[cfg(feature = "render")]
pub mod look;
pub mod observe;
#[cfg(feature = "render")]
pub mod outline;
#[cfg(feature = "render")]
pub mod paint;
pub mod player;
pub mod pose;
pub mod run;
pub mod score;
#[cfg(feature = "render")]
pub mod screens;
pub mod sound;
pub mod store;
#[cfg(feature = "render")]
pub mod touchpad;

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::fighter::Combat;
use crate::flow::Phase;

/// The game's frame (`Update`), in order. A module puts each system in the set it belongs to, and this is the only
/// place the sets are ordered:
///
/// 1. [`Frame::Decide`]: the player's control and the enemies' brains turn intent into moves and swings; the doors
///    call in the crowd.
/// 2. [`Frame::Act`]: what was decided plays out: the carry (grab, whirl, throw, flight) and the bombs' fuses and
///    blasts, which send their strikes.
/// 3. [`Frame::Resolve`]: swings advance and every strike lands ([`Combat`], first), then what follows from it:
///    debris, the box clock and escape or death, the score.
/// 4. [`Frame::Show`]: whatever shows the settled frame or keeps books across the screens: the look, the camera,
///    impacts, sound, shake, the HUD and screens, the arena's rebuild, the score's tally.
///
/// The first three run only while playing; `Show` always runs. Pausing is one rule: off `Playing` the gameplay sets
/// stop, and the virtual clock (also the hit-stop's freeze, `fighter`) stops physics. Input is read before the frame
/// (`input::ReadActions`, `PreUpdate`), and `flow`'s phase changes sit outside it: they only queue the next phase.
#[derive(SystemSet, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Frame {
    Decide,
    Act,
    Resolve,
    Show,
}

/// Gravity, m/s²: heavier than Earth's, so falls and tumbles read snappily at this camera distance.
pub const GRAVITY: f32 = 15.0;

/// Everything that makes up the game except windowing and rendering: physics, the phases, the box, fighting, the
/// player, grabbing and throwing, enemies and their bomb-heads, their debris, the run, the score and best, the sound
/// setting, and comfort (reduced motion, quality scaling).
pub fn plugin(app: &mut App) {
    // Every rigid body's transform is interpolated between physics steps (64 Hz), so fighters and tumbling debris
    // move smoothly at any frame rate.
    app.add_plugins(PhysicsPlugins::default().set(PhysicsInterpolationPlugin::interpolate_all()))
        .configure_sets(Update, (Frame::Decide, Frame::Act, Frame::Resolve, Frame::Show).chain())
        .configure_sets(Update, (Frame::Decide, Frame::Act, Frame::Resolve).run_if(in_state(Phase::Playing)))
        .configure_sets(Update, Combat.in_set(Frame::Resolve))
        .insert_resource(Gravity(Vec3::NEG_Y * GRAVITY))
        .add_plugins((
            input::plugin,
            comfort::plugin,
            flow::plugin,
            arena::plugin,
            fighter::plugin,
            player::plugin,
            carry::plugin,
            bomb::plugin,
            enemy::plugin,
            debris::plugin,
            run::plugin,
            score::plugin,
            sound::plugin,
            hint::plugin,
        ));
}

/// A plane vector from `game_core` (`[x, y]`, y away from the camera) on the floor, as a world direction.
pub fn on_floor(v: [f32; 2]) -> Vec3 {
    Vec3::new(v[0], 0.0, -v[1])
}

/// A world vector's floor part as a `game_core` plane vector.
pub fn plane(v: Vec3) -> [f32; 2] {
    [v.x, -v.z]
}

/// A `game_core` plane point as a point on the floor (x and z), the inverse of [`plane`] there.
pub fn plane_on(p: [f32; 2]) -> Vec2 {
    Vec2::new(p[0], -p[1])
}
