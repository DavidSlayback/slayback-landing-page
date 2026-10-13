//! Wall doors: enemies enter through doors in the side walls, which warn before they open.

/// How long a door shows its warning before it opens, seconds.
pub const WARNING: f32 = 0.5;
/// How long it stays open (the enemy steps through), seconds.
pub const OPEN: f32 = 0.4;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Door {
    #[default]
    Closed,
    /// Telegraphing an entry; seconds left until it opens.
    Warning(f32),
    /// Open; seconds left until it closes.
    Open(f32),
}

impl Door {
    /// Asks the door to let an enemy in. Returns false if it is already busy.
    pub fn request(&mut self) -> bool {
        if *self != Door::Closed {
            return false;
        }
        *self = Door::Warning(WARNING);
        true
    }

    /// Advances time. Returns true on the one frame the door opens: spawn the enemy then.
    pub fn tick(&mut self, dt: f32) -> bool {
        match *self {
            Door::Closed => false,
            Door::Warning(t) if t - dt > 0.0 => {
                *self = Door::Warning(t - dt);
                false
            }
            Door::Warning(_) => {
                *self = Door::Open(OPEN);
                true
            }
            Door::Open(t) => {
                *self = if t - dt > 0.0 { Door::Open(t - dt) } else { Door::Closed };
                false
            }
        }
    }

    pub fn warning(&self) -> bool {
        matches!(self, Door::Warning(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    #[test]
    fn a_door_warns_for_half_a_second_then_lets_one_enemy_in_and_closes() {
        let mut door = Door::default();
        assert!(door.request());
        let mut warned = 0.0;
        let mut spawns = 0;
        for _ in 0..120 {
            if door.warning() {
                warned += DT;
            }
            if door.tick(DT) {
                spawns += 1;
                assert!(warned >= WARNING - DT, "opened after only {warned} s of warning");
            }
        }
        assert_eq!(spawns, 1);
        assert_eq!(door, Door::Closed);
    }

    #[test]
    fn a_busy_door_refuses_another_enemy_until_it_has_closed() {
        let mut door = Door::default();
        assert!(door.request());
        assert!(!door.request());
        for _ in 0..((WARNING + OPEN) / DT) as usize + 2 {
            door.tick(DT);
        }
        assert!(door.request());
    }
}
