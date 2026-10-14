//! The camera: frames the fight, not the world. Box 1 is a long, low strip (1280 wide, rim 185 high) in a world
//! 720 high, so a fixed view of everything leaves the fighters ~50 px tall under a band of empty black. Instead the
//! view fits the player and the enemies near enough to matter, never tighter than the floor-to-rim span plus
//! headroom, never wider than the box, and eases between framings.

use bevy::prelude::*;
use bevy::transform::TransformSystems;
use game_core::impact::{self, BLAST_TRAUMA, trauma};

use crate::ARENA;
use crate::bomb::Exploded;
use crate::comfort::Comfort;
use crate::enemy::Brain;
use crate::fighter::{Fighter, Hit};
use crate::player::Player;
use crate::run::{RunState, floor_top};

/// Space kept above the rim for a fighter standing on top of the pile, world units.
const HEADROOM: f32 = 120.0;
/// Space kept around each framed fighter.
const MARGIN: f32 = 110.0;
/// Space below the floor surface, for the slab's front edge (it sits nearer the camera, so it projects lower).
const BELOW: f32 = 45.0;
/// The tightest view (world units of height): about four fighter heights.
const MIN_HEIGHT: f32 = 300.0;
/// Enemies further than this from the player are left out of the frame until they close in.
const ENGAGE: f32 = 520.0;
/// How far above its target the camera sits, per unit of distance: a slight downward tilt shows the floor's depth.
const TILT: f32 = 0.12;
/// Easing rate (1/s) toward the framing.
const EASE: f32 = 3.5;
/// Vertical field of view.
pub const FOV: f32 = 30.0;

/// The part of the play plane (z = 0) on screen: its centre and height, world units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub centre: Vec2,
    pub height: f32,
}

/// The view that frames `points` (fighter centres) in a box whose rim stands `rim` above the floor.
///
/// # Arguments
/// * `aspect` - viewport width over height.
pub fn frame(points: &[Vec2], rim: f32, aspect: f32) -> View {
    let floor = floor_top();
    let (lo, mut hi) = (floor - BELOW, floor + rim + HEADROOM);
    let (mut left, mut right) = (f32::MAX, f32::MIN);
    for p in points {
        hi = hi.max(p.y + MARGIN);
        left = left.min(p.x - MARGIN);
        right = right.max(p.x + MARGIN);
    }
    if points.is_empty() {
        (left, right) = (-ARENA.x / 2.0, ARENA.x / 2.0);
    }
    let widest = (ARENA.x / aspect).max(hi - lo);
    let height = (hi - lo).max((right - left) / aspect).clamp(MIN_HEIGHT, widest);
    // Horizontally: centred on the action, but never showing past the box's outer walls.
    let slack = ((ARENA.x - height * aspect) / 2.0).max(0.0);
    let x = ((left + right) / 2.0).clamp(-slack, slack);
    // Vertically: the floor stays at the bottom of the screen; any extra height goes above.
    View { centre: Vec2::new(x, lo + height / 2.0), height }
}

/// Camera distance from the play plane that shows `height` world units vertically.
fn distance(height: f32) -> f32 {
    height / 2.0 / (FOV.to_radians() / 2.0).tan()
}

/// Where the camera sits and looks to show `view`.
pub fn placement(view: View) -> Transform {
    let d = distance(view.height);
    Transform::from_xyz(view.centre.x, view.centre.y + d * TILT, d).looking_at(view.centre.extend(0.0), Vec3::Y)
}

/// Screen shake (`game_core::impact::Shake`): trauma from impacts, drained over real time, so it keeps going through
/// hit-stop: a frozen frame that shakes is what sells the weight.
#[derive(Resource, Default, Debug, Deref, DerefMut)]
pub struct Shake(pub impact::Shake);

/// Shake at full trauma, world units.
const MAX_SHAKE: f32 = 14.0;
/// How much tighter the view gets at full trauma: a slight punch-in on big impacts.
const PUNCH_IN: f32 = 0.05;

impl Shake {
    /// The view's offset at time `t` (seconds), world units.
    pub fn offset(&self, t: f32) -> Vec2 {
        Vec2::from(self.0.offset(t)) * MAX_SHAKE
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Shake>()
        .add_systems(Update, add_trauma)
        .add_systems(PostUpdate, follow.before(TransformSystems::Propagate));
}

/// Reduced motion turns the shake off entirely (decision Q35).
fn add_trauma(
    comfort: Res<Comfort>,
    mut shake: ResMut<Shake>,
    mut hits: MessageReader<Hit>,
    mut blasts: MessageReader<Exploded>,
) {
    let amount =
        hits.read().map(|h| trauma(h.mv, h.finisher)).sum::<f32>() + blasts.read().count() as f32 * BLAST_TRAUMA;
    if !comfort.reduced_motion {
        shake.add(amount);
    }
}

/// The view the camera is easing through (not yet where it's heading).
#[derive(Component)]
pub struct Eased(pub View);

#[allow(clippy::type_complexity)]
fn follow(
    time: Res<Time>,
    real: Res<Time<Real>>,
    mut shake: ResMut<Shake>,
    run: Res<RunState>,
    mut cameras: Query<(&mut Transform, &mut Eased, &Projection), With<Camera3d>>,
    player: Query<&GlobalTransform, (With<Player>, With<Fighter>)>,
    enemies: Query<&GlobalTransform, (With<Brain>, With<Fighter>)>,
) {
    let Ok((mut tf, mut eased, projection)) = cameras.single_mut() else { return };
    let aspect = match projection {
        Projection::Perspective(p) => p.aspect_ratio,
        _ => 16.0 / 9.0,
    };
    let me = player.single().ok().map(|g| g.translation().truncate());
    let mut points: Vec<Vec2> = me.into_iter().collect();
    points.extend(
        enemies.iter().map(|g| g.translation().truncate()).filter(|e| me.is_none_or(|m| e.distance(m) < ENGAGE)),
    );
    let target = frame(&points, run.0.spec.rim, aspect);
    let k = 1.0 - (-EASE * time.delta_secs()).exp();
    let View { centre, height } = eased.0;
    eased.0 = View { centre: centre.lerp(target.centre, k), height: height + (target.height - height) * k };
    shake.drain(real.delta_secs());
    let jolt = View {
        centre: eased.0.centre + shake.offset(real.elapsed_secs()),
        height: eased.0.height * (1.0 - PUNCH_IN * shake.trauma),
    };
    *tf = placement(jolt);
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIDE: f32 = 16.0 / 9.0;

    #[test]
    fn a_lone_player_in_box_one_fills_the_screen_with_the_box() {
        let v = frame(&[Vec2::new(-400.0, floor_top() + 26.0)], 185.0, WIDE);
        let span = 185.0 + HEADROOM + BELOW;
        assert!((v.height - span).abs() < 1.0, "fits floor to rim plus headroom, got {}", v.height);
        assert!(v.height < ARENA.y * 0.55, "nearly twice as close as the whole world");
        assert!(v.centre.x < 0.0, "follows the player toward the left wall");
    }

    #[test]
    fn the_view_never_shows_past_the_outer_walls() {
        let v = frame(&[Vec2::new(-ARENA.x / 2.0 + 40.0, floor_top() + 26.0)], 185.0, WIDE);
        assert!(v.centre.x - v.height * WIDE / 2.0 >= -ARENA.x / 2.0 - 0.01);
    }

    #[test]
    fn fighters_far_apart_widen_the_view_up_to_the_whole_box() {
        let y = floor_top() + 26.0;
        let v = frame(&[Vec2::new(-600.0, y), Vec2::new(600.0, y)], 185.0, WIDE);
        assert!((v.height * WIDE - ARENA.x).abs() < 1.0, "exactly the box's width");
        assert_eq!(v.centre.x, 0.0);
    }

    #[test]
    fn a_tall_box_shows_its_rim() {
        let v = frame(&[Vec2::new(0.0, floor_top() + 26.0)], 600.0, WIDE);
        assert!(v.centre.y + v.height / 2.0 >= floor_top() + 600.0);
    }
}
