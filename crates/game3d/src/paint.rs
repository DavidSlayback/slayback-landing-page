//! Every colour the game draws, in one place: Rumble Box's (Q75). [`colour`] holds the rules (`game_core::palette`
//! plus the 3D look's own); [`Paints`] hands out one shared material per [`Role`], made on first use, and [`repaint`]
//! recolours them all when a box changes the stage. Things that animate a colour of their own (a door's warning
//! pulse, a lit bomb's blink, the HUD's health bar) take it from [`colour`] too, so nothing keeps a stage's colour after
//! the stage has changed.

use std::collections::HashMap;

use bevy::prelude::*;
use game_core::palette::{self, Fighter, Rgb};

use crate::arena::FLOOR;
use crate::flat::Flat;
use crate::look::{Palette, TILE};

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
    /// Behind the box (the clear colour, not a material).
    Sky,
    /// The floor's and walls' tiles: the first tile colour, checkered with the second.
    Tiles,
    /// The thin lines between tiles.
    Line,
    /// A closed door: darker than the tiles.
    Door,
    /// Door warnings, impact bursts, score popups, bomb fuses and blinks.
    Signal,
    /// The player's floor ring (Q60).
    Ring,
    /// The HUD's health bar.
    Health,
    /// A debris piece, by the part it was: a pale wash toward the floor.
    Debris(Fighter, Paint),
    /// A fighter's part.
    Fighter(Fighter, Paint),
    /// An enemy winding up a blow: its body colour, lit pale.
    Windup(Fighter),
}

/// `who`'s colour for the parts painted `paint`.
fn part(who: Fighter, paint: Paint) -> Rgb {
    let f = palette::fighter(who);
    match paint {
        Paint::Body => f.body,
        Paint::Head => f.head,
        Paint::Fist => f.fist,
        Paint::Trim => f.trim,
    }
}

impl Palette {
    /// Box `n`'s (from 0).
    pub fn nth(n: usize) -> Self {
        Self { stage: palette::stage(n) }
    }
}

/// A `game_core::palette` colour as a Bevy colour.
fn srgb([r, g, b]: Rgb) -> Color {
    Color::srgb(r, g, b)
}

/// `role`'s colour in `palette`'s stage.
pub fn colour(role: Role, palette: &Palette) -> Color {
    let stage = palette.stage;
    srgb(match role {
        Role::Sky => stage.sky,
        Role::Tiles => stage.tiles[0],
        Role::Line => stage.line,
        Role::Door => palette::door(stage),
        Role::Signal => palette::SIGNAL,
        Role::Ring => palette::RING,
        Role::Health => palette::HEALTH,
        Role::Debris(who, paint) => palette::debris(stage, part(who, paint)),
        Role::Fighter(who, paint) => part(who, paint),
        Role::Windup(who) => return srgb(part(who, Paint::Body)).mix(&Color::WHITE, 0.6),
    })
}

/// Paints `m` as `role` in `palette`'s stage: its colour, and the tiles' checker (the tiles start at the floor's
/// corner, one every [`TILE`], as the tile lines do).
fn paint(m: &mut Flat, role: Role, palette: &Palette) {
    m.color = colour(role, palette).into();
    if role == Role::Tiles {
        m.check = srgb(palette.stage.tiles[1]).into();
        m.grid = Vec3::new(-FLOOR.x / 2.0, 0.0, -FLOOR.y / 2.0).extend(TILE);
    }
}

/// The material a role is drawn with: shaded by the light for surfaces and fighters, unlit (full strength, no bloom,
/// research section 7) for lines, marks and glows.
fn material(role: Role, palette: &Palette) -> Flat {
    let mut m = match role {
        Role::Line | Role::Signal | Role::Ring | Role::Windup(_) => Flat::unlit(Color::BLACK),
        _ => Flat::lit(Color::BLACK),
    };
    paint(&mut m, role, palette);
    m
}

/// One shared material per role, so everything wearing it batches and repaints together.
#[derive(Resource, Default)]
pub struct Paints(HashMap<Role, Handle<Flat>>);

impl Paints {
    /// `role`'s material, made on first use in the current stage.
    pub fn get(&mut self, role: Role, palette: &Palette, mats: &mut Assets<Flat>) -> Handle<Flat> {
        self.0.entry(role).or_insert_with(|| mats.add(material(role, palette))).clone()
    }

    /// `role`'s material if something has made it.
    pub fn made(&self, role: Role) -> Option<&Handle<Flat>> {
        self.0.get(&role)
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Paints>()
        .add_systems(Update, repaint.in_set(crate::Frame::Show).run_if(resource_changed::<Palette>));
}

/// Recolours every role's material, and the sky, when a box changes the stage.
fn repaint(paints: Res<Paints>, palette: Res<Palette>, mut mats: ResMut<Assets<Flat>>, mut sky: ResMut<ClearColor>) {
    sky.0 = colour(Role::Sky, &palette);
    for (&role, handle) in &paints.0 {
        if let Some(mut m) = mats.get_mut(handle) {
            paint(&mut m, role, &palette);
        }
    }
}

#[cfg(test)]
mod tests {
    use game_core::enemy::Kind;
    use game_core::palette::STAGES;

    use super::*;

    #[test]
    fn the_player_wears_rumble_boxs_colours_and_debris_pales() {
        let p = Palette::nth(0);
        let player = |paint| colour(Role::Fighter(Fighter::Player, paint), &p).to_srgba();
        let (head, body, fist) = (player(Paint::Head), player(Paint::Body), player(Paint::Fist));
        assert!(head.red > head.blue, "an orange head");
        assert!(body.blue > body.red, "a blue body");
        assert!(fist.red > fist.green && fist.red > fist.blue, "red fists");
        let fodder = Fighter::Enemy(Kind::Fodder);
        let live = colour(Role::Fighter(fodder, Paint::Body), &p).to_srgba();
        let dead = colour(Role::Debris(fodder, Paint::Body), &p).to_srgba();
        assert!(dead.green > live.green, "debris washes toward box 1's green");
    }

    #[test]
    fn a_shared_material_is_made_once_and_repaints_with_the_stage() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Flat>()
            .init_resource::<ClearColor>()
            .insert_resource(Palette::nth(0))
            .add_plugins(plugin);
        app.update();
        let world = app.world_mut();
        let (a, b) = world.resource_scope(|world, mut paints: Mut<Paints>| {
            let palette = *world.resource::<Palette>();
            let mut mats = world.resource_mut::<Assets<Flat>>();
            (paints.get(Role::Tiles, &palette, &mut mats), paints.get(Role::Tiles, &palette, &mut mats))
        });
        assert_eq!(a, b, "one material per role");
        *app.world_mut().resource_mut::<Palette>() = Palette::nth(1);
        app.update();
        let now = app.world().resource::<Assets<Flat>>().get(&a).unwrap().clone();
        assert_eq!(now.color, srgb(STAGES[1].tiles[0]).into(), "repainted in the new stage");
        assert_eq!(now.check, srgb(STAGES[1].tiles[1]).into(), "its checker too");
        assert_eq!(app.world().resource::<ClearColor>().0, srgb(STAGES[1].sky), "and the sky");
    }
}
