//! HUD and screens (`bevy_ui`): health, timer, score and box while playing; the start overlay with the controls, the
//! pause screen, and the game-over screen with score, box reached and best. Only the windowed game adds this plugin;
//! it also puts a fresh page on the start overlay.

use bevy::prelude::*;

use crate::fighter::Hp;
use crate::flow::Phase;
use crate::look::Scheme;
use crate::player::Player;
use crate::run::RunState;
use crate::score::{BestScore, LastRun, RunScore};
use crate::sound::Sound;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, (spawn_hud, spawn_screens, |mut next: ResMut<NextState<Phase>>| next.set(Phase::Start)))
        .add_systems(Update, (update_hud, show_screens, show_sound))
        // Every frame while over: the run's final numbers can land a frame after the screen appears.
        .add_systems(Update, write_game_over.run_if(in_state(Phase::Over)));
}

#[derive(Component)]
struct HealthFill;

#[derive(Component)]
struct Readout;

/// A full-screen overlay shown in one phase.
#[derive(Component)]
struct Screen(Phase);

#[derive(Component)]
struct GameOverText;

/// The pause screen's sound toggle.
#[derive(Component)]
struct SoundLine;

const INK: Color = Color::srgb(0.93, 0.98, 0.9);

fn text(s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (Text::new(s), TextFont::from_font_size(size), TextColor(color))
}

fn spawn_hud(mut commands: Commands, scheme: Res<Scheme>) {
    let accent = scheme.accents[0];
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: Val::Px(18.0),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            padding: UiRect::horizontal(Val::Px(28.0)),
            justify_content: JustifyContent::FlexEnd,
            column_gap: Val::Px(36.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|hud| {
            // Health: a bar in the scheme's accent.
            hud.spawn(Node { width: Val::Px(220.0), height: Val::Px(10.0), ..default() })
                .insert(BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.12)))
                .with_child((
                    HealthFill,
                    Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
                    BackgroundColor(accent),
                ));
            hud.spawn((Readout, text("", 18.0, INK)));
        });
}

fn spawn_screens(mut commands: Commands) {
    let controls = "A / D  run      Space / Shift  dash\nJ / left click  punch      K / right click  kick\nattack while dashing  drop-kick\nL / middle click  grab      attack while holding  spin-throw\nEsc  pause\n\nPile the fallen up to the rim to escape the box.";
    let screens: [(Phase, Vec<(String, f32)>); 3] = [
        (
            Phase::Start,
            vec![
                ("ESCAPE THE BOX".into(), 46.0),
                (controls.into(), 18.0),
                ("Click or press Enter to start".into(), 22.0),
            ],
        ),
        (Phase::Paused, vec![("PAUSED".into(), 46.0), ("Esc to resume".into(), 22.0)]),
        (Phase::Over, vec![("GAME OVER".into(), 46.0)]),
    ];
    for (phase, lines) in screens {
        commands
            .spawn((
                Screen(phase),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(26.0),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
                GlobalZIndex(10),
            ))
            .with_children(|s| {
                for (line, size) in lines {
                    s.spawn((text(line, size, INK), TextLayout::justify(Justify::Center)));
                }
                if phase == Phase::Over {
                    s.spawn((GameOverText, text("", 22.0, INK), TextLayout::justify(Justify::Center)));
                }
                if phase == Phase::Paused {
                    s.spawn((SoundLine, text("", 18.0, INK), TextLayout::justify(Justify::Center)));
                }
            });
    }
}

fn show_screens(phase: Res<State<Phase>>, mut screens: Query<(&Screen, &mut Node)>) {
    if !phase.is_changed() {
        return;
    }
    for (screen, mut node) in &mut screens {
        node.display = if screen.0 == *phase.get() { Display::Flex } else { Display::None };
    }
}

fn update_hud(
    run: Res<RunState>,
    score: Res<RunScore>,
    scheme: Res<Scheme>,
    player: Query<&Hp, With<Player>>,
    mut fill: Query<(&mut Node, &mut BackgroundColor), With<HealthFill>>,
    mut readout: Query<&mut Text, With<Readout>>,
) {
    if let (Ok(hp), Ok((mut node, mut color))) = (player.single(), fill.single_mut()) {
        node.width = Val::Percent((hp.0.hp / hp.0.max * 100.0).clamp(0.0, 100.0));
        color.0 = scheme.accents[0];
    }
    if let Ok(mut t) = readout.single_mut() {
        let secs = run.0.time_left.max(0.0).ceil() as u32;
        let streak = if score.0.streak > 1 { format!("  x{:.1}", score.0.multiplier()) } else { String::new() };
        t.0 = format!("BOX {}    {}:{:02}    {}{}", run.0.spec.number, secs / 60, secs % 60, score.0.points, streak);
    }
}

fn write_game_over(last: Res<LastRun>, best: Res<BestScore>, mut text: Query<&mut Text, With<GameOverText>>) {
    let Ok(mut t) = text.single_mut() else { return };
    let best_line = match &best.0 {
        Some(b) if last.new_best => format!("New best!  (box {}, {})", b.reached, b.date),
        Some(b) => format!("Best  {}  (box {}, {})", b.score, b.reached, b.date),
        None => String::new(),
    };
    t.0 =
        format!("Score  {}    Box  {}\n{}\n\nClick or press Enter to play again", last.score, last.reached, best_line);
}

fn show_sound(sound: Option<Res<Sound>>, mut line: Query<&mut Text, With<SoundLine>>) {
    let (Some(sound), Ok(mut t)) = (sound, line.single_mut()) else { return };
    t.0 = format!("M  sound {}", if sound.on { "on" } else { "off" });
}
