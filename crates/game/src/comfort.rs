//! Comfort and smoothness (D12): reduced motion (softer flashes, fewer sparks; the game has no screen shake), and
//! automatic quality scaling (`game_core::quality`) that sheds bloom, then MSAA, then shadows when frames run long.
//! Both log what they do, so the browser harness can check them.

use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::Msaa;
use game_core::quality::Scaler;

/// Whether the visitor asked for reduced motion.
#[derive(Resource, Clone, Copy, Debug)]
pub struct Comfort {
    pub reduced_motion: bool,
}

impl Comfort {
    /// Share of the usual sparks to spray.
    pub fn sparks(&self) -> f32 {
        if self.reduced_motion { 0.4 } else { 1.0 }
    }
}

#[derive(Resource, Default)]
struct Quality(Scaler);

pub fn plugin(app: &mut App) {
    app.insert_resource(Comfort { reduced_motion: prefers_reduced_motion() })
        .init_resource::<Quality>()
        .add_systems(Startup, |comfort: Res<Comfort>| {
            if comfort.reduced_motion {
                info!("comfort: reduced motion on (softer flashes, fewer sparks)");
            }
        })
        .add_systems(Update, scale_quality);
}

/// The page's `prefers-reduced-motion` (or `?motion=reduce`), or natively `MOTION=reduce`.
fn prefers_reduced_motion() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        let Some(window) = web_sys::window() else { return false };
        let asked = window.location().search().ok().is_some_and(|q| q.contains("motion=reduce"));
        asked || window.match_media("(prefers-reduced-motion: reduce)").ok().flatten().is_some_and(|m| m.matches())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var("MOTION").is_ok_and(|v| v == "reduce")
    }
}

fn scale_quality(
    mut commands: Commands,
    real: Res<Time<Real>>,
    mut quality: ResMut<Quality>,
    mut cameras: Query<(Entity, &mut Msaa), With<Tonemapping>>,
    mut lights: Query<&mut DirectionalLight>,
) {
    let Some(level) = quality.0.observe(real.delta_secs()) else { return };
    info!("quality: {level:?} (frames averaged {:.1} ms)", quality.0.average_ms);
    for (camera, mut msaa) in &mut cameras {
        if !level.bloom() {
            commands.entity(camera).remove::<Bloom>();
        }
        if !level.msaa() {
            *msaa = Msaa::Off;
        }
    }
    if !level.shadows() {
        for mut light in &mut lights {
            light.shadow_maps_enabled = false;
        }
    }
}
