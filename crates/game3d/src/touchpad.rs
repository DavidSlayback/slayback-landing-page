//! The touch controls on screen (Q78, `game_core::touch`): the stick, resting faintly at the bottom left until a thumb
//! lands on the left half and then centred under it, and the diamond of buttons at the bottom right. Drawn while
//! playing, on a touch screen or once the visitor has touched.

use bevy::prelude::*;
use game_core::touch::{Button, RANGE};

use crate::flow::Phase;
use crate::hud::{EDGE, INK};
use crate::input::Touch;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn).add_systems(Update, update);
}

#[derive(Component)]
struct Controls;

#[derive(Component)]
struct StickBase;

#[derive(Component)]
struct StickKnob;

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
    commands.spawn((
        StickBase,
        circle(RANGE),
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.25)),
        BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.4)),
        ChildOf(root),
    ));
    commands.spawn((StickKnob, circle(KNOB), BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.45)), ChildOf(root)));
    for b in Button::ALL {
        commands
            .spawn((PadButton(b), circle(30.0), BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.5)), ChildOf(root)))
            .with_child((Text::new(b.label()), TextFont::from_font_size(11.0), TextColor(INK), EDGE));
    }
}

/// Each part of the controls, apart (no entity is two of them).
type Base<'w, 's> = Query<'w, 's, &'static mut Node, (With<StickBase>, Without<Controls>)>;
type Knob<'w, 's> = Query<
    'w,
    's,
    (&'static mut Node, &'static mut BackgroundColor),
    (With<StickKnob>, Without<Controls>, Without<StickBase>),
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
    mut base: Base,
    mut knob: Knob,
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
    // At rest the stick sits faintly at the bottom left, a hint of where it is; under a thumb it is clear.
    let home = touch.pad.layout.stick_home();
    let (held, (centre, at)) = touch.pad.stick().map_or((false, (home, home)), |s| (true, s));
    if let Ok(mut node) = base.single_mut() {
        place(&mut node, centre, RANGE);
    }
    if let Ok((mut node, mut fill)) = knob.single_mut() {
        place(&mut node, at, KNOB);
        fill.set_if_neq(BackgroundColor(Color::srgba(1.0, 1.0, 1.0, if held { 0.55 } else { 0.25 })));
    }
    let r = touch.pad.layout.radius();
    for (PadButton(b), mut node, mut fill) in &mut buttons {
        place(&mut node, touch.pad.layout.button(*b), r);
        let shade =
            if touch.pad.held(*b) { Color::srgba(1.0, 1.0, 1.0, 0.4) } else { Color::srgba(0.0, 0.0, 0.0, 0.4) };
        fill.set_if_neq(BackgroundColor(shade));
    }
}
