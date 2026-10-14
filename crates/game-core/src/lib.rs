//! Engine-free rules for the escape-the-box game.
//!
//! Everything here is plain data and functions so it can be tested natively without Bevy; `crates/game3d` wires it into
//! ECS systems. Lengths are metres, speeds m/s and times seconds throughout.

pub mod aim;
pub mod bomb;
pub mod climb;
pub mod combat;
pub mod controls;
pub mod debris;
pub mod doors;
pub mod enemy;
pub mod grab;
pub mod impact;
pub mod palette;
pub mod plane;
pub mod quality;
pub mod run;
pub mod scheme;
pub mod score;
pub mod steer;

/// The rules once spoke the retired 2.5D game's world units (32 to the metre); these pin the metre values to the old
/// numbers over 32, so the move to metres changed nothing.
#[cfg(test)]
mod metres {
    use crate::combat::Move;
    use crate::grab::{Held, flat, impact};

    const UNITS_PER_METRE: f32 = 32.0;

    fn close(metres: f32, units: f32) -> bool {
        (metres - units / UNITS_PER_METRE).abs() < 1e-5
    }

    #[test]
    fn moves_reach_land_and_launch_as_the_old_units_over_32() {
        let old = [
            (Move::Punch, 34.0, 10.0, (520.0, 260.0)),
            (Move::Kick, 46.0, -14.0, (900.0, 420.0)),
            (Move::DropKick, 30.0, -6.0, (1000.0, 360.0)),
            (Move::Smash, 44.0, 16.0, (750.0, 480.0)),
            (Move::Slash, 30.0, 6.0, (450.0, 200.0)),
        ];
        for (mv, reach, offset_y, knockback) in old {
            let s = mv.spec();
            assert!(close(s.reach, reach) && close(s.offset_y, offset_y), "{mv:?}");
            assert!(close(s.knockback.0, knockback.0) && close(s.knockback.1, knockback.1), "{mv:?}");
            // The launch keeps the old 3D scale, 0.35 and 0.4 over 32 on the old units.
            let (across, up) = crate::aim::launch(mv, 0.0);
            assert!((across[0] - knockback.0 * (0.35 / 32.0)).abs() < 1e-5, "{mv:?} across {across:?}");
            assert!((up - knockback.1 * (0.4 / 32.0)).abs() < 1e-5, "{mv:?} up {up}");
        }
        let punch = crate::aim::Cone::of(Move::Punch, [0.0, 0.0], 0.0, 0.35);
        assert!(close(punch.reach - 0.35, 34.0), "punch reach {}", punch.reach);
    }

    #[test]
    fn grabs_throws_and_bursts_keep_the_old_units_over_32() {
        assert!(close(flat::REACH, 30.0));
        for (held, knockback) in
            [(Held::Enemy, (700.0, 380.0)), (Held::Debris, (450.0, 260.0)), (Held::Bomb, (250.0, 150.0))]
        {
            let k = impact(held).knockback;
            assert!(close(k.0, knockback.0) && close(k.1, knockback.1), "{held:?}");
        }
        let (across, up) = flat::knock(Held::Enemy, 0.0);
        assert!((across[0] - 700.0 * (0.35 / 32.0)).abs() < 1e-5 && (up - 380.0 * (0.4 / 32.0)).abs() < 1e-5);
        assert!(close(crate::impact::BLAST_BURST, 108.0));
        for (mv, base) in
            [(Move::Punch, 18.0), (Move::Slash, 18.0), (Move::Kick, 26.0), (Move::DropKick, 30.0), (Move::Smash, 32.0)]
        {
            assert!(close(crate::impact::burst_size(mv, false), base), "{mv:?}");
            assert!(close(crate::impact::burst_size(mv, true), base * 1.8), "{mv:?} finisher");
        }
    }

    #[test]
    fn enemy_ranges_the_blast_and_the_rims_keep_the_old_numbers() {
        use crate::enemy::{FODDER, NINJA, NinjaBrain, THUG};
        assert!(close(FODDER.range, 52.0) && close(THUG.range, 66.0) && close(NINJA.range, 44.0));
        assert!(close(NinjaBrain::AVOID, 170.0));
        // The blast at 1 m: the old rule fed 1 m as the same share of its 140-unit radius, launching at 1100 units/s
        // falling off to a third, a level body lifted at 0.35 of that, all scaled as a blow's.
        let k = 1.0 - (140.0 / 3.0) / 140.0;
        let speed = 1100.0 * (1.0 / 3.0 + 2.0 / 3.0 * k);
        let (across, up) = crate::bomb::space::push([1.0, 0.0], 0.0).unwrap();
        assert!((across[0] - speed * (0.35 / 32.0)).abs() < 1e-5, "{across:?}");
        assert!((up - 0.35 * speed * (0.4 / 32.0)).abs() < 1e-5, "{up}");
        let (across, up) = crate::bomb::space::push([0.0, 0.0], 0.0).unwrap();
        assert!(across == [0.0, 0.0] && (up - 1.35 * 1100.0 * (0.4 / 32.0)).abs() < 1e-4, "dead centre: {up}");
        // The rims: 1.5 m scaled by the old curve (140 units, +45 a box, to 600), capped at 3 m.
        for n in 1..=12 {
            let old = (1.5 * (140.0 + 45.0 * (n - 1) as f32).min(600.0) / 140.0).min(3.0);
            assert!((crate::run::box_spec(n).rim - old).abs() < 1e-5, "box {n}");
        }
    }
}
