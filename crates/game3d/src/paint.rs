//! Every colour the game takes from the page's scheme, in one place. [`colour`] holds the rules (`game_core::palette`
//! plus the 3D look's own, Q50/Q66); [`Paints`] hands out one shared material per [`Role`], made on first use, and
//! [`repaint`] recolours them all when a box rotates the scheme (Q31). Things that animate a colour of their own (a
//! door's warning pulse, a lit bomb's blink, the HUD's health bar) take it from [`colour`] too, so nothing keeps a
//! scheme's colour after the scheme has changed.

use std::collections::HashMap;

use bevy::prelude::*;
use game_core::palette::{self, Fighter, Rgb};

use crate::flat::Flat;
use crate::look::{Palette, srgb};

/// Which of a fighter's colours a part of its figure wears.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Paint {
    Body,
    Head,
    Fist,
    Trim,
}

/// What something is coloured as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Floor,
    Wall,
    /// A closed door: a darker arena tone.
    Door,
    /// Door warnings, impact bursts, score popups, bomb fuses and blinks: the scheme's signal accent.
    Signal,
    /// The player's floor ring and health bar (Q60): the main accent.
    Ring,
    /// One of the debris shades (the arena's muted tone), by part.
    Debris(usize),
    /// A fighter's part.
    Fighter(Fighter, Paint),
    /// An enemy winding up a blow: its colour, lit pale.
    Windup(Fighter),
}

/// `role`'s colour in `palette`'s scheme. The player reads by contrast between its parts, as Rumble Box's does
/// (orange head, blue body, red fists): a pale head, fists in the scheme's accent furthest from the body, dark legs.
/// An enemy wears one vivid colour.
pub fn colour(role: Role, palette: &Palette) -> Color {
    let scheme = palette.scheme;
    let arena = palette::arena_tone(scheme);
    match role {
        Role::Floor => srgb(arena.floor),
        Role::Wall => srgb(arena.wall),
        Role::Door => srgb(arena.door),
        Role::Signal => srgb(palette::signal(scheme)),
        Role::Ring => srgb(palette::ring(scheme)),
        Role::Debris(i) => srgb(palette::debris_tone(scheme, i)),
        Role::Windup(who) => srgb(palette::fighter_colour(scheme, who).body).mix(&Color::WHITE, 0.6),
        Role::Fighter(who, paint) => {
            let f = palette::fighter_colour(scheme, who);
            match (who, paint) {
                (Fighter::Player, Paint::Head) => srgb(f.body).mix(&Color::WHITE, 0.6),
                (Fighter::Player, Paint::Fist) => srgb(furthest(scheme.accents, f.body)),
                (Fighter::Player, Paint::Trim) => srgb(f.trim),
                _ => srgb(f.body),
            }
        }
    }
}

/// The accent furthest from `from` (by squared sRGB distance).
fn furthest(accents: [Rgb; 4], from: Rgb) -> Rgb {
    let d = |c: &Rgb| c.iter().zip(from).map(|(a, b)| (a - b) * (a - b)).sum::<f32>();
    accents.into_iter().max_by(|a, b| d(a).total_cmp(&d(b))).unwrap_or(from)
}

/// The material a role is drawn with, in `colour`: shaded by the light for surfaces and fighters, unlit (full
/// strength, no bloom, research section 7) for marks and glows.
fn material(role: Role, colour: Color) -> Flat {
    match role {
        Role::Signal | Role::Ring | Role::Windup(_) => Flat::unlit(colour),
        _ => Flat::lit(colour),
    }
}

/// One shared material per role, so everything wearing it batches and repaints together.
#[derive(Resource, Default)]
pub struct Paints(HashMap<Role, Handle<Flat>>);

impl Paints {
    /// `role`'s material, made on first use in the current scheme.
    pub fn get(&mut self, role: Role, palette: &Palette, mats: &mut Assets<Flat>) -> Handle<Flat> {
        self.0.entry(role).or_insert_with(|| mats.add(material(role, colour(role, palette)))).clone()
    }

    /// `role`'s material if something has made it.
    pub fn made(&self, role: Role) -> Option<&Handle<Flat>> {
        self.0.get(&role)
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Paints>().add_systems(Update, repaint.run_if(resource_changed::<Palette>));
}

/// Recolours every role's material when a box changes the scheme.
fn repaint(paints: Res<Paints>, palette: Res<Palette>, mut mats: ResMut<Assets<Flat>>) {
    for (&role, handle) in &paints.0 {
        if let Some(mut m) = mats.get_mut(handle) {
            m.color = colour(role, &palette).into();
        }
    }
}

#[cfg(test)]
mod tests {
    use game_core::enemy::Kind;
    use game_core::scheme::SCHEMES;

    use super::*;

    #[test]
    fn the_players_parts_contrast_and_an_enemy_wears_one_colour() {
        for i in 0..SCHEMES.len() {
            let p = Palette::nth(i);
            let player = |paint| colour(Role::Fighter(Fighter::Player, paint), &p);
            assert_ne!(player(Paint::Fist), player(Paint::Body), "{}: fists stand out", p.scheme.key);
            assert_ne!(player(Paint::Head), player(Paint::Body), "{}: so does the head", p.scheme.key);
            let fodder = |paint| colour(Role::Fighter(Fighter::Enemy(Kind::Fodder), paint), &p);
            assert!([Paint::Head, Paint::Fist, Paint::Trim].iter().all(|&q| fodder(q) == fodder(Paint::Body)));
        }
    }

    #[test]
    fn a_shared_material_is_made_once_and_repaints_with_the_scheme() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Flat>()
            .insert_resource(Palette::nth(0))
            .add_plugins(plugin);
        app.update();
        let world = app.world_mut();
        let (a, b) = world.resource_scope(|world, mut paints: Mut<Paints>| {
            let palette = *world.resource::<Palette>();
            let mut mats = world.resource_mut::<Assets<Flat>>();
            (paints.get(Role::Signal, &palette, &mut mats), paints.get(Role::Signal, &palette, &mut mats))
        });
        assert_eq!(a, b, "one material per role");
        *app.world_mut().resource_mut::<Palette>() = Palette::nth(1);
        app.update();
        let now = app.world().resource::<Assets<Flat>>().get(&a).unwrap().color;
        assert_eq!(now, colour(Role::Signal, &Palette::nth(1)).into(), "repainted in the new scheme");
    }
}
