//! Touch controls (decision Q78): a floating stick under the left thumb, and under the right thumb a diamond of
//! buttons laid out like the gamepad's face (punch left, kick top, grab right, dash bottom) on a pad that also takes
//! gestures, since the game may move too fast for buttons alone: a tap punches, a swipe dashes the way it went, a held
//! touch grabs. Tapping the HUD pauses. Engine-free: the game feeds it touches and draws it ([`TouchPad::stick`],
//! [`Layout`], [`TouchPad::held`]).
//!
//! Screen positions are logical pixels with y down, as touches arrive; the move direction it reports has y up the
//! screen, as the keys' and sticks' do.

use crate::controls::{Verbs, eight_way};

/// The buttons in the diamond.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Button {
    Punch,
    Kick,
    Grab,
    Dash,
}

impl Button {
    pub const ALL: [Button; 4] = [Button::Punch, Button::Kick, Button::Grab, Button::Dash];

    /// Where it sits in the diamond, as a unit offset from its centre (y down).
    fn offset(self) -> [f32; 2] {
        match self {
            Button::Punch => [-1.0, 0.0],
            Button::Kick => [0.0, -1.0],
            Button::Grab => [1.0, 0.0],
            Button::Dash => [0.0, 1.0],
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Button::Punch => "PUNCH",
            Button::Kick => "KICK",
            Button::Grab => "GRAB",
            Button::Dash => "DASH",
        }
    }
}

/// What a touch landed on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Zone {
    /// The HUD, top right: pauses.
    Pause,
    /// One of the diamond's buttons.
    Button(Button),
    /// The left half: the stick, centred where the thumb lands.
    Stick,
    /// The rest of the right half: gestures.
    Pad,
}

/// How far a touch must slide before it is a swipe rather than a tap or a hold, pixels.
pub const SWIPE: f32 = 40.0;
/// How long a touch held still (within [`SWIPE`]) is before it grabs, seconds.
pub const HOLD: f32 = 0.3;
/// The stick's travel: past `DEAD` pixels from where the thumb landed it moves, and the knob is drawn no further than
/// `RANGE`.
pub const DEAD: f32 = 14.0;
pub const RANGE: f32 = 56.0;
/// The HUD's band at the top of the screen, pixels.
const HUD_BAND: f32 = 72.0;

/// Where the controls sit on a screen of a given size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub size: [f32; 2],
}

impl Layout {
    /// A button's radius: big enough for a thumb, scaled down on small screens.
    pub fn radius(&self) -> f32 {
        (self.size[0].min(self.size[1]) * 0.085).clamp(24.0, 40.0)
    }

    /// How far from the bottom corners the stick's resting place and the diamond's centre are.
    fn inset(&self) -> f32 {
        self.radius() * 3.2
    }

    /// A button's centre.
    pub fn button(&self, b: Button) -> [f32; 2] {
        let [w, h] = self.size;
        let (r, at) = (self.radius(), self.inset());
        let [dx, dy] = b.offset();
        [w - at + dx * r * 1.75, h - at + dy * r * 1.75]
    }

    /// Where the stick is drawn while no thumb holds it: a hint at the bottom left.
    pub fn stick_home(&self) -> [f32; 2] {
        [self.inset(), self.size[1] - self.inset()]
    }

    pub fn zone(&self, p: [f32; 2]) -> Zone {
        let half = self.size[0] / 2.0;
        if p[1] < HUD_BAND && p[0] > half {
            return Zone::Pause;
        }
        // A little forgiveness round each button, for a thumb that lands near it.
        let reach = self.radius() * 1.2;
        if let Some(b) = Button::ALL.into_iter().find(|&b| distance(p, self.button(b)) <= reach) {
            return Zone::Button(b);
        }
        if p[0] < half { Zone::Stick } else { Zone::Pad }
    }
}

fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// A finger on the screen.
#[derive(Clone, Copy, Debug)]
struct Finger {
    id: u64,
    zone: Zone,
    start: [f32; 2],
    at: [f32; 2],
    since: f64,
    /// It has done what it does (a button pressed, a swipe's dash, a hold's grab), so it does nothing on release.
    spent: bool,
}

/// One frame of touch input.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Frame {
    /// The move direction (y up the screen): one of eight unit directions or zero. A swipe's dash goes its way.
    pub dir: [f32; 2],
    /// Punch, kick, grab, dash and pause, as pressed this frame.
    pub verbs: Verbs,
    /// Where a finger landed this frame, if one did, anywhere: what starts, resumes or restarts from a screen.
    pub landed: Option<[f32; 2]>,
}

/// The touches in play and what they have done since the last [`TouchPad::frame`].
#[derive(Clone, Debug)]
pub struct TouchPad {
    pub layout: Layout,
    fingers: Vec<Finger>,
    pending: Frame,
}

impl TouchPad {
    pub fn new(size: [f32; 2]) -> Self {
        Self { layout: Layout { size }, fingers: Vec::new(), pending: Frame::default() }
    }

    pub fn down(&mut self, id: u64, p: [f32; 2], t: f64) {
        let zone = self.layout.zone(p);
        self.pending.landed.get_or_insert(p);
        let spent = match zone {
            Zone::Pause => {
                self.pending.verbs.pause = true;
                true
            }
            Zone::Button(b) => {
                self.press(b);
                true
            }
            // Only one stick: a second finger on the left half is ignored.
            Zone::Stick if self.fingers.iter().any(|f| f.zone == Zone::Stick) => true,
            Zone::Stick | Zone::Pad => false,
        };
        self.fingers.retain(|f| f.id != id);
        self.fingers.push(Finger { id, zone, start: p, at: p, since: t, spent });
    }

    pub fn moved(&mut self, id: u64, p: [f32; 2]) {
        let Some(f) = self.fingers.iter_mut().find(|f| f.id == id) else { return };
        f.at = p;
        let d = [p[0] - f.start[0], f.start[1] - p[1]];
        if f.zone == Zone::Pad && !f.spent && d[0].hypot(d[1]) >= SWIPE {
            f.spent = true;
            self.pending.verbs.dash = true;
            self.pending.dir = eight_way(d);
        }
    }

    /// A finger lifted: a touch on the pad that never swiped or held was a tap, and punches.
    pub fn up(&mut self, id: u64) {
        let Some(i) = self.fingers.iter().position(|f| f.id == id) else { return };
        let f = self.fingers.swap_remove(i);
        if f.zone == Zone::Pad && !f.spent {
            self.pending.verbs.punch = true;
        }
    }

    /// A finger the browser took away (a system gesture): it does nothing more.
    pub fn cancel(&mut self, id: u64) {
        self.fingers.retain(|f| f.id != id);
    }

    /// Forgets every finger, so a touch that started or resumed the game doesn't also punch when it lifts.
    pub fn clear(&mut self) {
        for f in &mut self.fingers {
            f.spent = true;
        }
    }

    fn press(&mut self, b: Button) {
        let v = &mut self.pending.verbs;
        match b {
            Button::Punch => v.punch = true,
            Button::Kick => v.kick = true,
            Button::Grab => v.grab = true,
            Button::Dash => v.dash = true,
        }
    }

    /// This frame's input at time `t`, seconds; what was pressed is cleared for the next frame.
    pub fn frame(&mut self, t: f64) -> Frame {
        for f in &mut self.fingers {
            if f.zone == Zone::Pad && !f.spent && t - f.since >= HOLD as f64 {
                f.spent = true;
                self.pending.verbs.grab = true;
            }
        }
        let mut out = std::mem::take(&mut self.pending);
        if out.dir == [0.0, 0.0]
            && let Some((origin, at)) = self.stick_finger()
        {
            let d = [at[0] - origin[0], origin[1] - at[1]];
            if d[0].hypot(d[1]) >= DEAD {
                out.dir = eight_way(d);
            }
        }
        out
    }

    fn stick_finger(&self) -> Option<([f32; 2], [f32; 2])> {
        self.fingers.iter().find(|f| f.zone == Zone::Stick && !f.spent).map(|f| (f.start, f.at))
    }

    /// The stick under a thumb, to draw: its centre, and the knob's place (within [`RANGE`] of it). None while no
    /// thumb holds it.
    pub fn stick(&self) -> Option<([f32; 2], [f32; 2])> {
        self.stick_finger().map(|(o, at)| {
            let d = [at[0] - o[0], at[1] - o[1]];
            let k = (RANGE / d[0].hypot(d[1]).max(1e-3)).min(1.0);
            (o, [o[0] + d[0] * k, o[1] + d[1] * k])
        })
    }

    /// Whether a finger is on the button, for drawing it pressed.
    pub fn held(&self, b: Button) -> bool {
        self.fingers.iter().any(|f| f.zone == Zone::Button(b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A landscape phone.
    const SIZE: [f32; 2] = [800.0, 380.0];
    const LEFT: [f32; 2] = [150.0, 250.0];
    const RIGHT: [f32; 2] = [520.0, 180.0];

    fn pad() -> TouchPad {
        TouchPad::new(SIZE)
    }

    fn plus(p: [f32; 2], d: [f32; 2]) -> [f32; 2] {
        [p[0] + d[0], p[1] + d[1]]
    }

    #[test]
    fn the_stick_centres_where_the_thumb_lands_and_points_up_the_screen() {
        let mut p = pad();
        assert_eq!(p.layout.zone(LEFT), Zone::Stick);
        p.down(1, LEFT, 0.0);
        p.moved(1, plus(LEFT, [DEAD - 2.0, 0.0]));
        assert_eq!(p.frame(0.02).dir, [0.0, 0.0], "a little slide is inside the dead zone");
        p.moved(1, plus(LEFT, [0.0, -30.0]));
        assert_eq!(p.frame(0.04).dir, [0.0, 1.0], "dragging up the screen walks up it");
        p.moved(1, plus(LEFT, [200.0, 0.0]));
        assert_eq!(p.frame(0.06).dir, [1.0, 0.0], "held, it keeps walking");
        assert_eq!(p.stick(), Some((LEFT, plus(LEFT, [RANGE, 0.0]))), "the knob stops at the stick's edge");
        p.up(1);
        let f = p.frame(0.08);
        assert_eq!(f.dir, [0.0, 0.0]);
        assert_eq!(f.verbs, Verbs::default(), "letting go of the stick does nothing else");
        assert_eq!(p.stick(), None);
    }

    #[test]
    fn a_button_presses_the_frame_it_is_touched_while_the_stick_is_held() {
        let mut p = pad();
        p.down(1, LEFT, 0.0);
        p.moved(1, plus(LEFT, [-40.0, 0.0]));
        for b in Button::ALL {
            p.down(2, p.layout.button(b), 0.1);
            assert!(p.held(b));
            let f = p.frame(0.1);
            assert_eq!(f.dir, [-1.0, 0.0], "{b:?}: still walking");
            let pressed = [f.verbs.punch, f.verbs.kick, f.verbs.grab, f.verbs.dash];
            assert_eq!(pressed.iter().filter(|&&x| x).count(), 1, "{b:?}");
            p.up(2);
            assert_eq!(p.frame(0.9).verbs, Verbs::default(), "{b:?}: once, not on release or held");
        }
    }

    #[test]
    fn on_the_pad_a_tap_punches_a_swipe_dashes_its_way_and_a_hold_grabs() {
        let mut p = pad();
        assert_eq!(p.layout.zone(RIGHT), Zone::Pad);
        p.down(1, RIGHT, 0.0);
        assert!(!p.frame(0.05).verbs.punch, "not until it lifts: it might be a swipe");
        p.up(1);
        assert!(p.frame(0.1).verbs.punch, "a tap");

        p.down(2, RIGHT, 1.0);
        p.moved(2, plus(RIGHT, [-30.0, 30.0]));
        let f = p.frame(1.05);
        assert!(f.verbs.dash);
        assert_eq!(f.dir, eight_way([-1.0, -1.0]), "down and to the left");
        p.up(2);
        assert_eq!(p.frame(1.1).verbs, Verbs::default(), "a swipe doesn't punch when it lifts");

        p.down(3, RIGHT, 2.0);
        assert!(!p.frame(2.0 + HOLD as f64 * 0.9).verbs.grab);
        assert!(p.frame(2.0 + HOLD as f64).verbs.grab, "held still");
        assert!(!p.frame(3.0).verbs.grab, "once");
        p.up(3);
        assert_eq!(p.frame(3.1).verbs, Verbs::default(), "nor does a hold");
    }

    #[test]
    fn the_hud_pauses_and_any_touch_is_a_touch() {
        let mut p = pad();
        p.down(1, [SIZE[0] - 60.0, 30.0], 0.0);
        let f = p.frame(0.0);
        assert!(f.verbs.pause && f.landed == Some([SIZE[0] - 60.0, 30.0]));
        p.down(2, LEFT, 0.1);
        assert_eq!(p.frame(0.1).landed, Some(LEFT));
        assert_eq!(p.frame(0.2).landed, None, "only the frame it lands");
    }

    #[test]
    fn cleared_fingers_do_nothing_more() {
        let mut p = pad();
        p.down(1, RIGHT, 0.0);
        p.down(2, LEFT, 0.0);
        p.moved(2, plus(LEFT, [40.0, 0.0]));
        p.frame(0.0);
        p.clear();
        p.up(1);
        assert_eq!(p.frame(1.0), Frame::default(), "the touch that left a screen neither punches, grabs nor walks");
    }

    #[test]
    fn the_buttons_fit_on_screen_on_the_right_without_overlapping() {
        for size in [[800.0, 380.0], [568.0, 320.0], [1280.0, 720.0], [380.0, 800.0]] {
            let l = Layout { size };
            let r = l.radius();
            for (i, a) in Button::ALL.into_iter().enumerate() {
                let c = l.button(a);
                assert!(c[0] - r >= 0.0 && c[0] + r <= size[0] && c[1] + r <= size[1], "{size:?} {a:?}");
                assert!(c[0] > size[0] / 2.0, "{size:?} {a:?} under the right thumb");
                assert_eq!(l.zone(c), Zone::Button(a));
                for b in Button::ALL.into_iter().skip(i + 1) {
                    assert!(distance(c, l.button(b)) > 2.0 * r, "{size:?}: {a:?} and {b:?} overlap");
                }
            }
        }
    }
}
