//! The action mapping both games share (D11): which gamepad button does what, and how sticks, d-pads and keys turn
//! into a move direction. Engine-free, so each game reads its devices its own way and maps them identically.
//!
//! | Action | Keyboard | Mouse | Gamepad |
//! |---|---|---|---|
//! | Move | WASD, arrows (2.5D: A / D, left and right) | | left stick, d-pad |
//! | Dash | Space, Shift | | A (south), right trigger |
//! | Punch | J | left | X (west) |
//! | Kick | K | right | Y (north) |
//! | Grab / set down | L | middle | B (east) |
//! | Pause | Esc | | Start |
//! | Start / again | Enter, Space | left | Start, A |
//! | Sound (paused) | M | | Select |

/// Stick travel below this is ignored (worn sticks drift).
pub const DEADZONE: f32 = 0.3;

/// The verbs pressed this frame (all "just pressed").
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Verbs {
    pub dash: bool,
    pub punch: bool,
    pub kick: bool,
    pub grab: bool,
    pub pause: bool,
    pub begin: bool,
    pub sound: bool,
}

impl Verbs {
    /// Two devices at once: either one pressing counts.
    pub fn or(self, o: Verbs) -> Verbs {
        Verbs {
            dash: self.dash || o.dash,
            punch: self.punch || o.punch,
            kick: self.kick || o.kick,
            grab: self.grab || o.grab,
            pause: self.pause || o.pause,
            begin: self.begin || o.begin,
            sound: self.sound || o.sound,
        }
    }
}

/// One gamepad's state, as the mapping needs it. Axes are x right, y up.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pad {
    pub stick: [f32; 2],
    pub dpad: [f32; 2],
    /// Buttons just pressed this frame.
    pub south: bool,
    pub east: bool,
    pub north: bool,
    pub west: bool,
    pub right_trigger: bool,
    pub start: bool,
    pub select: bool,
}

impl Pad {
    /// The verbs this pad pressed (the table above).
    pub fn verbs(&self) -> Verbs {
        Verbs {
            dash: self.south || self.right_trigger,
            punch: self.west,
            kick: self.north,
            grab: self.east,
            pause: self.start,
            begin: self.start || self.south,
            sound: self.select,
        }
    }

    /// The 2.5D game's run axis: the d-pad wins, else the stick past the deadzone; -1, 0 or 1.
    pub fn axis(&self) -> f32 {
        if self.dpad[0] != 0.0 {
            self.dpad[0].signum()
        } else if self.stick[0].abs() >= DEADZONE {
            self.stick[0].signum()
        } else {
            0.0
        }
    }

    /// The 3D game's move direction: the d-pad wins, else the stick past the deadzone, snapped to eight ways.
    pub fn direction(&self) -> [f32; 2] {
        if self.dpad != [0.0, 0.0] {
            eight_way(self.dpad)
        } else if self.stick[0].hypot(self.stick[1]) >= DEADZONE {
            eight_way(self.stick)
        } else {
            [0.0, 0.0]
        }
    }
}

/// The move direction from held keys: unit length (diagonals too) or zero; opposite keys cancel.
pub fn keys_direction(right: bool, left: bool, up: bool, down: bool) -> [f32; 2] {
    let axis = |plus: bool, minus: bool| (plus as i8 - minus as i8) as f32;
    eight_way([axis(right, left), axis(up, down)])
}

/// The nearest of the eight compass directions to `v`, at unit length; zero stays zero.
pub fn eight_way(v: [f32; 2]) -> [f32; 2] {
    if v == [0.0, 0.0] {
        return v;
    }
    let step = std::f32::consts::FRAC_PI_4;
    let a = (v[1].atan2(v[0]) / step).round() * step;
    // Exact zeros on the axes, so straight moves stay straight.
    let snap = |c: f32| if c.abs() < 1e-6 { 0.0 } else { c };
    [snap(a.cos()), snap(a.sin())]
}

/// The first device that is pushing a direction wins (no adding two devices into a faster diagonal).
pub fn first_direction(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    if a != [0.0, 0.0] { a } else { b }
}

#[cfg(test)]
mod tests {
    use super::*;

    const D: f32 = std::f32::consts::FRAC_1_SQRT_2;

    fn close(a: [f32; 2], b: [f32; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-5 && (a[1] - b[1]).abs() < 1e-5
    }

    #[test]
    fn the_stick_has_a_deadzone_and_the_dpad_wins() {
        assert_eq!(Pad { stick: [0.2, 0.0], ..Default::default() }.axis(), 0.0);
        assert_eq!(Pad { stick: [-0.7, 0.0], ..Default::default() }.axis(), -1.0);
        assert_eq!(Pad { stick: [-0.7, 0.0], dpad: [1.0, 0.0], ..Default::default() }.axis(), 1.0);
        assert_eq!(Pad { stick: [0.15, 0.2], ..Default::default() }.direction(), [0.0, 0.0]);
        assert_eq!(Pad { stick: [-0.7, 0.1], dpad: [0.0, 1.0], ..Default::default() }.direction(), [0.0, 1.0]);
    }

    #[test]
    fn every_verb_has_a_button() {
        let v = |p: Pad| p.verbs();
        assert!(v(Pad { south: true, ..Default::default() }).dash);
        assert!(v(Pad { right_trigger: true, ..Default::default() }).dash);
        assert!(v(Pad { west: true, ..Default::default() }).punch);
        assert!(v(Pad { north: true, ..Default::default() }).kick);
        assert!(v(Pad { east: true, ..Default::default() }).grab);
        assert!(
            v(Pad { start: true, ..Default::default() }).pause && v(Pad { start: true, ..Default::default() }).begin
        );
        assert!(v(Pad { select: true, ..Default::default() }).sound);
    }

    #[test]
    fn two_devices_press_together() {
        let keys = Verbs { punch: true, ..Default::default() };
        let pad = Verbs { dash: true, ..Default::default() };
        let both = keys.or(pad);
        assert!(both.punch && both.dash && !both.kick);
        assert_eq!(first_direction([0.0, 0.0], [1.0, 0.0]), [1.0, 0.0]);
        assert_eq!(first_direction([0.0, -1.0], [1.0, 0.0]), [0.0, -1.0]);
    }

    #[test]
    fn keys_give_eight_unit_directions_and_opposites_cancel() {
        assert_eq!(keys_direction(true, false, false, false), [1.0, 0.0]);
        assert_eq!(keys_direction(false, false, false, true), [0.0, -1.0]);
        assert!(close(keys_direction(true, false, true, false), [D, D]), "diagonals are unit length, not faster");
        assert_eq!(keys_direction(true, true, false, false), [0.0, 0.0]);
        assert_eq!(keys_direction(true, true, true, false), [0.0, 1.0]);
    }

    #[test]
    fn a_stick_snaps_to_the_nearest_of_eight_directions() {
        assert_eq!(eight_way([0.9, 0.3]), [1.0, 0.0], "18 degrees off the axis snaps to it");
        assert!(close(eight_way([0.6, 0.5]), [D, D]), "40 degrees snaps to the diagonal");
        assert!(close(eight_way([-0.2, -0.9]), [0.0, -1.0]));
        assert!(close(eight_way([-0.5, -0.4]), [-D, -D]));
        assert_eq!(eight_way([0.0, 0.0]), [0.0, 0.0]);
        assert!(close(Pad { stick: [0.35, -0.4], ..Default::default() }.direction(), [D, -D]));
    }
}
