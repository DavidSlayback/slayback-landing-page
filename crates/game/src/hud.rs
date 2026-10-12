//! Health bars floating above every fighter.

use bevy::prelude::*;

use crate::fighter::{Fighter, Hp};

#[derive(Component)]
struct BarFill;

const WIDTH: f32 = 40.0;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (attach_bars, update_bars));
}

fn attach_bars(new: Query<(Entity, &Fighter), Added<Fighter>>, mut commands: Commands) {
    for (entity, fighter) in &new {
        let y = fighter.half.y + 10.0;
        commands.entity(entity).with_children(|c| {
            c.spawn((Sprite::from_color(Color::srgb(0.12, 0.12, 0.12), Vec2::new(WIDTH, 4.0)), Transform::from_xyz(0.0, y, 0.7)));
            c.spawn((BarFill, Sprite::from_color(Color::srgb(0.65, 1.0, 0.0), Vec2::new(WIDTH, 4.0)), Transform::from_xyz(0.0, y, 0.8)));
        });
    }
}

fn update_bars(fighters: Query<(&Hp, &Children)>, mut fills: Query<&mut Transform, With<BarFill>>) {
    for (hp, children) in &fighters {
        let frac = (hp.0.hp / hp.0.max).clamp(0.0, 1.0);
        for child in children.iter() {
            if let Ok(mut t) = fills.get_mut(child) {
                // shrink from the right, keeping the left edge fixed
                t.scale.x = frac;
                t.translation.x = -WIDTH * (1.0 - frac) / 2.0;
            }
        }
    }
}
