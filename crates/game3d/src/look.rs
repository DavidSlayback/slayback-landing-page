//! The 3D look: the hybrid of decision Q50. Flat-shaded cubes under one directional light with no shadow map and no
//! bloom (the WebGL2 budget, research section 7), dark edge outlines (`outline`), and the page's colour scheme by the
//! rules in `game_core::palette`: the arena muted, live fighters in the vivid accents, the player's ring in the main
//! accent.
//!
//! Gameplay entities carry no visuals; this plugin dresses them when they appear, so the headless scenarios run the
//! same game without assets or a renderer.

use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;
use game_core::debris::solid::Limb;
use game_core::palette::{self, Fighter, Rgb};
use game_core::scheme::{self, Scheme};

use crate::arena::{Kind, Slab, WALL};
use crate::fighter::Attack;
use crate::player::{Dasher, Facing, HEIGHT, Holding, Player};
use crate::pose::{self, Act};
use crate::run::BoxStarted;

/// The current box's colour scheme (`game_core::scheme`): the page's at the start, then rotating box by box (Q31).
/// It changes as each box starts, so whatever is coloured from it repaints when it changes. Colour things by `game_core::palette`'s rules for
/// [`Palette::scheme`]: muted tones through [`srgb`], a fighter's lit body through [`vivid`], e.g.
/// `vivid(palette::fighter_colour(p.scheme, who).body)`. The raw accents are here for effects that want one directly.
#[derive(Resource, Clone, Copy, Debug)]
pub struct Palette {
    pub scheme: &'static Scheme,
    /// The four neon accents, the first one primary.
    pub accents: [Color; 4],
}

/// A `game_core::palette` colour as a Bevy colour.
pub fn srgb([r, g, b]: Rgb) -> Color {
    Color::srgb(r, g, b)
}

impl Palette {
    pub fn nth(i: usize) -> Self {
        let scheme = &scheme::SCHEMES[i % scheme::SCHEMES.len()];
        Self { scheme, accents: scheme.accents.map(srgb) }
    }
}

/// The page's scheme, which the boxes rotate from: `?palette=` first, then the visitor's saved pick (the hero's
/// `slayback-scheme`).
#[derive(Resource)]
struct PageScheme(usize);

impl PageScheme {
    fn read() -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            let window = web_sys::window();
            let query = window.as_ref().and_then(|w| w.location().search().ok()).unwrap_or_default();
            let saved = window.as_ref().and_then(|w| w.local_storage().ok()??.get_item("slayback-scheme").ok()?);
            Self(scheme::page_choice(&query, saved.as_deref()))
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self(0)
        }
    }
}

pub fn plugin(app: &mut App) {
    let page = PageScheme::read();
    app.insert_resource(ClearColor(Color::srgb(0.035, 0.04, 0.055)))
        .insert_resource(GlobalAmbientLight { color: Color::srgb(0.75, 0.82, 1.0), brightness: 900.0, ..default() })
        .insert_resource(Palette::nth(page.0))
        .insert_resource(page)
        .add_plugins((
            crate::camera::plugin,
            crate::outline::plugin,
            crate::figure::plugin,
            crate::impact::plugin,
            crate::hud::plugin,
        ))
        .add_systems(Startup, (load_kit, spawn_light))
        .add_systems(
            Update,
            (
                rotate_scheme,
                repaint.run_if(resource_changed::<Palette>),
                dress_slabs,
                fade_walls,
                dress_player,
                animate,
            )
                .chain(),
        );
}

/// Each box takes the next scheme, starting from the page's (Q31).
fn rotate_scheme(mut started: MessageReader<BoxStarted>, page: Res<PageScheme>, mut palette: ResMut<Palette>) {
    let Some(BoxStarted(spec)) = started.read().last() else { return };
    let next = Palette::nth(page.0 + spec.scheme);
    if palette.scheme.key != next.scheme.key {
        *palette = next;
    }
}

/// Paints the kit's materials from the palette: once at the start, then whenever a box changes the scheme.
fn repaint(palette: Res<Palette>, kit: Res<Kit>, mut mats: ResMut<Assets<StandardMaterial>>) {
    let arena = palette::arena_tone(palette.scheme);
    let player = palette::fighter_colour(palette.scheme, Fighter::Player);
    for (mat, colour) in [
        (&kit.floor, srgb(arena.floor)),
        (&kit.wall, srgb(arena.wall)),
        (&kit.body, srgb(player.body)),
        // Rumble Box's player reads by contrast between its parts (orange head, blue body, red fists): a pale head,
        // and fists in the scheme's accent furthest from the body.
        (&kit.head, srgb(player.body).mix(&Color::WHITE, 0.6)),
        (&kit.fist, srgb(furthest(palette.scheme.accents, player.body))),
        (&kit.trim, srgb(player.trim)),
        (&kit.ring_mat, srgb(palette::ring(palette.scheme))),
    ] {
        if let Some(mut m) = mats.get_mut(mat) {
            m.base_color = colour;
        }
    }
}

/// The accent furthest from `from` (by squared sRGB distance).
fn furthest(accents: [Rgb; 4], from: Rgb) -> Rgb {
    let d = |c: &Rgb| c.iter().zip(from).map(|(a, b)| (a - b) * (a - b)).sum::<f32>();
    accents.into_iter().max_by(|a, b| d(a).total_cmp(&d(b))).unwrap_or(from)
}

/// Shared meshes and materials, so every cube batches.
#[derive(Resource)]
struct Kit {
    cube: Handle<Mesh>,
    floor: Handle<StandardMaterial>,
    wall: Handle<StandardMaterial>,
    glass: Handle<StandardMaterial>,
    /// Black, unlit: the rim and glass bars and the tile lines.
    line: Handle<StandardMaterial>,
    ball: Handle<Mesh>,
    body: Handle<StandardMaterial>,
    head: Handle<StandardMaterial>,
    fist: Handle<StandardMaterial>,
    trim: Handle<StandardMaterial>,
    ring: Handle<Mesh>,
    ring_mat: Handle<StandardMaterial>,
    blob: Handle<Mesh>,
    blob_mat: Handle<StandardMaterial>,
}

/// Makes the kit; [`repaint`] colours it.
fn load_kit(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>) {
    let mut matte = || mats.add(StandardMaterial { perceptual_roughness: 0.9, ..default() });
    let (floor, wall, body, head, fist, trim) = (matte(), matte(), matte(), matte(), matte(), matte());
    let unlit = || StandardMaterial { unlit: true, ..default() };
    let kit = Kit {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        ball: meshes.add(Sphere::new(0.5).mesh().ico(2).unwrap()),
        floor,
        wall,
        body,
        head,
        fist,
        trim,
        // Barely there: the pane's faces stack, and anything more washes out the floor behind it.
        glass: mats.add(StandardMaterial {
            base_color: Color::srgba(0.7, 0.85, 1.0, 0.035),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
        line: mats.add(StandardMaterial { base_color: Color::BLACK, ..unlit() }),
        // The player's ring (Q60): flat on the floor under the feet, in the main accent, unlit so it stays bright.
        ring: meshes.add(Annulus::new(0.5, 0.64).mesh().resolution(48)),
        ring_mat: mats.add(unlit()),
        // A soft dark disc under the feet stands in for a shadow (no shadow map).
        blob: meshes.add(Circle::new(0.45).mesh().resolution(32)),
        blob_mat: mats.add(StandardMaterial {
            base_color: Color::srgba(0.0, 0.0, 0.0, 0.4),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
    };
    commands.insert_resource(kit);
}

/// One directional light (WebGL2 keeps only one), from above and in front, a little to the right, so the two side
/// walls' faces read apart; no shadow map.
fn spawn_light(mut commands: Commands) {
    commands.spawn((
        DirectionalLight { illuminance: 9_000.0, shadow_maps_enabled: false, ..default() },
        Transform::from_xyz(5.0, 14.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// The black bars along the rim and round the glass, metres thick (Rumble Box's thick black edges, Q66).
const EDGE: f32 = 0.14;
/// Rumble Box's tiles: a black line every metre across the floor and up the walls, this thick.
const TILE: f32 = 1.0;
const LINE: f32 = 0.05;

/// A wall's solid face, its rim bar or one of its tile lines. A wall the camera is behind turns to glass and loses
/// its lines and bar, so it never hides the fight.
#[derive(Component)]
struct WallFace {
    /// The wall's centre and its outward normal, on the floor.
    centre: Vec2,
    outward: Vec2,
    solid: bool,
}

/// Dresses each slab in scaled unit cubes (children, so the slab's own transform stays unscaled for its collider).
fn dress_slabs(mut commands: Commands, kit: Res<Kit>, slabs: Query<(Entity, &Slab, &Transform), Added<Slab>>) {
    for (e, slab, at) in &slabs {
        let s = slab.size;
        commands.entity(e).insert(Visibility::default());
        let mut cube = |size: Vec3, at: Vec3, mat: &Handle<StandardMaterial>, face: Option<WallFace>| {
            let mut c = commands.spawn((
                Mesh3d(kit.cube.clone()),
                MeshMaterial3d(mat.clone()),
                Transform::from_translation(at).with_scale(size),
                ChildOf(e),
            ));
            if let Some(face) = face {
                c.insert(face);
            }
        };
        match slab.kind {
            Kind::Floor => {
                cube(s, Vec3::ZERO, &kit.floor, None);
                // Tile lines over the inner floor, just proud of it.
                let inner = Vec2::new(s.x, s.z) - Vec2::splat(2.0 * WALL);
                let y = s.y / 2.0 + 0.004;
                for x in lines(inner.x) {
                    cube(Vec3::new(LINE, 0.008, inner.y), Vec3::new(x, y, 0.0), &kit.line, None);
                }
                for z in lines(inner.y) {
                    cube(Vec3::new(inner.x, 0.008, LINE), Vec3::new(0.0, y, z), &kit.line, None);
                }
            }
            Kind::Wall => {
                // Which way the wall faces: along z for the back wall, x for the side walls. The back wall spans the
                // corners; the side walls only the floor between them.
                let along_z = s.z < s.x;
                let outward =
                    if along_z { Vec2::Y * at.translation.z.signum() } else { Vec2::X * at.translation.x.signum() };
                let face = |solid| Some(WallFace { centre: at.translation.xz(), outward, solid });
                cube(s, Vec3::ZERO, &kit.wall, face(true));
                // The rim bar goes with the wall when the camera is behind it, or it would slice across the fight.
                cube(Vec3::new(s.x + 0.02, EDGE, s.z + 0.02), Vec3::Y * s.y / 2.0, &kit.line, face(false));
                // Tile lines on the inner face: across at each metre of height, and up at each metre along.
                let inward = -Vec3::new(outward.x, 0.0, outward.y) * (WALL / 2.0 + 0.004);
                let span = if along_z { s.x - 2.0 * WALL } else { s.z };
                let bar = |along: f32, up: f32| {
                    if along_z { Vec3::new(along, up, 0.008) } else { Vec3::new(0.008, up, along) }
                };
                let mut y = TILE;
                while y < s.y - TILE / 2.0 {
                    cube(bar(span, LINE), Vec3::Y * (y - s.y / 2.0) + inward, &kit.line, face(false));
                    y += TILE;
                }
                for a in lines(span) {
                    let at = if along_z { Vec3::X * a } else { Vec3::Z * a };
                    cube(bar(LINE, s.y), at + inward, &kit.line, face(false));
                }
            }
            // The glass is a faint pane drawn by its edges: a bar along the rim and the two corner posts.
            Kind::Glass => {
                cube(s, Vec3::ZERO, &kit.glass, None);
                cube(Vec3::new(s.x, EDGE, EDGE), Vec3::Y * (s.y - EDGE) / 2.0, &kit.line, None);
                for x in [-1.0, 1.0] {
                    cube(Vec3::new(EDGE, s.y, EDGE), Vec3::X * x * (s.x - EDGE) / 2.0, &kit.line, None);
                }
            }
        }
    }
}

/// Positions of the tile lines across a span `len` metres long, centred on zero: every [`TILE`], ends included.
fn lines(len: f32) -> impl Iterator<Item = f32> {
    let n = (len / TILE).round() as i32;
    (0..=n).map(move |i| -len / 2.0 + i as f32 * len / n as f32)
}

/// Turns the walls the camera is behind to glass, and hides their tile lines and rim bars.
fn fade_walls(
    kit: Res<Kit>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    mut faces: Query<(&WallFace, &mut MeshMaterial3d<StandardMaterial>, &mut Visibility)>,
) {
    let Some(eye) = cameras.iter().next().map(|c| c.translation().xz()) else { return };
    for (face, mut mat, mut shown) in &mut faces {
        let behind = (eye - face.centre).dot(face.outward) > 0.0;
        if face.solid {
            let want = if behind { &kit.glass } else { &kit.wall };
            if mat.0 != *want {
                mat.0 = want.clone();
            }
        } else {
            let want = if behind { Visibility::Hidden } else { Visibility::Inherited };
            if *shown != want {
                *shown = want;
            }
        }
    }
}

/// The player's figure, turned by its facing and posed by `pose`; it is built looking along +x.
#[derive(Component)]
pub(crate) struct Rig;

/// One loose part of the player: its limb, where it rests, and how far down the limb it sits (0 at the shoulder or
/// hip, 1 at the fist or foot).
#[derive(Component)]
struct Part {
    limb: Limb,
    rest: Vec3,
    reach: f32,
}

/// The player's motion: walk phase, the side its last blow came from, how far into the current swing it was last
/// frame (negative for none), whether it was holding something, and seconds since it let go.
#[derive(Component)]
struct Motion {
    phase: f32,
    side: f32,
    last: f32,
    held: bool,
    released: Option<f32>,
}

impl Default for Motion {
    fn default() -> Self {
        Self { phase: 0.0, side: -1.0, last: -1.0, held: false, released: None }
    }
}

#[derive(Clone, Copy)]
enum Shape {
    Cube,
    Ball,
}

#[derive(Clone, Copy)]
enum Paint {
    Body,
    Head,
    Fist,
    Trim,
}

const BLOCK: Vec3 = Vec3::splat(0.27);
const UPPER: Vec3 = Vec3::splat(0.2);
const THIGH: Vec3 = Vec3::new(0.22, 0.3, 0.22);
const FOOT: Vec3 = Vec3::new(0.3, 0.18, 0.22);
const FIST: Vec3 = Vec3::splat(0.3);

/// The player, Rumble Box style: loose parts with gaps between them, a ball head, a torso of four cubes, an upper arm
/// and a big ball fist each side, two cubes to each leg. (shape, size, centre relative to the capsule's centre,
/// limb, reach, paint); +z is the right-hand side, the feet at -HEIGHT / 2.
const FIGURE: [(Shape, Vec3, Vec3, Limb, f32, Paint); 14] = [
    (Shape::Cube, THIGH, Vec3::new(0.0, -0.42, -0.15), Limb::LeftLeg, 0.35, Paint::Trim),
    (Shape::Cube, THIGH, Vec3::new(0.0, -0.42, 0.15), Limb::RightLeg, 0.35, Paint::Trim),
    (Shape::Cube, FOOT, Vec3::new(0.04, -0.785, -0.15), Limb::LeftLeg, 1.0, Paint::Trim),
    (Shape::Cube, FOOT, Vec3::new(0.04, -0.785, 0.15), Limb::RightLeg, 1.0, Paint::Trim),
    (Shape::Cube, BLOCK, Vec3::new(0.0, -0.1, -0.145), Limb::Trunk, 0.0, Paint::Body),
    (Shape::Cube, BLOCK, Vec3::new(0.0, -0.1, 0.145), Limb::Trunk, 0.0, Paint::Body),
    (Shape::Cube, BLOCK, Vec3::new(0.0, 0.2, -0.145), Limb::Trunk, 0.0, Paint::Body),
    (Shape::Cube, BLOCK, Vec3::new(0.0, 0.2, 0.145), Limb::Trunk, 0.0, Paint::Body),
    (Shape::Cube, UPPER, Vec3::new(0.0, 0.24, -0.42), Limb::LeftArm, 0.35, Paint::Body),
    (Shape::Cube, UPPER, Vec3::new(0.0, 0.24, 0.42), Limb::RightArm, 0.35, Paint::Body),
    (Shape::Ball, FIST, Vec3::new(0.06, -0.02, -0.45), Limb::LeftArm, 1.0, Paint::Fist),
    (Shape::Ball, FIST, Vec3::new(0.06, -0.02, 0.45), Limb::RightArm, 1.0, Paint::Fist),
    (Shape::Ball, Vec3::splat(0.42), Vec3::new(0.0, 0.62, 0.0), Limb::Trunk, 0.0, Paint::Head),
    // A visor on the face shows which way the player faces.
    (Shape::Cube, Vec3::new(0.06, 0.1, 0.26), Vec3::new(0.2, 0.64, 0.0), Limb::Trunk, 0.0, Paint::Trim),
];

fn dress_player(mut commands: Commands, kit: Res<Kit>, players: Query<Entity, Added<Player>>) {
    for e in &players {
        let feet = -HEIGHT / 2.0;
        commands.entity(e).insert((Visibility::default(), Motion::default()));
        commands.spawn((
            Mesh3d(kit.blob.clone()),
            MeshMaterial3d(kit.blob_mat.clone()),
            Transform::from_xyz(0.0, feet + 0.01, 0.0)
                .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            ChildOf(e),
        ));
        commands.spawn((
            Mesh3d(kit.ring.clone()),
            MeshMaterial3d(kit.ring_mat.clone()),
            Transform::from_xyz(0.0, feet + 0.02, 0.0)
                .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            ChildOf(e),
        ));
        let rig = commands.spawn((Rig, Transform::default(), Visibility::default(), ChildOf(e))).id();
        for (shape, size, at, limb, reach, paint) in FIGURE {
            let mesh = match shape {
                Shape::Cube => kit.cube.clone(),
                Shape::Ball => kit.ball.clone(),
            };
            let mat = match paint {
                Paint::Body => &kit.body,
                Paint::Head => &kit.head,
                Paint::Fist => &kit.fist,
                Paint::Trim => &kit.trim,
            };
            commands.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(mat.clone()),
                Transform::from_translation(at).with_scale(size),
                Part { limb, rest: at, reach },
                ChildOf(rig),
            ));
        }
    }
}

/// Turns the player's figure to its facing (or, while it whirls something, toward the thing), and poses it
/// (`pose`): the walk, each blow from the arm or leg it uses (punches alternate), a held thing overhead, a whirl and
/// the throw's follow-through, and a lean while dashing. On the game clock, so hit-stop freezes the pose.
#[allow(clippy::type_complexity)]
fn animate(
    time: Res<Time>,
    mut players: Query<
        (&GlobalTransform, &Facing, &LinearVelocity, &Attack, Option<&Holding>, &Dasher, &mut Motion, &Children),
        With<Player>,
    >,
    held: Query<&GlobalTransform, Without<Player>>,
    mut rigs: Query<(&mut Transform, &Children), (With<Rig>, Without<Part>)>,
    mut parts: Query<(&Part, &mut Transform), Without<Rig>>,
) {
    let dt = time.delta_secs();
    for (me, facing, v, attack, holding, dasher, mut m, children) in &mut players {
        let speed = v.0.with_y(0.0).length();
        let amount = (speed / game_core::plane::RUN_SPEED).min(1.0);
        m.phase += speed * 3.0 * dt;
        let mut turn = facing.rotation();
        let act = match (holding, attack.swing) {
            (Some(h), _) => {
                m.held = true;
                if h.spinning() {
                    // Face the thing as it goes round.
                    if let Ok(t) = held.get(h.target) {
                        let to = (t.translation() - me.translation()).with_y(0.0);
                        if to.length() > 1e-3 {
                            turn = Quat::from_rotation_arc(Vec3::X, to.normalize());
                        }
                    }
                    Act::Whirl
                } else {
                    Act::Hold
                }
            }
            (None, Some(swing)) => {
                if swing.elapsed < m.last || m.last < 0.0 {
                    m.side = -m.side;
                }
                m.last = swing.elapsed;
                Act::Strike { swing, side: m.side }
            }
            (None, None) => Act::None,
        };
        if attack.swing.is_none() {
            m.last = -1.0;
        }
        if holding.is_none() && std::mem::take(&mut m.held) {
            m.released = Some(0.0);
        }
        let act = match (act, m.released) {
            (Act::None, Some(t)) => Act::Release { t },
            (act, _) => act,
        };
        m.released = m.released.map(|t| t + dt).filter(|t| *t < pose::RELEASE);
        let (lift, mut lean) = pose::body(m.phase, amount, act);
        if dasher.0.dashing() && matches!(act, Act::None) {
            lean = Quat::from_rotation_z(-0.3);
        }
        let k = pose::follow(dt);
        for &c in children {
            let Ok((mut rig, pieces)) = rigs.get_mut(c) else { continue };
            rig.translation = rig.translation.lerp(lift, k);
            rig.rotation = rig.rotation.slerp(turn * lean, k);
            for &p in pieces {
                if let Ok((part, mut tf)) = parts.get_mut(p) {
                    let to = part.rest + pose::part(part.limb, part.reach, part.rest, m.phase, amount, act);
                    tf.translation = tf.translation.lerp(to, k);
                }
            }
        }
    }
}
