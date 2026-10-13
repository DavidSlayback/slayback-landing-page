//! The player: a fighter with snappy horizontal control, a dash, and punch / kick / drop-kick (no jump: climbing is
//! walking up debris).

use avian2d::prelude::*;
use bevy::prelude::*;
use game_core::combat::Move;
use game_core::movement::{Input, Mover, run_velocity};

use crate::fighter::{Attack, Fighter, Hp, Team, body};
use crate::grab::Holding;
use crate::input::Actions;

/// The player, with its engine-free movement state.
#[derive(Component, Default)]
pub struct Player(pub Mover);

/// Player collider size, world units.
pub const SIZE: Vec2 = Vec2::new(28.0, 56.0);
/// Spawn point, world units.
pub const SPAWN: Vec2 = Vec2::new(0.0, -200.0);
pub const MAX_HP: f32 = 100.0;


pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn).add_systems(Update, control);
}

fn spawn(mut commands: Commands) {
    commands.spawn((
        Player::default(),
        Name::new("player"),
        body(Team::Player, SIZE / 2.0, MAX_HP),
        Transform::from_translation(SPAWN.extend(0.0)),
    ));
}

/// Turns this frame's actions (any device; see `input`) into movement and attacks. Public so other systems can order themselves after it.
#[allow(clippy::type_complexity)]
pub fn control(
    time: Res<Time>,
    actions: Res<Actions>,
    mut players: Query<(&mut Player, &mut Fighter, &mut Attack, &Hp, &mut LinearVelocity, Has<Holding>)>,
) {
    let Ok((mut player, mut fighter, mut attack, hp, mut velocity, holding)) = players.single_mut() else { return };
    let dt = time.delta_secs();
    if hp.0.stunned() {
        return; // knockback plays out
    }
    let button = if actions.punch {
        Some(Move::Punch)
    } else if actions.kick {
        Some(Move::Kick)
    } else {
        None
    };
    // While holding something, the attack buttons throw it instead (see `grab`).
    if let Some(button) = button.filter(|_| !holding) {
        attack.start(Move::for_button(button, player.0.dashing()));
    }
    // A punch or kick plants the feet; a drop-kick rides the dash.
    let planted = attack.swing.is_some_and(|s| s.mv != Move::DropKick);
    if planted {
        velocity.x = run_velocity(velocity.x, 0.0, dt);
        return;
    }
    let input = Input { axis: actions.axis, dash: actions.dash };
    velocity.x = player.0.step(velocity.x, input, dt);
    fighter.facing = player.0.facing;
}
