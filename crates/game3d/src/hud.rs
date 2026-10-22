//! The HUD (`bevy_ui`), top right, clear of the host page's back link: the player's health as a bar in Rumble Box's
//! health green, then the box number, the box clock, and the score with its streak multiplier; on a touch screen, a
//! pause sign first, since a tap on the HUD pauses. Under it, the climb gauge (Q80): how high the pile under the
//! player stands against the rim, in the player's ring colour at a wall and grey away from one (only a pile against a
//! wall gets you out, Q81), with a tick at the tallest heap against a wall. The start, pause and game-over screens are
//! `screens`.

use bevy::prelude::*;

use crate::fighter::Hp;
use crate::input::Touch;
use crate::look::Palette;
use crate::paint::{Role, colour};
use crate::player::Player;
use crate::run::{Climb, RunState};
use crate::score::RunScore;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn).add_systems(Update, (update, update_climb).in_set(crate::Frame::Show));
}

#[derive(Component)]
struct HealthFill;

#[derive(Component)]
struct Readout;

#[derive(Component)]
struct ClimbFill;

#[derive(Component)]
struct ClimbTallest;

#[derive(Component)]
struct ClimbReadout;

/// The climb gauge's length, pixels: the rim is its right end.
const GAUGE: f32 = 240.0;

/// Text colour, shared with the screens.
pub(crate) const INK: Color = Color::srgb(0.93, 0.98, 0.9);

/// A crisp dark edge under text drawn over the box, so it reads on every stage's tiles, pale or dark (Q75).
pub(crate) const EDGE: TextShadow = TextShadow { offset: Vec2::splat(2.0), color: Color::BLACK };

fn spawn(mut commands: Commands, palette: Res<Palette>) {
    let row = commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: Val::Px(18.0),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            padding: UiRect::horizontal(Val::Px(28.0)),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexEnd,
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    // On a dark panel: the HUD sits over the sky or the box's back wall, which runs from white to near-black by stage
    // (Q75), and light text on the pale ones didn't read.
    commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                column_gap: Val::Px(24.0),
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            ChildOf(row),
        ))
        .with_children(|hud| {
            hud.spawn((
                Node { width: Val::Px(240.0), height: Val::Px(12.0), ..default() },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.14)),
            ))
            .with_child((
                HealthFill,
                Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
                BackgroundColor(colour(Role::Health, &palette)),
            ));
            hud.spawn((Readout, Text::new(""), TextFont::from_font_size(20.0), TextColor(INK), EDGE));
        });
    // The climb gauge, on its own panel under the HUD: label, bar to the rim, the numbers.
    commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(16.0), Val::Px(6.0)),
                column_gap: Val::Px(14.0),
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            ChildOf(row),
        ))
        .with_children(|gauge| {
            gauge.spawn((Text::new("Climb"), TextFont::from_font_size(16.0), TextColor(INK), EDGE));
            gauge
                .spawn((
                    Node { width: Val::Px(GAUGE), height: Val::Px(12.0), ..default() },
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.14)),
                ))
                .with_children(|bar| {
                    bar.spawn((
                        ClimbFill,
                        Node { width: Val::Px(0.0), height: Val::Percent(100.0), ..default() },
                        BackgroundColor(colour(Role::Ring, &palette)),
                    ));
                    bar.spawn((
                        ClimbTallest,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(0.0),
                            top: Val::Px(-3.0),
                            width: Val::Px(3.0),
                            height: Val::Px(18.0),
                            ..default()
                        },
                        BackgroundColor(INK),
                    ));
                });
            gauge.spawn((ClimbReadout, Text::new(""), TextFont::from_font_size(16.0), TextColor(INK), EDGE));
        });
}

/// Where `height` sits along the gauge, pixels (the rim at the far end, nothing past it).
fn along(height: f32, rim: f32) -> f32 {
    if rim <= 0.0 { 0.0 } else { (height / rim).clamp(0.0, 1.0) * GAUGE }
}

/// The gauge's fill away from a wall: a pile there doesn't get you out.
const OFF_WALL: Color = Color::srgba(1.0, 1.0, 1.0, 0.35);

#[allow(clippy::type_complexity)]
fn update_climb(
    climb: Res<Climb>,
    palette: Res<Palette>,
    mut fill: Query<(&mut Node, &mut BackgroundColor), (With<ClimbFill>, Without<ClimbTallest>)>,
    mut tallest: Query<&mut Node, (With<ClimbTallest>, Without<ClimbFill>)>,
    mut readout: Query<&mut Text, With<ClimbReadout>>,
) {
    if !climb.is_changed() {
        return;
    }
    if let Ok((mut node, mut paint)) = fill.single_mut() {
        node.width = Val::Px(along(climb.under, climb.rim));
        paint.0 = if climb.at_wall { colour(Role::Ring, &palette) } else { OFF_WALL };
    }
    if let Ok(mut node) = tallest.single_mut() {
        node.left = Val::Px((along(climb.tallest, climb.rim) - 1.5).max(0.0));
    }
    if let Ok(mut t) = readout.single_mut() {
        let wall = if climb.at_wall || climb.under < 0.05 { "" } else { "   needs a wall" };
        t.0 = format!("{:.1} / {:.1} m{wall}", climb.under, climb.rim);
    }
}

fn update(
    run: Res<RunState>,
    score: Res<RunScore>,
    touch: Res<Touch>,
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
        let pause = if touch.used { "II   " } else { "" };
        let line =
            format!("{pause}Box {}   {}:{:02}   {}{}", run.0.spec.number, secs / 60, secs % 60, score.0.points, streak);
        if t.0 != line {
            t.0 = line;
        }
    }
}
