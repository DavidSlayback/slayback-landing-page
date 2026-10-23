//! Touch controls (decision Q82, replacing Q78's floating stick, diamond and gestures): twin sticks, fixed and always
//! drawn in the bottom corners. The left one moves. The right one aims and attacks: pushed, the fighter turns that way
//! and punches, again and again while it's held, so it can back away while punching forward. Kick, Grab and Dash are
//! three buttons arced just above the right stick. Tapping the HUD pauses. Engine-free: the game feeds it touches and
//! draws it ([`TouchPad::stick`], [`Layout`], [`TouchPad::held`]).
//!
//! Screen positions are logical pixels with y down, as touches arrive; the directions it reports have y up the screen,
//! as the keys' and sticks' do.

use crate::controls::{Verbs, eight_way};

/// The buttons over the right stick.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Button {
    Kick,
    Grab,
    Dash,
}

impl Button {
    pub const ALL: [Button; 3] = [Button::Kick, Button::Grab, Button::Dash];

    /// Where it sits round the right stick: an angle, radians, with y down (so -π/2 is straight up).
    fn angle(self) -> f32 {
        use std::f32::consts::PI;
        match self {
            Button::Kick => -PI / 2.0,
            Button::Grab => -PI * 0.75,
            Button::Dash => PI,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Button::Kick => "KICK",
            Button::Grab => "GRAB",
            Button::Dash => "DASH",
        }
    }
}

/// The two sticks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stick {
    /// Bottom left: moves.
    Move,
    /// Bottom right: aims and punches.
    Aim,
}

/// What a touch landed on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Zone {
    /// The HUD, top right: pauses.
    Pause,
    /// One of the buttons over the right stick.
    Button(Button),
    /// A stick: the move stick takes the whole left half, the aim stick a thumb landing within [`CATCH`] of it.
    Stick(Stick),
    /// Anywhere else: nothing, in play.
    None,
}

/// The knob's travel from a stick's centre, pixels: the knob is drawn no further, and it is the stick's radius.
pub const RANGE: f32 = 56.0;
/// How far the knob must be pushed before the move stick walks, pixels.
pub const DEAD: f32 = 14.0;
/// How far the aim stick must be pushed before it attacks, pixels: further than the move stick's, so a thumb resting
/// on it doesn't punch.
pub const AIM_DEAD: f32 = 24.0;
/// How near the aim stick's centre a thumb must land to take it, pixels.
pub const CATCH: f32 = RANGE * 2.5;
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

    /// How far from the bottom corners the sticks' centres are.
    fn inset(&self) -> f32 {
        RANGE + 28.0
    }

    /// A stick's centre.
    pub fn stick(&self, s: Stick) -> [f32; 2] {
        let [w, h] = self.size;
        let at = self.inset();
        match s {
            Stick::Move => [at, h - at],
            Stick::Aim => [w - at, h - at],
        }
    }

    /// A button's centre, on an arc just outside the aim stick.
    pub fn button(&self, b: Button) -> [f32; 2] {
        let [cx, cy] = self.stick(Stick::Aim);
        let ring = RANGE + self.radius() + 10.0;
        let (s, c) = b.angle().sin_cos();
        [cx + c * ring, cy + s * ring]
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
        if p[0] < half {
            Zone::Stick(Stick::Move)
        } else if distance(p, self.stick(Stick::Aim)) <= CATCH {
            Zone::Stick(Stick::Aim)
        } else {
            Zone::None
        }
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
    at: [f32; 2],
    /// It does nothing more: it pressed a button, paused, found its stick already held, or left a screen.
    spent: bool,
}

/// One frame of touch input.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Frame {
    /// The move direction (y up the screen): one of eight unit directions or zero.
    pub dir: [f32; 2],
    /// Where the aim stick points (y up the screen, unit length), while it's pushed past [`AIM_DEAD`]; it punches
    /// then too.
    pub aim: Option<[f32; 2]>,
    /// Punch, kick, grab, dash and pause: pressed this frame, or for the punch, aimed.
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

    pub fn down(&mut self, id: u64, p: [f32; 2]) {
        let zone = self.layout.zone(p);
        self.pending.landed.get_or_insert(p);
        let spent = match zone {
            Zone::Pause => {
                self.pending.verbs.pause = true;
                true
            }
            Zone::Button(b) => {
                let v = &mut self.pending.verbs;
                match b {
                    Button::Kick => v.kick = true,
                    Button::Grab => v.grab = true,
                    Button::Dash => v.dash = true,
                }
                true
            }
            // One thumb to a stick: a second finger on it is ignored.
            Zone::Stick(s) => self.held_by(s).is_some(),
            Zone::None => true,
        };
        self.fingers.retain(|f| f.id != id);
        self.fingers.push(Finger { id, zone, at: p, spent });
    }

    pub fn moved(&mut self, id: u64, p: [f32; 2]) {
        if let Some(f) = self.fingers.iter_mut().find(|f| f.id == id) {
            f.at = p;
        }
    }

    pub fn up(&mut self, id: u64) {
        self.fingers.retain(|f| f.id != id);
    }

    /// A finger the browser took away (a system gesture): it does nothing more.
    pub fn cancel(&mut self, id: u64) {
        self.up(id);
    }

    /// Forgets every finger, so a touch that started or resumed the game doesn't also walk or punch.
    pub fn clear(&mut self) {
        for f in &mut self.fingers {
            f.spent = true;
        }
    }

    /// This frame's input; what was pressed is cleared for the next frame.
    pub fn frame(&mut self) -> Frame {
        let mut out = std::mem::take(&mut self.pending);
        if let Some(d) = self.push(Stick::Move)
            && d[0].hypot(d[1]) >= DEAD
        {
            out.dir = eight_way(d);
        }
        if let Some(d) = self.push(Stick::Aim) {
            let len = d[0].hypot(d[1]);
            if len >= AIM_DEAD {
                out.aim = Some([d[0] / len, d[1] / len]);
                out.verbs.punch = true;
            }
        }
        out
    }

    /// The finger holding stick `s`, if one is.
    fn held_by(&self, s: Stick) -> Option<&Finger> {
        self.fingers.iter().find(|f| f.zone == Zone::Stick(s) && !f.spent)
    }

    /// How far stick `s` is pushed, y up the screen, while a thumb holds it.
    fn push(&self, s: Stick) -> Option<[f32; 2]> {
        let [cx, cy] = self.layout.stick(s);
        self.held_by(s).map(|f| [f.at[0] - cx, cy - f.at[1]])
    }

    /// Stick `s` to draw: its centre, the knob's place (within [`RANGE`] of it), and whether a thumb holds it.
    pub fn stick(&self, s: Stick) -> ([f32; 2], [f32; 2], bool) {
        let o = self.layout.stick(s);
        match self.held_by(s) {
            Some(f) => {
                let d = [f.at[0] - o[0], f.at[1] - o[1]];
                let k = (RANGE / d[0].hypot(d[1]).max(1e-3)).min(1.0);
                (o, [o[0] + d[0] * k, o[1] + d[1] * k], true)
            }
            None => (o, o, false),
        }
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

    fn pad() -> TouchPad {
        TouchPad::new(SIZE)
    }

    fn plus(p: [f32; 2], d: [f32; 2]) -> [f32; 2] {
        [p[0] + d[0], p[1] + d[1]]
    }

    #[test]
    fn the_move_stick_points_from_its_fixed_centre_and_up_the_screen() {
        let mut p = pad();
        let c = p.layout.stick(Stick::Move);
        assert_eq!(p.layout.zone([300.0, 100.0]), Zone::Stick(Stick::Move), "the whole left half takes it");
        p.down(1, plus(c, [DEAD - 2.0, 0.0]));
        assert_eq!(p.frame().dir, [0.0, 0.0], "a thumb near the middle is inside the dead zone");
        p.moved(1, plus(c, [0.0, -30.0]));
        assert_eq!(p.frame().dir, [0.0, 1.0], "pushed up the screen it walks up it");
        p.moved(1, plus(c, [200.0, 0.0]));
        assert_eq!(p.frame().dir, [1.0, 0.0], "held, it keeps walking");
        assert_eq!(p.stick(Stick::Move), (c, plus(c, [RANGE, 0.0]), true), "the knob stops at the stick's edge");
        p.up(1);
        let f = p.frame();
        assert_eq!((f.dir, f.aim, f.verbs), ([0.0, 0.0], None, Verbs::default()), "letting go does nothing else");
        assert_eq!(p.stick(Stick::Move), (c, c, false), "and the knob is back in the middle, still drawn");
    }

    #[test]
    fn the_aim_stick_punches_its_way_every_frame_it_is_pushed_while_the_other_thumb_walks() {
        let mut p = pad();
        let (m, a) = (p.layout.stick(Stick::Move), p.layout.stick(Stick::Aim));
        p.down(1, m);
        p.moved(1, plus(m, [-40.0, 0.0]));
        p.down(2, a);
        p.moved(2, plus(a, [AIM_DEAD - 4.0, 0.0]));
        let f = p.frame();
        assert!(f.aim.is_none() && !f.verbs.punch, "resting on it doesn't punch");
        p.moved(2, plus(a, [0.0, -40.0]));
        for _ in 0..3 {
            let f = p.frame();
            assert_eq!(f.dir, [-1.0, 0.0], "walking left");
            assert_eq!(f.aim, Some([0.0, 1.0]), "while aiming up the screen");
            assert!(f.verbs.punch, "and punching, every frame it's held");
        }
        p.moved(2, plus(a, [30.0, 10.0]));
        let [x, y] = p.frame().aim.unwrap();
        assert!((x - 0.9487).abs() < 1e-3 && (y + 0.3162).abs() < 1e-3, "aim isn't snapped to eight ways: {x} {y}");
        p.up(2);
        let f = p.frame();
        assert!(f.aim.is_none() && !f.verbs.punch);
    }

    #[test]
    fn a_button_presses_once_the_frame_it_is_touched() {
        let mut p = pad();
        for b in Button::ALL {
            p.down(2, p.layout.button(b));
            assert!(p.held(b));
            let v = p.frame().verbs;
            assert_eq!([v.punch, v.kick, v.grab, v.dash].iter().filter(|&&x| x).count(), 1, "{b:?}");
            assert_eq!(p.frame().verbs, Verbs::default(), "{b:?}: once, not while held");
            p.up(2);
        }
    }

    #[test]
    fn the_hud_pauses_far_taps_do_nothing_and_any_touch_is_a_touch() {
        let mut p = pad();
        p.down(1, [SIZE[0] - 60.0, 30.0]);
        let f = p.frame();
        assert!(f.verbs.pause && f.landed == Some([SIZE[0] - 60.0, 30.0]));
        let far = [SIZE[0] / 2.0 + 20.0, 90.0];
        assert_eq!(p.layout.zone(far), Zone::None);
        p.down(2, far);
        let f = p.frame();
        assert_eq!((f.verbs, f.aim, f.landed), (Verbs::default(), None, Some(far)));
        assert_eq!(p.frame().landed, None, "only the frame it lands");
    }

    #[test]
    fn a_second_thumb_on_a_held_stick_and_cleared_fingers_do_nothing() {
        let mut p = pad();
        let a = p.layout.stick(Stick::Aim);
        p.down(1, plus(a, [-40.0, 0.0]));
        p.down(2, plus(a, [0.0, -40.0]));
        assert_eq!(p.frame().aim, Some([-1.0, 0.0]), "the first thumb keeps it");
        p.clear();
        assert_eq!(p.frame(), Frame::default(), "the touch that left a screen neither aims nor punches");
    }

    #[test]
    fn the_sticks_and_buttons_fit_on_screen_without_overlapping() {
        for size in [[800.0, 380.0], [568.0, 320.0], [1280.0, 720.0], [380.0, 800.0]] {
            let l = Layout { size };
            let r = l.radius();
            let (m, a) = (l.stick(Stick::Move), l.stick(Stick::Aim));
            for c in [m, a] {
                assert!(c[0] - RANGE >= 0.0 && c[0] + RANGE <= size[0] && c[1] + RANGE <= size[1], "{size:?}");
            }
            assert!(distance(m, a) > 2.0 * RANGE + 2.0 * r, "{size:?}: the sticks are apart");
            assert_eq!(l.zone(a), Zone::Stick(Stick::Aim));
            for (i, b) in Button::ALL.into_iter().enumerate() {
                let c = l.button(b);
                assert!(c[0] - r >= 0.0 && c[0] + r <= size[0] && c[1] - r >= HUD_BAND, "{size:?} {b:?}");
                assert!(c[0] > size[0] / 2.0, "{size:?} {b:?} under the right thumb");
                assert!(distance(c, a) > RANGE + r, "{size:?} {b:?} clear of the stick");
                assert!(distance(c, m) > RANGE + r, "{size:?} {b:?} clear of the move stick");
                assert_eq!(l.zone(c), Zone::Button(b));
                for o in Button::ALL.into_iter().skip(i + 1) {
                    assert!(distance(c, l.button(o)) > 2.0 * r, "{size:?}: {b:?} and {o:?} overlap");
                }
            }
        }
    }
}
