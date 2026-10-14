//! The player: moves in eight directions over the floor and faces the way it last moved, turning smoothly (Q49); dashes;
//! and punches, kicks and drop-kicks (any attack during a dash), with aim assist turning a swing toward the nearest
//! enemy inside its cone.
//!
//! G1's body is a dynamic capsule whose floor velocity is set from input each frame, so the solver stops it at walls;
//! the pile-climbing move-and-slide controller with its step-up is G3's.

use avian3d::prelude::*;
use bevy::prelude::*;
use game_core::aim::{Cone, level};
use game_core::combat::Move;
use game_core::plane::{self, Dash, run_velocity};

use crate::enemy::Brain;
use crate::fighter::{Attack, Combat, Defeated, Hp, Team, body};
use crate::input::Actions;
use crate::{on_floor, plane as to_plane};

/// The player.
#[derive(Component, Default)]
pub struct Player;

/// The player's dash (`game_core::plane::Dash`).
#[derive(Component, Default)]
pub struct Dasher(pub Dash);

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
pub const MAX_HP: f32 = 100.0;
/// Spawn point (the capsule's centre): mid-floor, a little nearer the camera, just above the floor.
pub const SPAWN: Vec3 = Vec3::new(0.0, HEIGHT / 2.0 + 0.3, 1.5);

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn).add_systems(Update, (control.before(Combat), recover.after(Combat)));
}

fn spawn(mut commands: Commands) {
    commands.spawn((
        Player,
        Name::new("player"),
        Dasher::default(),
        Facing::default(),
        body(Team::Player, RADIUS, HEIGHT, MAX_HP),
        Transform::from_translation(SPAWN),
    ));
}

/// Turns this frame's actions (any device; see `input`) into floor velocity, facing and attacks. Public so other
/// systems can order themselves after it.
#[allow(clippy::type_complexity)]
pub fn control(
    time: Res<Time>,
    actions: Res<Actions>,
    enemies: Query<&Transform, (With<Brain>, Without<Player>)>,
    mut players: Query<(&Transform, &mut LinearVelocity, &mut Facing, &mut Dasher, &mut Attack, &Hp), With<Player>>,
) {
    let Ok((transform, mut velocity, mut facing, mut dash, mut attack, hp)) = players.single_mut() else { return };
    let dt = time.delta_secs();
    if hp.0.stunned() {
        return; // knockback plays out
    }
    let button = if actions.verbs.punch {
        Some(Move::Punch)
    } else if actions.verbs.kick {
        Some(Move::Kick)
    } else {
        None
    };
    if let Some(button) = button.filter(|_| !attack.busy()) {
        let mv = Move::for_button(button, dash.0.dashing());
        // Aim assist: turn toward the nearest enemy in the swing's cone (a drop-kick's dash bends toward it too).
        let me = transform.translation;
        let cone = Cone::of(mv, to_plane(me), facing.0.yaw, RADIUS);
        let level_enemies: Vec<Vec3> = enemies.iter().map(|t| t.translation).filter(|e| level(me.y, e.y)).collect();
        if let Some((_, yaw)) = cone.assist(level_enemies.iter().map(|e| to_plane(*e)), crate::enemy::RADIUS) {
            facing.0.snap(yaw);
            dash.0.aim(facing.0.forward());
        }
        attack.start(mv);
    }
    // A punch or kick plants the feet; a drop-kick rides the dash.
    if attack.swing.is_some_and(|s| s.mv != Move::DropKick) {
        let v = run_velocity(to_plane(velocity.0), [0.0, 0.0], dt);
        velocity.x = v[0];
        velocity.z = -v[1];
        return;
    }
    let v = on_floor(dash.0.step(to_plane(velocity.0), actions.dir, actions.verbs.dash, facing.0.forward(), dt));
    velocity.x = v.x;
    velocity.z = v.z;
    facing.0.step(if dash.0.dashing() { dash.0.direction() } else { actions.dir }, dt);
}

/// Until G3's run loop ends the run on a defeat, a beaten player gets back up at full health.
fn recover(mut defeats: MessageReader<Defeated>, mut players: Query<&mut Hp, With<Player>>) {
    for d in defeats.read().filter(|d| d.team == Team::Player) {
        if let Ok(mut hp) = players.get_mut(d.entity) {
            hp.0.hp = hp.0.max;
        }
    }
}
