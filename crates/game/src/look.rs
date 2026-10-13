//! The 2.5D look (ticket D1): the 2D game drawn as lit, flat-shaded 3D primitives in a slightly tilted perspective,
//! with emissive neon and bloom in the page's colour scheme.
//!
//! Gameplay entities carry no visuals; this plugin dresses them when they appear, so the headless scenarios run the
//! same game without assets or a renderer. Choices and budget come from the B2 spike (docs/research/game-research.md,
//! section 6): one directional light with a single shadow cascade, low ambient, no point lights, ACES tonemapping,
//! bloom and MSAA 4x, a backbuffer capped at 1280x720 by the host page.

use avian2d::prelude::LinearVelocity;
use bevy::camera::Hdr;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::light::CascadeShadowConfigBuilder;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::Msaa;
use game_core::combat::{Move, Phase};
use game_core::debris::Shape;
use game_core::doors::Door;
use game_core::enemy::Kind;
use game_core::movement::RUN_SPEED;

use crate::ARENA;
use crate::arena::{WALL, Wall};
use crate::bomb::{BOMB_RADIUS, BombHead, Exploded, LitBomb};
use crate::comfort::Comfort;
use crate::debris::{DebrisPart, Shattered};
use crate::enemy::{Brain, HALF, WallDoor, door_spot};
use crate::fighter::{Attack, Fighter, Flash, Hit, Team};
use crate::player::Player;
use crate::run::BoxStarted;

/// Depth of the box, world units (the 2D game lives on the z = 0 plane).
pub const DEPTH: f32 = 192.0;

/// A colour scheme shared with the homepage hero (`site/hero.js`): four neon accents, the first one primary.
#[derive(Resource, Clone, Copy, Debug)]
pub struct Scheme {
    pub key: &'static str,
    pub accents: [Color; 4],
}

pub const SCHEMES: [Scheme; 3] = [
    Scheme {
        key: "netrunner",
        accents: [
            Color::srgb(0.62, 1.0, 0.0),
            Color::srgb(0.15, 1.0, 0.2),
            Color::srgb(0.0, 1.0, 0.72),
            Color::srgb(0.88, 1.0, 0.12),
        ],
    },
    Scheme {
        key: "synthwave",
        accents: [
            Color::srgb(1.0, 0.08, 0.72),
            Color::srgb(0.0, 0.9, 1.0),
            Color::srgb(1.0, 0.92, 0.05),
            Color::srgb(0.5, 0.15, 1.0),
        ],
    },
    Scheme {
        key: "gold",
        accents: [
            Color::srgb(1.0, 0.55, 0.04),
            Color::srgb(1.0, 0.82, 0.2),
            Color::srgb(1.0, 0.3, 0.0),
            Color::srgb(1.0, 0.95, 0.65),
        ],
    },
];

impl Scheme {
    pub fn by_key(key: &str) -> Option<Scheme> {
        SCHEMES.iter().copied().find(|s| s.key == key)
    }

    /// Index in [`SCHEMES`] of the page's scheme: `?palette=` first, then the visitor's saved pick (the hero's
    /// `slayback-scheme`), else Netrunner.
    fn page_index() -> usize {
        #[cfg(target_arch = "wasm32")]
        {
            let window = web_sys::window();
            let query = window.as_ref().and_then(|w| w.location().search().ok()).unwrap_or_default();
            let from_query =
                query.trim_start_matches('?').split('&').find_map(|kv| kv.strip_prefix("palette=")).map(str::to_owned);
            let saved = || window.as_ref()?.local_storage().ok()??.get_item("slayback-scheme").ok()?;
            if let Some(i) = from_query.or_else(saved).and_then(|k| SCHEMES.iter().position(|s| s.key == k)) {
                return i;
            }
        }
        0
    }
}

/// The page's scheme (index in [`SCHEMES`]); box `n` uses the one `n - 1` steps after it.
#[derive(Resource)]
struct BaseScheme(usize);

pub fn plugin(app: &mut App) {
    app.insert_resource(ClearColor(Color::BLACK))
        .insert_resource(GlobalAmbientLight { color: Color::WHITE, brightness: 120.0, ..default() })
        .add_plugins(crate::comfort::plugin)
        .insert_resource(BaseScheme(Scheme::page_index()))
        .insert_resource(SCHEMES[Scheme::page_index()])
        .add_systems(Startup, (load_kit, spawn_stage).chain())
        // Before anything is dressed for the new box.
        .add_systems(PreUpdate, (rotate_scheme, build_frame))
        .add_systems(
            Update,
            (
                dress_walls,
                dress_fighters,
                dress_doors,
                dress_debris,
                dress_bombs,
                blink_bombs,
                animate_fighters,
                tint_fighters,
                light_doors,
                spawn_sparks,
                fly_sparks,
            ),
        );
}

/// Shared meshes and materials, so every figure and slab batches.
#[derive(Resource)]
struct Kit {
    wall: Handle<StandardMaterial>,
    rim: Handle<StandardMaterial>,
    skin: Handle<StandardMaterial>,
    spark: Handle<StandardMaterial>,
    debris: Handle<StandardMaterial>,
    spark_mesh: Handle<Mesh>,
    door_mesh: Handle<Mesh>,
    plate: Handle<Mesh>,
    bomb: Handle<Mesh>,
    shell: Handle<StandardMaterial>,
    torso: Handle<Mesh>,
    head: Handle<Mesh>,
    limb: Handle<Mesh>,
    visor: Handle<Mesh>,
}

fn flat(mesh: impl Into<Mesh>) -> Mesh {
    mesh.into().with_duplicated_vertices().with_computed_flat_normals()
}

fn load_kit(
    mut commands: Commands,
    scheme: Res<Scheme>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let accent = scheme.accents[0];
    let mut mat = |base: Color, emissive: LinearRgba| {
        mats.add(StandardMaterial {
            base_color: base,
            emissive,
            perceptual_roughness: 0.85,
            metallic: 0.0,
            ..default()
        })
    };
    let kit = Kit {
        wall: mat(Color::srgb(0.05, 0.07, 0.05), LinearRgba::BLACK),
        rim: mat(accent, accent.to_linear() * 4.0),
        skin: mat(Color::srgb(0.85, 0.9, 0.8), LinearRgba::BLACK),
        // White-hot, like the hero's welding sparks; bloom does the rest.
        spark: mat(Color::WHITE, LinearRgba::rgb(14.0, 12.0, 8.0)),
        // Debris keeps the fallen enemy's colours (recoloured with the scheme each box).
        debris: mat(scheme.accents[2].darker(0.45), scheme.accents[2].to_linear() * 0.6),
        spark_mesh: meshes.add(Cuboid::new(7.0, 1.6, 1.6)),
        door_mesh: meshes.add(flat(Cuboid::new(4.0, 64.0, 72.0))),
        // A thug's shoulder plate (rig units).
        plate: meshes.add(flat(Cuboid::new(0.45, 0.18, 0.5))),
        // A bomb: a dark, slightly glossy ball (world units for lit bombs; heads scale it to rig units).
        bomb: meshes.add(flat(Sphere::new(BOMB_RADIUS).mesh().ico(1).expect("ico subdivision 1 is valid"))),
        shell: mats.add(StandardMaterial {
            base_color: Color::srgb(0.06, 0.06, 0.07),
            perceptual_roughness: 0.4,
            metallic: 0.3,
            ..default()
        }),
        // A figure in "rig units" (about 2.45 tall), scaled to each fighter's collider by its `Rig`.
        torso: meshes.add(flat(Cuboid::new(0.7, 1.0, 0.4))),
        head: meshes.add(flat(Sphere::new(0.32).mesh().ico(1).expect("ico subdivision 1 is valid"))),
        limb: meshes.add(flat(Cuboid::new(0.22, 0.75, 0.22))),
        visor: meshes.add(flat(Cuboid::new(0.4, 0.08, 0.1))),
    };
    commands.insert_resource(kit);
}

/// Camera and light; the box's own back wall and neon come from `build_frame`.
fn spawn_stage(mut commands: Commands) {
    let half = ARENA / 2.0;
    // A slightly tilted view of the whole box (from the B2 spike, scaled to world units).
    commands.spawn((
        Camera3d::default(),
        Hdr,
        Projection::Perspective(PerspectiveProjection { fov: 30f32.to_radians(), far: 10_000.0, ..default() }),
        Transform::from_xyz(0.0, half.y * 0.42, 1600.0).looking_at(Vec3::new(0.0, -40.0, 0.0), Vec3::Y),
        Msaa::Sample4,
        Tonemapping::AcesFitted,
        Bloom::NATURAL,
    ));
    // One key light; WebGL2 supports a single shadow cascade.
    commands.spawn((
        DirectionalLight { illuminance: 6_000.0, shadow_maps_enabled: true, ..default() },
        Transform::from_xyz(-320.0, 960.0, 800.0).looking_at(Vec3::ZERO, Vec3::Y),
        CascadeShadowConfigBuilder { num_cascades: 1, maximum_distance: 4_000.0, ..default() }.build(),
    ));
}

/// The back wall and neon frame of the current box, rebuilt at each box's rim height.
#[derive(Component)]
struct FramePiece;

fn build_frame(
    mut commands: Commands,
    kit: Res<Kit>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut started: MessageReader<BoxStarted>,
    old: Query<Entity, With<FramePiece>>,
) {
    let Some(BoxStarted(spec)) = started.read().last() else { return };
    for e in &old {
        commands.entity(e).despawn();
    }
    let half = ARENA / 2.0;
    let (floor, rim) = (-half.y + WALL, -half.y + WALL + spec.rim);
    let (inner, strip, h) = (half.x - WALL, 5.0, spec.rim);
    let slabs = [
        // The back wall, from under the floor to the rim, behind the play plane.
        (
            Vec3::new(ARENA.x, h + WALL, WALL),
            Vec3::new(0.0, floor + h / 2.0 - WALL / 2.0, -DEPTH / 2.0 - WALL / 2.0),
            &kit.wall,
        ),
        // Neon: a frame on the back wall, along the tops of the side walls (the rim), and the floor's front edge.
        (Vec3::new(2.0 * inner, strip, strip), Vec3::new(0.0, rim, -DEPTH / 2.0), &kit.rim),
        (Vec3::new(2.0 * inner, strip, strip), Vec3::new(0.0, floor, -DEPTH / 2.0), &kit.rim),
        (Vec3::new(strip, h, strip), Vec3::new(-inner, floor + h / 2.0, -DEPTH / 2.0), &kit.rim),
        (Vec3::new(strip, h, strip), Vec3::new(inner, floor + h / 2.0, -DEPTH / 2.0), &kit.rim),
        (Vec3::new(strip, strip, DEPTH), Vec3::new(-half.x + WALL / 2.0, rim, 0.0), &kit.rim),
        (Vec3::new(strip, strip, DEPTH), Vec3::new(half.x - WALL / 2.0, rim, 0.0), &kit.rim),
        (Vec3::new(2.0 * inner, strip, strip), Vec3::new(0.0, floor, DEPTH / 2.0), &kit.rim),
    ];
    for (size, pos, mat) in slabs {
        commands.spawn((
            FramePiece,
            Mesh3d(meshes.add(flat(Cuboid::from_size(size)))),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(pos),
        ));
    }
}

/// Box by box the colours rotate through the schemes, starting from the page's (decision Q31).
fn rotate_scheme(
    mut started: MessageReader<BoxStarted>,
    base: Res<BaseScheme>,
    mut scheme: ResMut<Scheme>,
    kit: Res<Kit>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut tints: Query<(&mut Tint, &ChildOf)>,
    players: Query<(), With<Player>>,
) {
    let Some(BoxStarted(spec)) = started.read().last() else { return };
    *scheme = SCHEMES[(base.0 + spec.scheme) % SCHEMES.len()];
    if let Some(mut m) = mats.get_mut(&kit.rim) {
        m.base_color = scheme.accents[0];
        m.emissive = scheme.accents[0].to_linear() * 4.0;
    }
    if let Some(mut m) = mats.get_mut(&kit.debris) {
        m.base_color = scheme.accents[2].darker(0.45);
        m.emissive = scheme.accents[2].to_linear() * 0.6;
    }
    // The player keeps the primary accent of whatever scheme is up (enemies are dressed fresh in each box).
    for (mut tint, parent) in &mut tints {
        if players.contains(parent.parent()) {
            tint.base = scheme.accents[0].darker(0.55);
            tint.glow_base = scheme.accents[0].to_linear() * 6.0;
            if let Some(mut m) = mats.get_mut(&tint.glow) {
                m.base_color = scheme.accents[0];
            }
        }
    }
}

fn dress_walls(
    mut commands: Commands,
    kit: Res<Kit>,
    mut meshes: ResMut<Assets<Mesh>>,
    walls: Query<(Entity, &Wall), Added<Wall>>,
) {
    for (e, wall) in &walls {
        commands.entity(e).insert((
            Mesh3d(meshes.add(flat(Cuboid::new(wall.0.x, wall.0.y, DEPTH)))),
            MeshMaterial3d(kit.wall.clone()),
        ));
    }
}

/// A fighter's visual root: a primitive figure in rig units, scaled to the collider and turned toward its facing.
#[derive(Component, Default)]
struct Rig {
    /// Walk-cycle phase, advanced by distance travelled.
    stride: f32,
}

/// A fighter's own materials (so its flash and telegraph touch no one else) and their resting colours.
#[derive(Component)]
struct Tint {
    body: Handle<StandardMaterial>,
    glow: Handle<StandardMaterial>,
    base: Color,
    glow_base: LinearRgba,
}

/// A limb, for walk swings and attack poses. It pivots at the shoulder or hip.
#[derive(Component, Clone, Copy, PartialEq)]
enum Limb {
    BackArm,
    FrontArm,
    FrontLeg,
    BackLeg,
}

impl Limb {
    /// Walk swing direction.
    fn swing(self) -> f32 {
        match self {
            Limb::BackArm | Limb::FrontLeg => 1.0,
            Limb::FrontArm | Limb::BackLeg => -1.0,
        }
    }
}

/// Figure height in rig units, and the feet's depth below the rig's origin.
const RIG_HEIGHT: f32 = 2.45;
const RIG_FEET: f32 = 0.775;

#[allow(clippy::type_complexity)]
fn dress_fighters(
    mut commands: Commands,
    kit: Res<Kit>,
    scheme: Res<Scheme>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    fighters: Query<(Entity, &Fighter, &Team, Option<&Brain>, Has<BombHead>), Added<Fighter>>,
) {
    for (e, fighter, team, brain, bomb_head) in &fighters {
        let kind = brain.map(|b| b.0.style().kind);
        // The player wears the primary accent, fodder a contrasting one, thugs a third. Colour is a bonus: the
        // silhouettes differ too (decision Q35, colour-blind safe), so a thug reads as one in any scheme.
        let accent = match (team, kind) {
            (Team::Player, _) => scheme.accents[0],
            (_, Some(Kind::Thug)) => scheme.accents[1],
            (_, Some(Kind::Ninja)) => scheme.accents[3],
            _ => scheme.accents[2],
        };
        let base = accent.darker(0.55);
        let glow_base = accent.to_linear() * 6.0;
        let body = mats.add(StandardMaterial { base_color: base, perceptual_roughness: 0.85, ..default() });
        let glow = mats.add(StandardMaterial {
            base_color: accent,
            emissive: glow_base,
            perceptual_roughness: 0.85,
            ..default()
        });
        let scale = fighter.half.y * 2.0 / RIG_HEIGHT;
        // Thugs are built broad and deep, with shoulder plates; ninjas slim, with a headband trailing behind.
        let thug = kind == Some(Kind::Thug);
        let ninja = kind == Some(Kind::Ninja);
        let build = match kind {
            Some(Kind::Thug) => Vec3::new(1.5, 1.0, 1.25),
            Some(Kind::Ninja) => Vec3::new(0.8, 1.0, 0.8),
            _ => Vec3::ONE,
        };
        commands.entity(e).insert(Visibility::default()).with_children(|p| {
            p.spawn((
                Rig::default(),
                Tint { body: body.clone(), glow: glow.clone(), base, glow_base },
                Transform::from_xyz(0.0, -fighter.half.y + RIG_FEET * scale, 0.0).with_scale(build * scale),
                Visibility::default(),
            ))
            .with_children(|r| {
                if thug {
                    for dx in [-0.5, 0.5] {
                        r.spawn((
                            Mesh3d(kit.plate.clone()),
                            MeshMaterial3d(glow.clone()),
                            Transform::from_xyz(dx, 1.05, 0.0),
                        ));
                    }
                }
                if *team == Team::Player {
                    // The player's shape cue (colour-blind safe, decision Q35): a glowing pack on the back, which no enemy has.
                    r.spawn((
                        Mesh3d(kit.plate.clone()),
                        MeshMaterial3d(glow.clone()),
                        Transform::from_xyz(0.0, 0.55, -0.32).with_scale(Vec3::new(1.1, 2.6, 0.5)),
                    ));
                }
                if ninja {
                    // Band round the head, tails streaming back.
                    r.spawn((
                        Mesh3d(kit.visor.clone()),
                        MeshMaterial3d(glow.clone()),
                        Transform::from_xyz(0.0, 1.5, -0.32).with_scale(Vec3::new(1.2, 1.0, 1.0)),
                    ));
                    r.spawn((
                        Mesh3d(kit.limb.clone()),
                        MeshMaterial3d(glow.clone()),
                        Transform::from_xyz(0.0, 1.35, -0.55)
                            .with_rotation(Quat::from_rotation_x(1.2))
                            .with_scale(Vec3::new(0.5, 0.8, 0.5)),
                    ));
                }
                r.spawn((Mesh3d(kit.torso.clone()), MeshMaterial3d(body.clone()), Transform::from_xyz(0.0, 0.5, 0.0)));
                if bomb_head {
                    // A bomb for a head (a shape cue, not just a colour): dark ball, fuse spike glowing on top.
                    r.spawn((
                        Mesh3d(kit.bomb.clone()),
                        MeshMaterial3d(kit.shell.clone()),
                        Transform::from_xyz(0.0, 1.35, 0.0).with_scale(Vec3::splat(0.045)),
                    ));
                    r.spawn((
                        Mesh3d(kit.limb.clone()),
                        MeshMaterial3d(glow.clone()),
                        Transform::from_xyz(0.0, 1.8, 0.0).with_scale(Vec3::new(0.4, 0.35, 0.4)),
                    ));
                } else {
                    r.spawn((
                        Mesh3d(kit.head.clone()),
                        MeshMaterial3d(kit.skin.clone()),
                        Transform::from_xyz(0.0, 1.35, 0.0),
                    ));
                }
                r.spawn((Mesh3d(kit.visor.clone()), MeshMaterial3d(glow.clone()), Transform::from_xyz(0.0, 1.4, 0.3)));
                for (dx, dy, limb, mat) in [
                    (-0.48, 0.45, Limb::BackArm, &body),
                    (0.48, 0.45, Limb::FrontArm, &body),
                    (0.18, -0.4, Limb::FrontLeg, &glow),
                    (-0.18, -0.4, Limb::BackLeg, &body),
                ] {
                    // The pivot sits at the top of the limb; the mesh hangs below it.
                    r.spawn((limb, Transform::from_xyz(dx, dy + 0.375, 0.0), Visibility::default())).with_child((
                        Mesh3d(kit.limb.clone()),
                        MeshMaterial3d(mat.clone()),
                        Transform::from_xyz(0.0, -0.375, 0.0),
                    ));
                }
            });
        });
    }
}

#[allow(clippy::type_complexity)]
fn animate_fighters(
    time: Res<Time>,
    fighters: Query<(&Fighter, &Attack, &LinearVelocity, Option<&Player>, Option<&Brain>, &Children)>,
    mut rigs: Query<(&mut Rig, &mut Transform, &Children), Without<Limb>>,
    mut limbs: Query<(&Limb, &mut Transform), Without<Rig>>,
) {
    for (fighter, attack, velocity, player, brain, children) in &fighters {
        // A thug raises both arms as its telegraph builds (the smash then brings them down).
        let raise = brain.filter(|b| b.0.style().attack == Move::Smash).map_or(0.0, |b| b.0.telegraph());
        for &child in children {
            let Ok((mut rig, mut tf, rig_children)) = rigs.get_mut(child) else { continue };
            // Mostly side-on toward the facing side, so blows read as aimed at the opponent; a dash leans in.
            let dashing = player.is_some_and(|p| p.0.dashing());
            let lean = if dashing { -0.3 * fighter.facing } else { 0.0 };
            tf.rotation = Quat::from_rotation_z(lean) * Quat::from_rotation_y(1.15 * fighter.facing);
            let speed = (velocity.x.abs() / RUN_SPEED).min(1.0);
            rig.stride += velocity.x.abs() * time.delta_secs() * 0.045;
            // Attack pose: wind back during startup, extend while active, ease back in recovery.
            let pose = attack.swing.map(|s| {
                let reach = match s.phase() {
                    Some(Phase::Startup) => -0.5,
                    Some(Phase::Active) => 1.0,
                    _ => 0.4,
                };
                (s.mv, reach)
            });
            for &c in rig_children {
                let Ok((limb, mut lt)) = limbs.get_mut(c) else { continue };
                let walk = rig.stride.sin() * 0.7 * speed * limb.swing();
                let angle = match (pose, *limb) {
                    (Some((Move::Smash, r)), Limb::FrontArm | Limb::BackArm) => {
                        if r < 0.0 {
                            -2.7
                        } else {
                            -2.7 + 2.0 * r
                        }
                    }
                    (None, Limb::FrontArm | Limb::BackArm) if raise > 0.0 => -2.7 * raise,
                    (Some((Move::Punch, r)), Limb::FrontArm) => -1.55 * r,
                    (Some((Move::Kick | Move::DropKick, r)), Limb::FrontLeg) => -1.4 * r,
                    (Some((Move::DropKick, _)), Limb::BackLeg) => 0.5,
                    _ => walk,
                };
                lt.rotation = Quat::from_rotation_x(angle);
            }
        }
    }
}

/// Hit flash (white) and the fodder telegraph (the visor and front leg brighten as the windup builds).
fn tint_fighters(
    comfort: Res<Comfort>,
    fighters: Query<(&Flash, Option<&Brain>, &Children)>,
    rigs: Query<&Tint>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    // Reduced motion: a hit tints the body toward white instead of flashing it.
    let (flash_mix, flash_glow) = if comfort.reduced_motion { (0.35, 0.6) } else { (1.0, 3.0) };
    for (flash, brain, children) in &fighters {
        for &child in children {
            let Ok(tint) = rigs.get(child) else { continue };
            let lit = flash.0 > 0.0;
            if let Some(mut m) = mats.get_mut(&tint.body) {
                m.base_color = if lit { tint.base.mix(&Color::WHITE, flash_mix) } else { tint.base };
                m.emissive = if lit { LinearRgba::rgb(flash_glow, flash_glow, flash_glow) } else { LinearRgba::BLACK };
            }
            let windup = brain.map_or(0.0, |b| b.0.telegraph());
            if let Some(mut m) = mats.get_mut(&tint.glow) {
                m.emissive = if lit && !comfort.reduced_motion {
                    LinearRgba::rgb(8.0, 8.0, 8.0)
                } else {
                    tint.glow_base * (1.0 + 3.0 * windup)
                };
            }
        }
    }
}

/// A door's panel in its side wall, with its own material for the warning pulse.
#[derive(Component)]
struct DoorPanel(Handle<StandardMaterial>);

fn dress_doors(
    mut commands: Commands,
    kit: Res<Kit>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    doors: Query<(Entity, &WallDoor), Added<WallDoor>>,
) {
    for (e, door) in &doors {
        let mat = mats.add(StandardMaterial {
            base_color: Color::srgb(0.05, 0.07, 0.05),
            perceptual_roughness: 0.85,
            ..default()
        });
        // On the inside face of the wall, standing on the floor.
        let x = door.side * (ARENA.x / 2.0 - WALL - 1.0) - door_spot(door.side, HALF).x;
        commands.entity(e).insert((DoorPanel(mat.clone()), Visibility::default())).with_child((
            Mesh3d(kit.door_mesh.clone()),
            MeshMaterial3d(mat),
            Transform::from_xyz(x, 6.0, 0.0),
        ));
    }
}

/// Closed doors are faint outlines; a warning pulses hot; an open door goes dark.
fn light_doors(
    time: Res<Time<Real>>,
    scheme: Res<Scheme>,
    doors: Query<(&WallDoor, &DoorPanel)>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let warn = scheme.accents[3].to_linear();
    for (door, panel) in &doors {
        let Some(mut m) = mats.get_mut(&panel.0) else { continue };
        m.emissive = match door.door {
            Door::Closed => warn * 0.25,
            Door::Warning(_) => warn * (3.0 + 5.0 * (time.elapsed_secs() * 18.0).sin().abs()),
            Door::Open(_) => LinearRgba::BLACK,
        };
    }
}

/// A spark: flies, falls and shrinks away fast.
#[derive(Component)]
struct Spark {
    velocity: Vec3,
    life: f32,
}

const SPARKS_PER_HIT: usize = 14;
const SPARK_LIFE: f32 = 0.28;

fn spawn_sparks(
    mut commands: Commands,
    kit: Res<Kit>,
    mut hits: MessageReader<Hit>,
    mut shattered: MessageReader<Shattered>,
    mut blasts: MessageReader<Exploded>,
    comfort: Res<Comfort>,
    mut seed: Local<u32>,
) {
    let n = |count: usize| ((count as f32) * comfort.sparks()).ceil() as usize;
    for hit in hits.read() {
        // Along the blow, fanned up and down; heavier blows throw more.
        let heavy = matches!(hit.mv, Move::Kick | Move::DropKick);
        spray(&mut commands, &kit, &mut seed, hit.at, n(SPARKS_PER_HIT + if heavy { 8 } else { 0 }), hit.facing, 1.1);
    }
    for s in shattered.read() {
        // A figure breaking apart: a full ring, twice as many.
        spray(&mut commands, &kit, &mut seed, s.at, n(SPARKS_PER_HIT * 2), 1.0, std::f32::consts::PI);
    }
    for b in blasts.read() {
        // A blast: three full rings.
        for _ in 0..3 {
            spray(&mut commands, &kit, &mut seed, b.at, n(SPARKS_PER_HIT * 2), 1.0, std::f32::consts::PI);
        }
    }
}

/// `count` sparks from `at`, aimed along `facing` (+1 / -1) within ±`fan` radians.
fn spray(commands: &mut Commands, kit: &Kit, seed: &mut u32, at: Vec2, count: usize, facing: f32, fan: f32) {
    for _ in 0..count {
        // xorshift: a spray of directions without a rand dependency.
        *seed = seed.wrapping_add(0x9E37_79B9);
        let mut s = *seed;
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        let a = (s & 0xFFFF) as f32 / 65535.0;
        let b = (s >> 16) as f32 / 65535.0;
        let angle = (a - 0.5) * 2.0 * fan;
        let dir = Vec2::new(facing * angle.cos(), angle.sin());
        let velocity = (dir * (350.0 + 650.0 * b)).extend((b - 0.5) * 300.0);
        commands.spawn((
            Spark { velocity, life: SPARK_LIFE * (0.6 + 0.4 * a) },
            Mesh3d(kit.spark_mesh.clone()),
            MeshMaterial3d(kit.spark.clone()),
            Transform::from_translation(at.extend(20.0)).with_rotation(Quat::from_rotation_z(dir.y.atan2(dir.x))),
        ));
    }
}

/// Debris keeps its enemy's colours: the body tone for every part, so a pile reads as the fallen.
fn dress_debris(
    mut commands: Commands,
    kit: Res<Kit>,
    mut meshes: ResMut<Assets<Mesh>>,
    parts: Query<(Entity, &DebrisPart), Added<DebrisPart>>,
    // One mesh per distinct shape (an enemy type breaks into a few repeated ones), so a whole pile batches.
    mut shapes: Local<Vec<(Shape, Handle<Mesh>)>>,
) {
    for (e, part) in &parts {
        let mesh = match shapes.iter().find(|(s, _)| *s == part.shape) {
            Some((_, mesh)) => mesh.clone(),
            None => {
                let mesh = meshes.add(match part.shape {
                    Shape::Box { w, h } => flat(Cuboid::new(w, h, w.min(h).max(4.0))),
                    Shape::Ball { r } => flat(Sphere::new(r).mesh().ico(1).expect("ico subdivision 1 is valid")),
                });
                shapes.push((part.shape, mesh.clone()));
                mesh
            }
        };
        commands.entity(e).insert((Mesh3d(mesh), MeshMaterial3d(kit.debris.clone())));
    }
}

fn fly_sparks(mut commands: Commands, time: Res<Time>, mut sparks: Query<(Entity, &mut Spark, &mut Transform)>) {
    let dt = time.delta_secs();
    for (e, mut spark, mut tf) in &mut sparks {
        spark.life -= dt;
        if spark.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        spark.velocity.y -= crate::GRAVITY * 0.6 * dt;
        tf.translation += spark.velocity * dt;
        tf.scale = Vec3::splat((spark.life / SPARK_LIFE).clamp(0.2, 1.0));
    }
}

/// A lit bomb's own fuse-light material.
#[derive(Component)]
struct BombLight(Handle<StandardMaterial>);

fn dress_bombs(
    mut commands: Commands,
    kit: Res<Kit>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    bombs: Query<Entity, Added<LitBomb>>,
) {
    for e in &bombs {
        let light = mats.add(StandardMaterial {
            base_color: Color::srgb(0.06, 0.06, 0.07),
            perceptual_roughness: 0.4,
            ..default()
        });
        commands.entity(e).insert((
            Mesh3d(kit.bomb.clone()),
            MeshMaterial3d(light.clone()),
            BombLight(light),
            Visibility::default(),
        ));
    }
}

/// Lit bombs blink, faster as the fuse runs out.
fn blink_bombs(scheme: Res<Scheme>, bombs: Query<(&LitBomb, &BombLight)>, mut mats: ResMut<Assets<StandardMaterial>>) {
    let hot = scheme.accents[3].to_linear() * 10.0;
    for (lit, light) in &bombs {
        if let Some(mut m) = mats.get_mut(&light.0) {
            m.emissive = if lit.fuse.blink() { hot } else { LinearRgba::BLACK };
        }
    }
}
