//! Bomb-heads in 3D (rules: `game_core::bomb`, its `space` module for the 3D blast). From box 2 some enemies carry a
//! bomb for a head (the box spec's share, Q32). When one is defeated its head drops free as a lit bomb while the rest
//! bursts into debris as usual. The bomb blinks, faster as its 1.5 s fuse runs out, then blows: everyone within 3 m
//! (enemies and the player) is hurt and launched, more the nearer they are; other lit bombs in range go off a moment
//! later; and debris in range thaws and is flung, never deleted. A lit bomb is `Grabbable(Held::Bomb)`: it can be
//! carried and spin-thrown (its fuse burns on in hand), and a thrown one stops at the first enemy it hits.

use avian3d::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use game_core::bomb::space::{damage, push};
use game_core::bomb::{Fuse, HITSTOP};
use game_core::combat::HitOutcome;
use game_core::debris::solid::Settle;
use game_core::grab::Held;

use crate::debris::Piece;
use crate::fighter::{Combat, Defeated, FLASH, Flash, HitStopClock, Hp, Layer, Team};
use crate::flow::Phase;
use crate::grab::{Carried, Grabbable};
use crate::{on_floor, plane};

/// This enemy's head is a bomb.
#[derive(Component, Clone, Copy, Debug)]
pub struct BombHead;

/// A lit bomb, burning down.
#[derive(Component, Debug)]
pub struct LitBomb {
    pub fuse: Fuse,
}

/// A bomb went off here (the shake, the burst and the sound listen for it).
#[derive(Message, Clone, Copy, Debug)]
pub struct Exploded {
    pub at: Vec3,
}

/// A bomb's radius, metres: a little bigger than the fodder's ball head (0.19 m), so it reads as a bomb.
pub const RADIUS: f32 = 0.22;
/// How long a blast stuns those it hurts, seconds (as in the 2.5D game).
const STUN: f32 = 0.4;

pub fn plugin(app: &mut App) {
    // Fuses burn on the game clock, which stops outside play anyway; the gate keeps blasts out of the screens too.
    app.add_message::<Exploded>().add_systems(Update, burn.before(Combat).run_if(in_state(Phase::Playing)));
    #[cfg(feature = "render")]
    app.add_plugins(view::plugin);
}

/// A lit bomb at `at`, moving at `velocity`: a small ball on the debris layer that can be picked up.
pub fn lit_bomb(at: Vec3, velocity: Vec3) -> impl Bundle {
    (
        LitBomb { fuse: Fuse::default() },
        Grabbable(Held::Bomb),
        Name::new("bomb"),
        RigidBody::Dynamic,
        Collider::sphere(RADIUS),
        CollisionLayers::new(Layer::Debris, [Layer::World, Layer::Fighter, Layer::Debris]),
        Friction::new(0.8),
        Restitution::new(0.2),
        // So it stops rolling soon after it lands, near where it fell.
        LinearDamping(0.4),
        AngularDamping(2.0),
        Transform::from_translation(at),
        LinearVelocity(velocity),
    )
}

/// What a blast sets off besides the push: a freeze, defeats, and the shake and burst (via `Exploded`).
#[derive(SystemParam)]
struct BlastEffects<'w> {
    stop: ResMut<'w, HitStopClock>,
    defeated: MessageWriter<'w, Defeated>,
    exploded: MessageWriter<'w, Exploded>,
}

/// Fighters a blast can reach.
type Fighters = (
    Entity,
    &'static Team,
    &'static Transform,
    &'static mut Hp,
    &'static mut LinearVelocity,
    &'static mut Flash,
    Has<Carried>,
);
/// Lit bombs: their fuses, and the push a blast gives them.
type Bombs = (Entity, &'static mut LitBomb, &'static Transform, &'static mut LinearVelocity, Has<Carried>);
/// Debris pieces a blast can knock loose (not one in the player's hands).
type Pieces = (Entity, &'static Transform, &'static mut Piece, &'static RigidBody, &'static mut LinearVelocity);

/// Burns every fuse down; each bomb that goes off blasts everything in range.
#[allow(clippy::type_complexity)]
fn burn(
    mut commands: Commands,
    time: Res<Time>,
    mut bombs: Query<Bombs, Without<Team>>,
    mut fighters: Query<Fighters>,
    mut pieces: Query<Pieces, (Without<Team>, Without<LitBomb>, Without<Carried>)>,
    mut out: BlastEffects,
) {
    let mut blasts = Vec::new();
    for (e, mut lit, at, ..) in &mut bombs {
        if lit.fuse.tick(time.delta_secs()) {
            blasts.push((e, at.translation));
            commands.entity(e).despawn();
        }
    }
    let now = time.elapsed_secs();
    for &(_, centre) in &blasts {
        out.exploded.write(Exploded { at: centre });
        out.stop.0.trigger(HITSTOP);
        let launch = |at: Vec3| {
            let offset = at - centre;
            push(plane(offset), offset.y).map(|(across, up)| on_floor(across).with_y(up))
        };
        for (e, team, t, mut hp, mut v, mut flash, carried) in &mut fighters {
            let (Some(hurt), Some(fling)) = (damage(t.translation.distance(centre)), launch(t.translation)) else {
                continue;
            };
            if hp.0.defeated() {
                continue;
            }
            flash.0 = FLASH;
            hp.0.stun = hp.0.stun.max(STUN);
            if !carried {
                v.0 = fling;
            }
            if hp.0.hurt(hurt) == HitOutcome::Defeated {
                out.defeated.write(Defeated { entity: e, team: *team, at: t.translation });
            }
        }
        // Other lit bombs in range go off a moment later, and are flung (unless in the player's hands).
        for (e, mut lit, t, mut v, carried) in &mut bombs {
            if blasts.iter().any(|(b, _)| *b == e) {
                continue;
            }
            let Some(fling) = launch(t.translation) else { continue };
            lit.fuse.chain();
            if !carried {
                v.0 = fling;
            }
        }
        // Debris is knocked loose and flung, never deleted: the pile never loses progress outright. A thawed piece
        // starts again as a fresh loose one; the nearer the blast, the newer it counts, so if the blast thaws more than
        // the moving cap allows, `debris` freezes the farthest again first.
        for (e, t, mut piece, body, mut v) in &mut pieces {
            let Some(fling) = launch(t.translation) else { continue };
            if *body != RigidBody::Dynamic {
                commands.entity(e).insert(RigidBody::Dynamic);
            }
            piece.born = now - t.translation.distance(centre) * 1e-3;
            piece.settle = Settle::default();
            v.0 = fling;
        }
    }
}

/// How bombs look: a dark glossy ball with a fuse stub on top, glowing in the warning accent. A bomb-head wears one
/// for a head (`figure` puts it on the rig in place of the ball head, a shape cue, Q35); a lit bomb's whole shell
/// blinks, faster as its fuse runs out.
#[cfg(feature = "render")]
pub mod view {
    use bevy::prelude::*;

    use super::{LitBomb, RADIUS};
    use crate::look::Palette;

    /// The bomb's meshes, and the shell and fuse materials every unlit bomb shares.
    #[derive(Resource)]
    pub struct Kit {
        shell_mesh: Handle<Mesh>,
        fuse_mesh: Handle<Mesh>,
        shell: Handle<StandardMaterial>,
        fuse: Handle<StandardMaterial>,
    }

    /// A lit bomb's own shell material, which blinks.
    #[derive(Component)]
    struct Light(Handle<StandardMaterial>);

    /// The shell's colour: near-black, so the fuse and the blink stand out.
    const SHELL: Color = Color::srgb(0.07, 0.07, 0.09);

    pub fn plugin(app: &mut App) {
        app.add_systems(Startup, load).add_systems(Update, (dress, blink).chain());
    }

    fn shell(base_color: Color) -> StandardMaterial {
        StandardMaterial { base_color, perceptual_roughness: 0.3, ..default() }
    }

    fn load(
        mut commands: Commands,
        palette: Res<Palette>,
        mut meshes: ResMut<Assets<Mesh>>,
        mut mats: ResMut<Assets<StandardMaterial>>,
    ) {
        commands.insert_resource(Kit {
            // Faceted like the figures' cubes.
            shell_mesh: meshes.add(Sphere::new(RADIUS).mesh().ico(1).expect("a valid icosphere")),
            fuse_mesh: meshes.add(Cuboid::new(0.07, 0.14, 0.07)),
            shell: mats.add(shell(SHELL)),
            fuse: mats.add(StandardMaterial { base_color: palette.accents[3], unlit: true, ..default() }),
        });
    }

    /// A bomb's look, its shell in `shell`: the ball, and the fuse stub leaning out of its top.
    pub fn bomb(kit: &Kit, shell: Handle<StandardMaterial>) -> impl Bundle {
        (
            Mesh3d(kit.shell_mesh.clone()),
            MeshMaterial3d(shell),
            Visibility::default(),
            children![(
                Mesh3d(kit.fuse_mesh.clone()),
                MeshMaterial3d(kit.fuse.clone()),
                Transform::from_xyz(0.03, RADIUS + 0.04, 0.0).with_rotation(Quat::from_rotation_z(-0.35)),
            )],
        )
    }

    /// A bomb-head's head: an unlit bomb.
    pub fn head(kit: &Kit) -> impl Bundle {
        bomb(kit, kit.shell.clone())
    }

    fn dress(
        mut commands: Commands,
        kit: Res<Kit>,
        mut mats: ResMut<Assets<StandardMaterial>>,
        bombs: Query<Entity, Added<LitBomb>>,
    ) {
        for e in &bombs {
            let light = mats.add(shell(SHELL));
            commands.entity(e).insert((bomb(&kit, light.clone()), Light(light)));
        }
    }

    /// Lit bombs blink in the warning accent, faster as the fuse runs out (`Fuse::blink`).
    fn blink(palette: Res<Palette>, bombs: Query<(&LitBomb, &Light)>, mut mats: ResMut<Assets<StandardMaterial>>) {
        let hot = palette.accents[3].to_linear() * 1.5;
        for (lit, light) in &bombs {
            let glow = if lit.fuse.blink() { hot } else { LinearRgba::BLACK };
            if let Some(mut m) = mats.get_mut(&light.0)
                && m.emissive != glow
            {
                m.emissive = glow;
            }
        }
    }
}
