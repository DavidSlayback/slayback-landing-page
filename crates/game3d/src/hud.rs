//! The HUD (`bevy_ui`), top right, clear of the host page's back link: the player's health as a bar in the scheme's
//! main accent, then the box number, the box clock, and the score with its streak multiplier. The start, pause and
//! game-over screens are `screens`.

use bevy::prelude::*;

use crate::fighter::Hp;
use crate::look::Palette;
use crate::paint::{Role, colour};
use crate::player::Player;
use crate::run::RunState;
use crate::score::RunScore;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn).add_systems(Update, (update, repaint.run_if(resource_changed::<Palette>)));
}

#[derive(Component)]
struct HealthFill;

#[derive(Component)]
struct Readout;

/// Text colour, shared with the screens.
pub(crate) const INK: Color = Color::srgb(0.93, 0.98, 0.9);

fn spawn(mut commands: Commands, palette: Res<Palette>) {
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: Val::Px(18.0),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            padding: UiRect::horizontal(Val::Px(28.0)),
            justify_content: JustifyContent::FlexEnd,
            column_gap: Val::Px(28.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|hud| {
            hud.spawn((
                Node { width: Val::Px(240.0), height: Val::Px(12.0), ..default() },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.14)),
            ))
            .with_child((
                HealthFill,
                Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
                BackgroundColor(colour(Role::Ring, &palette)),
            ));
            hud.spawn((Readout, Text::new(""), TextFont::from_font_size(20.0), TextColor(INK), TextShadow::default()));
        });
}

/// The bar follows the scheme as boxes rotate it.
fn repaint(palette: Res<Palette>, mut fill: Query<&mut BackgroundColor, With<HealthFill>>) {
    for mut bg in &mut fill {
        bg.0 = colour(Role::Ring, &palette);
    }
}

fn update(
    run: Res<RunState>,
    score: Res<RunScore>,
    player: Query<&Hp, With<Player>>,
    mut fill: Query<&mut Node, With<HealthFill>>,
    mut readout: Query<&mut Text, With<Readout>>,
) {
    if let (Ok(hp), Ok(mut node)) = (player.single(), fill.single_mut()) {
        let width = Val::Percent((hp.0.hp / hp.0.max * 100.0).clamp(0.0, 100.0));
        if node.width != width {
            node.width = width;
        }
    }
    if let Ok(mut t) = readout.single_mut() {
        let streak = if score.0.streak > 1 { format!("  x{:.1}", score.0.multiplier()) } else { String::new() };
        let secs = run.0.time_left.ceil().max(0.0) as u32;
        let line = format!("Box {}   {}:{:02}   {}{}", run.0.spec.number, secs / 60, secs % 60, score.0.points, streak);
        if t.0 != line {
            t.0 = line;
        }
    }
}
