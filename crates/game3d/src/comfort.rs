//! Comfort and smoothness, as in the 2.5D game (decision Q35): reduced motion, and automatic quality scaling.
//!
//! - **Reduced motion** (the page's `prefers-reduced-motion` or `?motion=reduce`; natively `MOTION=reduce`): no screen
//!   shake and no squash, softer hit flashes (`figure`) and fewer star bursts (`impact`). Shake and squash start here,
//!   from each blow ([`Shake`], [`Squash`]), and `impact` draws them, so the headless scenarios see them too.
//! - **Quality scaling** (`game_core::quality`): when frames run long for a while, the game sheds its costliest effects
//!   first ([`Shed`]): the look's outline pass, then half the star bursts. It never steps back up. (The outline pass
//!   already turns MSAA off, so there is no MSAA step as in the 2.5D game.)
//!
//! Both log what they do, so the browser harness can check them.

use bevy::prelude::*;
use game_core::impact::{self, BLAST_TRAUMA, SQUASH_TIME, trauma};
use game_core::quality::{Level, Scaler};

use crate::bomb::Exploded;
use crate::fighter::{Combat, Hit};

/// Whether the visitor asked for reduced motion. Tests insert their own before the first update.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Comfort {
    pub reduced_motion: bool,
}

impl Comfort {
    /// The page's setting (or, natively, the environment's).
    pub fn page() -> Self {
        Self { reduced_motion: prefers_reduced_motion() }
    }

    /// Share of the usual star bursts to show: 40%, as the 2.5D game's sparks.
    pub fn bursts(&self) -> f32 {
        if self.reduced_motion { 0.4 } else { 1.0 }
    }
}

/// Screen shake (`game_core::impact::Shake`): blows and blasts add trauma here, and `impact` drains it over real time and moves
/// the camera by it. Reduced motion adds none.
#[derive(Resource, Default, Debug, Deref, DerefMut)]
pub struct Shake(pub impact::Shake);

/// A struck fighter's squash, seconds remaining: started here by a blow, drawn and eased back by `impact`. Reduced
/// motion starts none.
#[derive(Component)]
pub struct Squash(pub f32);

/// An effect quality scaling can turn off, in the order it goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shed {
    /// The look's edge-detection outline pass, the costliest full-screen effect.
    Outlines,
    /// Half the star bursts.
    Bursts,
}

const SHED: [Shed; 2] = [Shed::Outlines, Shed::Bursts];

/// The frame-time watcher (`game_core::quality::Scaler`) and what it has shed so far.
#[derive(Resource, Default, Debug)]
pub struct Quality(pub Scaler);

impl Quality {
    /// What has been turned off, in order.
    pub fn shed(&self) -> &'static [Shed] {
        // The shared scaler names the 2.5D game's three effects; here each step down is the next in `SHED`, and the
        // last step has nothing left to shed.
        let steps = match self.0.level {
            Level::Full => 0,
            Level::NoBloom => 1,
            Level::NoMsaa | Level::Minimal => 2,
        };
        &SHED[..steps]
    }

    pub fn keeps(&self, effect: Shed) -> bool {
        !self.shed().contains(&effect)
    }
}

/// Share of the usual star bursts to show, after reduced motion and quality scaling.
pub fn burst_share(comfort: &Comfort, quality: &Quality) -> f32 {
    comfort.bursts() * if quality.keeps(Shed::Bursts) { 1.0 } else { 0.5 }
}

/// Keeps a steady share of a stream of effects (two in five at 0.4), evenly rather than at random; the first is kept.
#[derive(Debug)]
pub struct Thin(f32);

impl Default for Thin {
    fn default() -> Self {
        Self(1.0)
    }
}

impl Thin {
    pub fn keep(&mut self, share: f32) -> bool {
        let keep = self.0 >= 1.0 - 1e-4;
        if keep {
            self.0 -= 1.0;
        }
        self.0 += share;
        keep
    }
}

pub fn plugin(app: &mut App) {
    if !app.world().contains_resource::<Comfort>() {
        app.insert_resource(Comfort::page());
    }
    app.init_resource::<Shake>()
        .init_resource::<Quality>()
        .add_systems(Startup, announce)
        .add_systems(Update, ((add_trauma, start_squash).after(Combat), scale_quality));
    #[cfg(feature = "render")]
    app.add_systems(Update, view::apply_quality.after(scale_quality));
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

/// Logged for the browser harness, which waits for "comfort: reduced motion on".
fn announce(comfort: Res<Comfort>) {
    if comfort.reduced_motion {
        info!("comfort: reduced motion on (no shake or squash, softer flashes, fewer bursts)");
    }
}

fn add_trauma(
    comfort: Res<Comfort>,
    mut shake: ResMut<Shake>,
    mut hits: MessageReader<Hit>,
    mut blasts: MessageReader<Exploded>,
) {
    let amount =
        hits.read().map(|h| trauma(h.mv, h.finisher)).sum::<f32>() + blasts.read().count() as f32 * BLAST_TRAUMA;
    if amount > 0.0 && !comfort.reduced_motion {
        shake.add(amount);
    }
}

fn start_squash(mut commands: Commands, comfort: Res<Comfort>, mut hits: MessageReader<Hit>) {
    for hit in hits.read() {
        // The victim may already be gone (a kill bursts it this frame).
        if !comfort.reduced_motion {
            commands.entity(hit.victim).try_insert(Squash(SQUASH_TIME));
        }
    }
}

/// Feeds the scaler each frame's real duration; a step down that sheds something is logged with what it turned off.
fn scale_quality(real: Res<Time<Real>>, mut quality: ResMut<Quality>) {
    let before = quality.shed().len();
    if quality.0.observe(real.delta_secs()).is_none() || quality.shed().len() == before {
        return;
    }
    let Some(&off) = quality.shed().last() else { return };
    let what = match off {
        Shed::Outlines => "outlines off",
        Shed::Bursts => "half the star bursts",
    };
    info!(
        "quality: {what} (step {} of {}; frames averaged {:.1} ms)",
        quality.shed().len(),
        SHED.len(),
        quality.0.average_ms
    );
}

#[cfg(feature = "render")]
mod view {
    use bevy::prelude::*;

    use super::{Quality, Shed};
    use crate::outline::EdgeOutline;

    /// Takes the outline pass off every camera once quality has shed it. (Bursts are thinned where `impact` spawns
    /// them.)
    pub fn apply_quality(
        mut commands: Commands,
        quality: Res<Quality>,
        cameras: Query<Entity, (With<Camera3d>, With<EdgeOutline>)>,
    ) {
        if quality.keeps(Shed::Outlines) {
            return;
        }
        for e in &cameras {
            commands.entity(e).remove::<EdgeOutline>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thinning_keeps_the_first_and_then_an_even_share() {
        let mut thin = Thin::default();
        let kept: Vec<bool> = (0..10).map(|_| thin.keep(0.4)).collect();
        assert!(kept[0], "the first burst shows");
        assert_eq!(kept.iter().filter(|k| **k).count(), 4, "four in ten: {kept:?}");
        let mut all = Thin::default();
        assert!((0..20).all(|_| all.keep(1.0)), "a full share keeps every one");
        let mut half = Thin::default();
        assert_eq!((0..10).filter(|_| half.keep(0.5)).count(), 5);
    }

    #[test]
    fn quality_sheds_outlines_then_bursts() {
        let mut q = Quality::default();
        assert!(q.shed().is_empty() && q.keeps(Shed::Outlines));
        let steps: Vec<Vec<Shed>> =
            (0..600).filter_map(|_| q.0.observe(1.0 / 30.0).map(|_| q.shed().to_vec())).collect();
        assert_eq!(steps, vec![vec![Shed::Outlines], SHED.to_vec(), SHED.to_vec()], "the last step sheds nothing more");
        let reduced = Comfort { reduced_motion: true };
        assert!((burst_share(&reduced, &q) - 0.2).abs() < 1e-6, "both cut the bursts");
        assert_eq!(burst_share(&Comfort::default(), &Quality::default()), 1.0);
    }
}
