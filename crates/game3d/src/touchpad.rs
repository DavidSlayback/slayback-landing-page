//! The touch controls on screen (Q82, `game_core::touch`): two sticks fixed in the bottom corners, always drawn, the
//! left to move and the right to aim and punch, and Kick, Grab and Dash arced above the right one. Drawn while playing,
//! on a touch screen or once the visitor has touched.

use bevy::prelude::*;
use game_core::touch::{Button, RANGE, Stick};

use crate::flow::Phase;
use crate::hud::{EDGE, INK};
use crate::input::Touch;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn).add_systems(Update, update.in_set(crate::Frame::Show));
}

#[derive(Component)]
struct Controls;

#[derive(Component)]
struct StickBase(Stick);

#[derive(Component)]
struct StickKnob(Stick);

#[derive(Component)]
struct PadButton(Button);

/// The knob's radius, pixels.
const KNOB: f32 = 26.0;

fn circle(radius: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: Val::Px(2.0 * radius),
        height: Val::Px(2.0 * radius),
        border: UiRect::all(Val::Px(2.0)),
        border_radius: BorderRadius::MAX,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    }
}

fn spawn(mut commands: Commands) {
    let root = commands
        .spawn((
            Controls,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::None,
                ..default()
            },
            // Above the box, below the screens.
            GlobalZIndex(5),
        ))
        .id();
    for (s, label) in [(Stick::Move, "MOVE"), (Stick::Aim, "PUNCH")] {
        commands
            .spawn((
                StickBase(s),
                circle(RANGE),
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.3)),
                BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.5)),
                ChildOf(root),
            ))
            .with_child((
                Node { position_type: PositionType::Absolute, bottom: Val::Px(-20.0), ..default() },
                Text::new(label),
                TextFont::from_font_size(11.0),
                TextColor(INK.with_alpha(0.7)),
                EDGE,
            ));
        commands.spawn((StickKnob(s), circle(KNOB), BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.4)), ChildOf(root)));
    }
    for b in Button::ALL {
        commands
            .spawn((PadButton(b), circle(30.0), BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.5)), ChildOf(root)))
            .with_child((Text::new(b.label()), TextFont::from_font_size(11.0), TextColor(INK), EDGE));
    }
}

/// Each part of the controls, apart (no entity is two of them).
type Bases<'w, 's> = Query<'w, 's, (&'static StickBase, &'static mut Node), Without<Controls>>;
type Knobs<'w, 's> = Query<
    'w,
    's,
    (&'static StickKnob, &'static mut Node, &'static mut BackgroundColor),
    (Without<Controls>, Without<StickBase>),
>;
type Buttons<'w, 's> = Query<
    'w,
    's,
    (&'static PadButton, &'static mut Node, &'static mut BackgroundColor),
    (Without<Controls>, Without<StickBase>, Without<StickKnob>),
>;

fn update(
    touch: Res<Touch>,
    phase: Option<Res<State<Phase>>>,
    mut root: Query<&mut Node, With<Controls>>,
    mut bases: Bases,
    mut knobs: Knobs,
    mut buttons: Buttons,
) {
    let Ok(mut root) = root.single_mut() else { return };
    let show = touch.used && phase.is_none_or(|p| *p.get() == Phase::Playing);
    let display = if show { Display::Flex } else { Display::None };
    if root.display != display {
        root.display = display;
    }
    if !show {
        return;
    }
    // Only writes what moved, so the layout isn't redone every frame.
    let place = |node: &mut Mut<Node>, [x, y]: [f32; 2], r: f32| {
        let (left, top, size) = (Val::Px(x - r), Val::Px(y - r), Val::Px(2.0 * r));
        if (node.left, node.top, node.width) != (left, top, size) {
            (node.left, node.top, node.width, node.height) = (left, top, size, size);
        }
    };
    for (StickBase(s), mut node) in &mut bases {
        place(&mut node, touch.pad.layout.stick(*s), RANGE);
    }
    for (StickKnob(s), mut node, mut fill) in &mut knobs {
        let (_, at, held) = touch.pad.stick(*s);
        place(&mut node, at, KNOB);
        fill.set_if_neq(BackgroundColor(Color::srgba(1.0, 1.0, 1.0, if held { 0.65 } else { 0.4 })));
    }
    let r = touch.pad.layout.radius();
    for (PadButton(b), mut node, mut fill) in &mut buttons {
        place(&mut node, touch.pad.layout.button(*b), r);
        let shade =
            if touch.pad.held(*b) { Color::srgba(1.0, 1.0, 1.0, 0.4) } else { Color::srgba(0.0, 0.0, 0.0, 0.4) };
        fill.set_if_neq(BackgroundColor(shade));
    }
}
