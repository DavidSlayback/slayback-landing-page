//! The player: moves in eight directions over the floor and faces the way it last moved, turning smoothly (Q49).
//!
//! G1's body is a dynamic capsule whose floor velocity is set from input each frame, so the solver stops it at walls;
//! the pile-climbing move-and-slide controller with its step-up is G3's.

use avian3d::prelude::*;
use bevy::prelude::*;
use game_core::plane::{self, run_velocity};

use crate::input::Actions;
use crate::{on_floor, plane as to_plane};

/// The player.
#[derive(Component, Default)]
pub struct Player;

/// Which way a fighter faces on the floor (`game_core::plane::Facing`).
#[derive(Component, Default, Clone, Copy, Debug)]
pub struct Facing(pub plane::Facing);

impl Facing {
    /// The world rotation for this facing, for a figure built looking along +x.
    pub fn rotation(&self) -> Quat {
        Quat::from_rotation_y(self.0.yaw)
    }
}

/// Capsule radius and full height, metres.
pub const RADIUS: f32 = 0.35;
pub const HEIGHT: f32 = 1.75;
/// Spawn point (the capsule's centre): mid-floor, a little nearer the camera, just above the floor.
pub const SPAWN: Vec3 = Vec3::new(0.0, HEIGHT / 2.0 + 0.3, 1.5);

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn).add_systems(Update, control);
}

fn spawn(mut commands: Commands) {
    commands.spawn((
        Player,
        Name::new("player"),
        Facing::default(),
        Transform::from_translation(SPAWN),
        RigidBody::Dynamic,
        Collider::capsule(RADIUS, HEIGHT - 2.0 * RADIUS),
        LockedAxes::ROTATION_LOCKED,
        // Frictionless, so walls don't hold the player; floor movement is set on the velocity.
        Friction::ZERO.with_combine_rule(CoefficientCombine::Min),
        SleepingDisabled,
        TransformInterpolation,
    ));
}

/// Turns this frame's move direction (any device; see `input`) into floor velocity and facing. Public so other
/// systems can order themselves after it.
pub fn control(
    time: Res<Time>,
    actions: Res<Actions>,
    mut players: Query<(&mut LinearVelocity, &mut Facing), With<Player>>,
) {
    let Ok((mut velocity, mut facing)) = players.single_mut() else { return };
    let dt = time.delta_secs();
    let v = on_floor(run_velocity(to_plane(velocity.0), actions.dir, dt));
    velocity.x = v.x;
    velocity.z = v.z;
    facing.0.step(actions.dir, dt);
}
