//! The grab tip (playtest feedback, Q80): a playtester didn't find out for "a dozen or so playthroughs" that the blocks
//! a beaten enemy breaks into can be grabbed. So the first kill of each run shows a tip saying so, for [`SHOW`]
//! seconds of play, until the player has grabbed a block once (then it never shows again on this page). The rule is
//! gameplay-side, so the headless scenarios see it; the window draws it (`view`).

use bevy::prelude::*;
use game_core::grab::Held;

use crate::Frame;
use crate::carry::Carry;
use crate::fighter::{Defeated, Team};
use crate::player::Player;
use crate::run::BoxStarted;

/// Seconds of play the tip stays up.
pub const SHOW: f32 = 8.0;

/// What the tip says, with the keys and gamepad, or on a touch screen.
pub const KEYS: &str = "Beaten enemies break into blocks, and you can pick the blocks up even after they land: \
    L (or B) grabs one.\nHeap them against a wall and climb. When the pile under your feet reaches the rim, you're out.";
pub const TOUCH: &str = "Beaten enemies break into blocks, and you can pick them up even after they land: \
    GRAB grabs one.\nHeap them against a wall and climb. When the pile under your feet reaches the rim, you're out.";

/// The tip's state.
#[derive(Resource, Debug, Default)]
pub struct GrabTip {
    /// The player has grabbed a block: the tip has done its job.
    pub learned: bool,
    /// Seconds of play it stays up for; 0 when hidden.
    pub left: f32,
    /// It has come up in this run already.
    shown: bool,
}

impl GrabTip {
    pub fn showing(&self) -> bool {
        self.left > 0.0 && !self.learned
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<GrabTip>().add_systems(Update, watch.in_set(Frame::Show));
    #[cfg(feature = "render")]
    app.add_plugins(view::plugin);
}

fn watch(
    time: Res<Time>,
    mut tip: ResMut<GrabTip>,
    mut started: MessageReader<BoxStarted>,
    mut defeated: MessageReader<Defeated>,
    carry: Query<&Carry, With<Player>>,
) {
    // A run starts at box 1 (the first, or again after a death).
    if started.read().any(|BoxStarted(spec)| spec.number == 1) {
        tip.shown = false;
    }
    if carry.iter().any(|c| c.kind == Held::Debris) {
        tip.learned = true;
        tip.left = 0.0;
    }
    let killed = defeated.read().any(|d| d.team == Team::Enemy);
    if killed && !tip.learned && !tip.shown {
        tip.shown = true;
        tip.left = SHOW;
    }
    // The play clock: the tip waits out a pause.
    tip.left = (tip.left - time.delta_secs()).max(0.0);
}

#[cfg(feature = "render")]
mod view {
    use bevy::prelude::*;

    use super::{GrabTip, KEYS, TOUCH};
    use crate::hud::{EDGE, INK};
    use crate::input::Touch;

    #[derive(Component)]
    struct Tip;

    pub fn plugin(app: &mut App) {
        app.add_systems(Startup, spawn).add_systems(Update, show.in_set(crate::Frame::Show));
    }

    /// Low and centred, on a dark panel like the HUD's, clear of the touch controls' thumbs.
    fn spawn(mut commands: Commands) {
        let row = commands
            .spawn(Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(28.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                ..default()
            })
            .id();
        commands.spawn((
            Tip,
            Node {
                max_width: Val::Px(760.0),
                padding: UiRect::axes(Val::Px(18.0), Val::Px(10.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
            Text::new(KEYS),
            TextFont::from_font_size(18.0),
            TextColor(INK),
            TextLayout::justify(Justify::Center),
            EDGE,
            ChildOf(row),
        ));
    }

    fn show(tip: Res<GrabTip>, touch: Res<Touch>, mut node: Query<(&mut Node, &mut Text), With<Tip>>) {
        let Ok((mut node, mut text)) = node.single_mut() else { return };
        let display = if tip.showing() { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
        let want = if touch.used { TOUCH } else { KEYS };
        if text.0 != want {
            text.0 = want.into();
        }
    }
}
