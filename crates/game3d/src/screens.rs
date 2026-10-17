//! The screens (`bevy_ui`), as in the 2.5D game (decision Q34): the start overlay with the controls, the pause screen
//! with the sound toggle, and the game-over screen with the score, the box reached and the best. On a touch screen
//! (`input::Touch`) they give the touch controls and say "tap". Only the window adds this plugin, and it puts a fresh
//! page on the start overlay (the demo skips it).

use bevy::prelude::*;

use crate::flow::Phase;
use crate::hud::INK;
use crate::input::{SOUND_BAND, Touch};
use crate::score::{BestScore, LastRun};
use crate::sound::Sound;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, (spawn, |mut next: ResMut<NextState<Phase>>| next.set(Phase::Start)))
        .add_systems(Update, (show, write_sound, write_prompts).in_set(crate::Frame::Show))
        // Every frame while over: the run's final numbers can land a frame after the screen appears.
        .add_systems(Update, write_game_over.run_if(in_state(Phase::Over)));
}

/// A full-screen overlay shown in one phase.
#[derive(Component)]
struct Screen(Phase);

#[derive(Component)]
struct GameOverText;

/// The pause screen's sound toggle.
#[derive(Component)]
struct SoundLine;

/// A line that reads differently on a touch screen.
#[derive(Component)]
struct Prompt {
    keys: &'static str,
    touch: &'static str,
}

const CONTROLS: &str = "WASD / arrows  move      Space / Shift  dash\n\
    J / left click  punch      K / right click  kick\n\
    attack while dashing  drop-kick\n\
    L / middle click  grab      attack while holding  spin-throw\n\
    Esc  pause      gamepad: stick, A dash, X punch, Y kick, B grab, Start pause\n\n\
    Pile the fallen up to the rim to escape the box.";

// Short lines: a phone held sideways is about 670 to 930 pixels across.
const TOUCH_CONTROLS: &str = "left thumb  move      right thumb  the buttons, or\n\
    tap  punch      swipe  dash that way      hold  grab\n\
    dash + attack  drop-kick      carrying + attack  spin-throw\n\
    tap the score  pause\n\n\
    Pile the fallen up to the rim to escape the box.";

fn text(s: impl Into<String>, size: f32) -> impl Bundle {
    (Text::new(s), TextFont::from_font_size(size), TextColor(INK), TextLayout::justify(Justify::Center))
}

fn spawn(mut commands: Commands) {
    let start = Prompt { keys: "Click or press Enter (Start on a gamepad) to start", touch: "Tap to start" };
    let screens: [(Phase, &[(Prompt, f32)]); 3] = [
        (
            Phase::Start,
            &[
                (Prompt { keys: "ESCAPE THE BOX", touch: "ESCAPE THE BOX" }, 46.0),
                (Prompt { keys: CONTROLS, touch: TOUCH_CONTROLS }, 18.0),
                (start, 22.0),
            ],
        ),
        (
            Phase::Paused,
            &[
                (Prompt { keys: "PAUSED", touch: "PAUSED" }, 46.0),
                (Prompt { keys: "Esc or Start to resume", touch: "Tap to resume" }, 22.0),
            ],
        ),
        (Phase::Over, &[(Prompt { keys: "GAME OVER", touch: "GAME OVER" }, 46.0)]),
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
                for &(Prompt { keys, touch }, size) in lines {
                    s.spawn((text(keys, size), Prompt { keys, touch }));
                }
                match phase {
                    Phase::Over => {
                        s.spawn((GameOverText, text("", 22.0)));
                    }
                    // Low on the screen, where a tap toggles it on a touch screen.
                    Phase::Paused => {
                        s.spawn((
                            SoundLine,
                            text("", 18.0),
                            Node {
                                position_type: PositionType::Absolute,
                                bottom: Val::Percent((1.0 - SOUND_BAND) * 100.0 / 2.0),
                                ..default()
                            },
                        ));
                    }
                    _ => {}
                }
            });
    }
}

fn show(phase: Res<State<Phase>>, mut screens: Query<(&Screen, &mut Node)>) {
    if !phase.is_changed() {
        return;
    }
    for (screen, mut node) in &mut screens {
        node.display = if screen.0 == *phase.get() { Display::Flex } else { Display::None };
    }
}

/// The game-over lines: the run's score and box, then the best (flagged when this run set it).
fn game_over_lines(last: &LastRun, best: Option<&game_core::score::Best>, touch: bool) -> String {
    let best_line = match best {
        Some(b) if last.new_best => format!("New best!  (box {}, {})", b.reached, b.date),
        Some(b) => format!("Best  {}  (box {}, {})", b.score, b.reached, b.date),
        None => String::new(),
    };
    let again = if touch { "Tap to play again" } else { "Click or press Enter to play again" };
    format!("Score  {}    Box  {}\n{}\n\n{again}", last.score, last.reached, best_line)
}

fn write_game_over(
    last: Res<LastRun>,
    best: Res<BestScore>,
    touch: Res<Touch>,
    mut text: Query<&mut Text, With<GameOverText>>,
) {
    let Ok(mut t) = text.single_mut() else { return };
    let lines = game_over_lines(&last, best.0.as_ref(), touch.used);
    if t.0 != lines {
        t.0 = lines;
    }
}

fn write_sound(sound: Res<Sound>, touch: Res<Touch>, mut line: Query<&mut Text, With<SoundLine>>) {
    let Ok(mut t) = line.single_mut() else { return };
    let how = if touch.used { "Tap here:" } else { "M / Select" };
    let lines = format!("{how}  sound {}", if sound.on { "on" } else { "off" });
    if t.0 != lines {
        t.0 = lines;
    }
}

/// Says "tap" once the visitor is on a touch screen.
fn write_prompts(touch: Res<Touch>, mut lines: Query<(&Prompt, &mut Text)>) {
    for (p, mut t) in &mut lines {
        let want = if touch.used { p.touch } else { p.keys };
        if t.0 != want {
            t.0 = want.into();
        }
    }
}
