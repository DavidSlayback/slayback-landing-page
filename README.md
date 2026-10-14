# slayback-landing-page

David Slayback's landing page. The hero is welding-spark physics in Rust → WebAssembly, flowing around the page's text; a Rumble Box–style game is next. Plan and decisions: [`docs/PLAN.md`](docs/PLAN.md).

## Layout

| Path | What |
|---|---|
| `crates/sim` | Pure Rust 2D particle physics (grains and sparks, uniform grid, text SDF obstacles, forcefield). Native tests. |
| `crates/hero-wasm` | C-ABI WASM exports of `sim` for the browser (no wasm-bindgen; no imports). |
| `crates/game-core` | Engine-free game rules shared by both games (movement, combat, waves, run, the input mapping in `controls`, colour `scheme`s, and the 3D floor movement in `plane`). Native tests. |
| `crates/game` | The game at `/play`: Bevy 0.19 + avian2d 0.7, a thin shell over `game-core`. The library's `plugin` is the gameplay (physics stays 2D); `look::plugin` dresses it as 2.5D lit primitives with bloom in the page's colour scheme; `main.rs` adds window and renderer. `tests/scenarios.rs` steps it headless with scripted input. The window, renderer and UI sit behind the default `render` feature, so the scenarios build without them: `cargo test -p game --no-default-features` (CI's `check` job). |
| `crates/game3d` | The 3D port that replaces `game` (spec #58, decisions Q46–Q64): Bevy 0.19 + avian3d 0.7, on the same `game-core`, with the same split (gameplay `plugin`; `look::plugin` and `camera::plugin` behind the default `render` feature; `main.rs` adds the window). `tests/scenarios.rs` steps it headless: `cargo test -p game3d --no-default-features`. See "The 3D game" below. Not a default member, like `game`. |
| `site/` | The static page: `index.html` and `hero.js` (WebGL2 renderer + WASM glue); `site/play/` hosts the game and `site/play/watch/` its touch-device clip page. |
| `tests/e2e` | Playwright browser smoke checks against the built `dist/` (see Browser tests), plus the clip recorder. |
| `scripts/build.sh` | Builds `dist/`: site, `hero.wasm`, and the game (`SKIP_GAME=1` to skip the slow Bevy build; `GAME=game3d` to serve the 3D game at `/play`; `GAME_RELEASE=1` for the release switch). |

## CI

Pull requests get one cheap `check` job: tests (including the headless game), clippy, and the hero build with its size budget. It runs only for non-draft PRs into `main`. PRs, stacked ones included, stay drafts while iterating; mark one ready only to merge it. The wasm game build, browser harness and deploys run on `main` (production, hero only) and on `rc/…` branches (a preview with the game). A newer run cancels an older one, so merging a stack builds once. See decisions Q41, Q42 and Q44.

Locally, pre-commit hooks are the gate (Q45). Install them once per clone: `uv tool install pre-commit && pre-commit install -t pre-commit -t pre-push`. Commits run hygiene checks, rustfmt and clippy; pushes add the tests, the wasm lint of the renderer modules and the hero size budget.

Deploying from this machine (when CI can't): `bash scripts/deploy.sh` ships production, hero only. `bash scripts/deploy.sh rc <n> [game|game3d]` ships a preview of a full game at `https://rc-<n>.slayback.pages.dev`. Without the crate, rc-1 ships the 2.5D `game` (its archive, Q64) and any other preview the 3D `game3d`, so `bash scripts/deploy.sh rc 2` is the 3D preview. Each one runs the browser harness on exactly the build it ships and refuses to deploy on a failure. Production takes about 20 s; a preview takes as long as the game build. Both need wrangler logged in, or credentials in `.env.local`. While CI is unavailable, merge once the local hooks pass and deploy right after.

## Develop

```sh
cargo test --workspace --release        # physics tests
cargo run -p sim --release --example bench
bash scripts/build.sh                   # → dist/
python -m http.server -d dist 8080      # preview at http://localhost:8080 (add ?hud=1 for timings)
```

### Game dev loop

```sh
cd crates/game && trunk serve          # http://127.0.0.1:8090, rebuilds and reloads on save
cd crates/game3d && trunk serve        # the 3D game: http://127.0.0.1:8091
```

Uses the fast `dev` profile: no LTO, no debug info, our crates at `-O1`, dependencies at `-O2`. Measured on this machine: the first build takes ~6 min. After that, an edit reaches a reloaded tab in **~14 s** (game crate) to **~16 s** (`game-core`). The size-optimised `game` profile (~10 min cold) is used only by `scripts/build.sh`, CI and deploys. `trunk` 0.21.14 comes from its prebuilt release; it fetches a matching `wasm-bindgen` if needed.

For feel-tuning, the game also runs natively in a desktop window:

```sh
bash scripts/native.sh                  # cargo run -p game; other args go to cargo: native.sh test -p game
bash scripts/native.sh run -p game3d    # the 3D game
```

On a Windows GNU-host toolchain the only install is `rustup component add llvm-tools`. The script copies the toolchain's own `rust-lld` (as `ld`) and `llvm-dlltool` into `target/native-tools/` and links with LLD. This is needed because GNU ld mislinks the `raw-dylib` import libraries that Bevy's Windows crates need, and the exe crashes at startup. No MSVC, nothing global, and wasm and CI builds are unchanged. On other hosts it is plain `cargo`. Measured: the first build takes ~7.5 min; rebuilds take ~17 s after a game-crate edit and ~6 s after a `game-core` edit.

Requires Rust stable with the `wasm32-unknown-unknown` target, plus `wasm-bindgen` 0.2.129 (must match `Cargo.lock`) for the game, and optionally Binaryen's `wasm-opt`. On a Windows GNU-host toolchain, install `wasm-bindgen` from its prebuilt release; building it with `cargo install` needs `dlltool`.

The first game build takes about 10 minutes (fat LTO for size); the web build is about 16 MB, or 5 MB gzipped. Throwaway design spikes live on the `spike/hero` branch.

`scripts/build.sh` honours `CARGO_TARGET_DIR`, so worktrees can share one target directory.

### Playing the game

**Controls.**

| Action | Keys |
|---|---|
| Run | A / D or the arrow keys |
| Dash | Space or Shift |
| Punch | J or left click |
| Kick | K or right click |
| Drop-kick | any attack during a dash |
| Grab | L or middle click: a live enemy or a debris part (frozen debris thaws) |
| Spin-throw | attack while holding; it hurts every enemy it passes through |
| Set down | L again |

Gamepads work too, with nothing to set up: left stick or d-pad to run, A or the right trigger to dash, X punch, Y kick, B grab, Start to pause or start (`crates/game/src/input.rs` maps every device to the same actions through the table in `game-core::controls`, shared with the 3D game; +0.03 MB gzipped).

There is no jump: you climb by walking up debris.

**The box.**
- **Enemies** come in through the side-wall doors, which glow for 0.5 s first:
  - **Fodder** walk up and punch.
  - **Thugs** (from box 2) are slow, wide and tough. Punches hurt them but don't interrupt their overhead smash; kicks, drop-kicks and throws do.
  - **Ninjas** (from box 3) are fast and fragile. They strike, retreat, and won't stand in front of you; they dart through to your back.
  - **Bomb-heads** (from box 2, any kind): when defeated, the head drops as a lit bomb. It blinks for 1.5 s, then blows, hurting everyone nearby and knocking debris loose (never deleting it). You can grab and throw it.
- **Debris:** a defeated enemy breaks into its six parts, which freeze 3 s later (or on sleep) and become solid terrain. Fighters step up ledges of up to 24 units, so piles are climbable. The budget (B1) is at most 1000 parts; the oldest go first.

**The run** (`game-core::run`) is endless. These numbers are first guesses for the playtest (Q29).
- **Escape:** pile debris under yourself until you stand at the rim, which loads the next box.
- **Each new box:** about 45 units taller, a shorter timer, one more enemy at a time, and the next colour scheme.
- **Timer:** running out costs 25 health and restarts the clock.
- **Health:** escaping refills 30%; dying restarts at box 1.

**Demo mode.** `/play/?demo=1` plays a scripted fight by synthetic key presses: punch, kick, drop-kick and spin-throw. Add `&slow=0.25` for slow motion (for recording a clip), `&debris=1000` to load the box with debris, or `&enemy=thug|ninja` and `&bombs=1` to change who steps up. Natively, run `DEMO=1 SLOW=0.25 bash scripts/native.sh`.

**The look** follows the B2 spike's budget (`docs/research/game-research.md`, section 6): `bevy_pbr`, one directional light with a single shadow cascade, emissive neon plus bloom, and ACES tonemapping.
- **Resolution:** `site/play/` caps the canvas at a 16:9 stage of 1280×720 CSS px and scales it up with a CSS transform; the game ignores the display's pixel ratio. The backbuffer therefore never exceeds 1280×720.
- **Measured** (headless Chrome, Intel UHD, vsync off): about 8 ms per frame, or 10 ms with a full 1000-part pile. `game_bg.wasm` is about 6.0 MB gzipped.
- **Reduced motion:** with `prefers-reduced-motion` (or `?motion=reduce`), a hit tints the body instead of flashing it white, and sparks drop to 40%. There is no screen shake to remove. The browser harness checks that the game picks the setting up.
- **Quality scaling** (`game-core::quality`): if frames average over 18 ms for 1.5 s, the game sheds bloom, then MSAA, then shadows, and never steps back up.
  - **Measured on software WebGL** (SwiftShader, 1920×1080): it stepped through all three levels, and frames went from 112 to 86 ms.
  - **On the Intel UHD** (~8 ms per frame) it never engages.
- **Shapes, not just colours** (Q35): thugs are broad with shoulder plates, ninjas slim with a trailing headband, bomb-heads have a bomb for a head, and the player wears a glowing pack. See the greyscale check in `docs/media/enemies-greyscale.png`.
- **Colour scheme:** the game starts in the visitor's hero colour scheme (`slayback-scheme` in localStorage), and `?palette=` overrides it. It then rotates the scheme box by box.

### Score and screens

A run starts from an overlay that lists the controls. Esc pauses, and so does the page losing focus. The HUD shows health, box, timer and score.
- **Score** (`game-core::score`): each kill is worth 100 × a streak multiplier. Kills within 2.5 s of each other step it up (×1.5, ×2, up to ×4). Escaping adds 10 points per second left × the box number.
- **Game over:** shows the score, the box reached, and the best run. The best is stored as `v1;score;box;date` in localStorage (`slayback-best`), or natively in the temp directory.
- **Sound:** off by default; M on the pause screen toggles it, and the choice is kept (`slayback-sound`). Effects (hit, heavy hit, shatter, boom) are synthesised by `python tools/sfx/generate.py`, which is deterministic, and embedded in the game. Audio added about 0.09 MB gzipped.
- **Size:** the UI (`bevy_ui` + text) added 0.41 MB gzipped. `game_bg.wasm` is 6.42 MB gz (Q34 estimated +0.38 MB).

### The 3D game

`crates/game3d` rebuilds the game in full 3D, Rumble Box style (spec #58, tickets #61–#70), beside the 2.5D game until it reaches parity (G10 retires `game`). Each slice lands on `main` and is previewed on rc-2; rc-1 keeps the 2.5D game.

- **Build:** `GAME=game3d bash scripts/build.sh` puts the 3D game at `dist/play/` under the same file names (`game.js`, `game_bg.wasm`), so `site/play/` hosts either. Without `GAME`, `/play` stays the 2.5D game.
- **Preview:** `bash scripts/deploy.sh rc 2` builds it with the release switch on, runs the browser harness on that build with `GAME=game3d`, and ships `https://rc-2.slayback.pages.dev`. Production is unchanged: hero only, switch off.
- **G1 (now):**
  - **The box:** a 15 × 10 m floor (3:2, Q56; about eight fighters across) with walls up to the rim (G3 sets it box by box). Every wall collides at full height; the front one is drawn as glass, a faint pane with a thin outline (Q57).
  - **The camera:** fixed at a 3/4 view, 50° down with a 34° field of view, placed to frame the whole box for the window's shape (`camera::frame`, an exact fit, Q48).
  - **The player:** moves in eight directions at 6 m/s (WASD or arrows, or the left stick or d-pad, through the shared mapping in `game-core::controls`). It turns smoothly to face its last move and has a ring on the floor in the scheme's main accent (Q49, Q60). G3 makes its body a move-and-slide controller that climbs the pile.
  - **The look:** one directional light with no shadow map and no bloom, ACES tonemapping, MSAA. Flat-shaded slabs and cubes, with a dark disc under the player standing in for a shadow (research §7). The hybrid look is G4.
- **G2, fighting:**
  - **Blows:** J punches and K kicks; either during a dash (Space or Shift, `game-core::plane::Dash`) is a drop-kick. Each hits a 100° cone in front of the player, reaching the 2.5D move's reach past the body (a punch about 1.4 m from the centre), level targets only. Aim assist snaps the swing toward the nearest enemy inside the cone, and bends a drop-kick's dash after it (`game-core::aim`). Timings, damage, hit-stop and the finisher freeze are the 2.5D rules (`game-core::combat`).
  - **Fodder:** eleven cubes and a ball head (`game-core::debris::solid::FODDER`), in the scheme's second accent (`figure::colours` holds every figure colour). Up to three at a time enter through four doors, two in the back wall and one in each side (Q58), whose panels pulse for 0.5 s first. They steer at the player while keeping apart and off the walls (`game-core::steer`, Q63), then telegraph (arm back, lit) and punch on the 2.5D brain's timings. Fighters pass through each other, so a crowd can't pin the player.
  - **Debris:** a defeat bursts the figure into its pieces with an outward fling and spin (damped, so heaps stay near where the figure fell and the ball head stops rolling). A piece freezes to static after 1.5 s at rest (or 6 s loose). At most 300 move at once (the oldest moving freeze early) and 1,500 stay frozen (the oldest go), per S2.
  - **Impact and HUD:** trauma shake (quadratic, real time, `impact::Shake`), a star burst at the point of contact, a squash on the victim, kill points floating up with the streak (`game-core::score`), and a health bar with the score. The feel rules are shared with the 2.5D game in `game-core::impact`.
- **G3, climbing and the run:**
  - **The controller** (`player.rs`): the player is a kinematic capsule moved once per physics step by avian's `MoveAndSlide`. It slides along walls and debris, falls under gravity and lands, walks up slopes up to 50° and steps up ledges up to 0.4 m with no jump (Q59; `game-core::climb`). That height is the fodder's torso block standing on end, so any piece lying on the floor is a step and a pile must be built up as a ramp. Input sets its floor velocity, and knockback sets all of it. It stands on a block's edge, and walks down piles without hopping.
  - **Escape** (Q51): when the pile under the player's footprint reaches the rim, the box is escaped. `game-core::climb::pile_height` takes the debris pieces as data (cubes and balls, any rotation) and returns the highest surface under the footprint at or below the feet. Only debris counts, and an invisible fence on every wall keeps the player and flung pieces from going over the rim.
  - **The run** (`run.rs`, rules in `game-core::run` as in the 2.5D game): boxes in sequence, one life. Each box has a shorter timer and more enemies at a time, and the console logs `box N started`. Escaping refills 30% health, adds the time bonus, empties the box and rebuilds it at the next rim. Running out of time costs 25 health and restarts the clock (Q27). A death ends the run on the game-over screen (G8). The HUD shows the box and its clock.
  - **The rim** (`game-core::run::rim_metres`): 1.5 m in box 1, then +0.48 m a box (the 2.5D curve scaled), levelling off at 3 m. The 2.5D rims (4.4 m and up) are out of reach in 3D, where a pile grows with the cube root of its volume. Measured headless: 25 fodder killed in one spot heap to ~2 m, and the player climbs ~1.8 m of it. The same kills spread over 2 m reach ~1.2 m. 50 in one spot reach ~3 m.
  - **Demo stairs run:** `/play/?demo=1&stairs=1` (natively `DEMO=1 STAIRS=1`) disables enemies, builds a staircase of 0.3 m steps up to each rim, and walks the player right, out of box after box. The browser harness's scripted climb uses it. The rest of demo mode is G9.
  - **Not yet:** the scheme doesn't rotate per box (the look owns the palette; the run reports the scheme index in `BoxStarted`). Enemies are still dynamic capsules that stop at a pile's foot.
- **G6, grab and spin-throw** (`grab.rs`, rules in `game-core::grab::flat`; the shared numbers in `game-core::grab`):
  - **Grab** (L, middle click or B): the nearest `Grabbable` thing in the blows' 100° cone within about a metre of the player's edge, between its feet and head. That is a live enemy or a debris part, and frozen debris thaws (it starts again as a fresh loose piece). It rides overhead out of the physics world: it collides with nothing, and a held enemy neither thinks nor swings. Grab again to set it down in front. A lit bomb is `Grabbable(Held::Bomb)` (G7).
  - **Spin-throw** (attack while holding): the player turns toward the biggest enemy group within 30° of its facing (Q62; `flat::throw_heading`: enemies within the throw's range, grouped by gaps of at most 1.5 m, the nearer group on a tie), plants its feet and whirls the thing once and a half round in 0.22 s, from overhead down to chest height. It flies flat (no gravity) at 12 m/s for 0.9 s, about 11 m. Held and thrown things stay inside the walls.
  - **In flight** it passes through fighters and hurts every enemy whose body it passes, once each, swept between frames: the 2.5D damage (a body 16, a part 9), knockback along the throw, hit-stun, flash, a star burst, hit-stop with the finisher's longer freeze on a kill, and the kill's points. A thrown enemy is hurt (10) by its first impact. Then it lands as what it was: a stunned enemy, or loose debris that freezes and counts toward the caps again.
- **G8, screens, score and sound** (as in the 2.5D game, Q34/Q35):
  - **Phases** (`flow.rs`): a fresh page opens on the start overlay with the controls (`screens.rs`); a click, Enter, Space or Start begins. Esc pauses, and so does the page losing focus; the pause screen has the sound toggle. Anything but playing pauses the game clock, so physics, the box clock, swings and streaks all stop. A death (a blow or the clock) shows the game-over screen with the score, the box reached and the best, flagged when the run set it; a click or Enter starts a fresh run at box 1. The console logs `phase start`, `phase playing` and so on for the browser harness.
  - **Best score** (`score.rs`): the 2.5D game's record (`game-core::score::Best`, `v1;score;box;date`) under the same key, `slayback-best`, so a best set in either game counts in both. `store.rs` keeps it in localStorage, or natively in a file in the temp directory; headless scenarios give each test its own directory.
  - **Sound** (`sound.rs`): the 2.5D game's generated set, embedded from `crates/game/assets/sfx/`: a hit or heavy hit per blow, a shatter per kill, and a new rising beep when a door starts warning (`python tools/sfx/generate.py` regenerates all five; the first four are byte-identical). Off by default; M on the pause screen toggles it, kept under `slayback-sound` as in the 2.5D game. A bomb's blast plays the 2.5D `boom.wav` (G7).
  - **Demo:** `?demo=1` goes straight into play with no overlay and doesn't pause on blur, as in the 2.5D demo.
- **G7, bomb-heads** (`bomb.rs`, rules in `game-core::bomb`, its `space` module for the 3D blast):
  - **Bomb-heads:** each door entry is one with the box spec's chance (none in box 1, 15% in box 2, rising to 30%), as in the 2.5D game. A bomb-head wears a dark faceted bomb with a glowing fuse stub in place of its ball head (a shape cue, Q35). On defeat the head drops free as a lit bomb and the other ten pieces burst into debris as usual.
  - **Lit bombs:** the 2.5D fuse (1.5 s) and blink (3 Hz rising to 12 Hz, the whole shell lighting in the warning accent). The blast reaches 3 m in 3D (not the 2.5D 4.4 m converted, which would cover two-fifths of the box), with the 2.5D damage (32 at the centre to 8 at the edge) and launch falloff fed the full 3D distance, pushed away across the floor and up a little (scaled as blows are). It hurts and launches everyone in range, the player included, with a 0.12 s hit-stop, the strongest shake (`BLAST_TRAUMA`) and the biggest star burst. Other lit bombs in range go off 0.15 s later (`CHAIN`), so chains ripple; a bomb-head the blast kills drops a fresh one. Debris in range thaws and is flung, never deleted; past the moving cap the farthest freeze again first. Each blast sends `bomb::Exploded` (for the sound).
  - **Grab and throw:** a lit bomb grabs, carries and spin-throws like anything else, its fuse burning on in hand; a live (stunned or not) bomb-head grabs as an enemy, and drops its bomb wherever it is defeated. A thrown bomb clips each enemy it passes (4 damage), but stops dead at the first one it hits and drops there, as the 2.5D bomb bounced off the first body (`flat::stops`), so it lies in the group until it blows on its fuse.
  - **Demo:** `/play/?demo=1&bombs=1` (natively `DEMO=1 BOMBS=1`) makes every enemy that comes through a door a bomb-head.
- **Headless scenarios** (`crates/game3d/tests/scenarios.rs`) cover landing, walking into a corner against the walls, no way out in any of eight directions (glass included), full speed on diagonals, smooth turning, and the gamepad stick with its deadzone. G2 adds: a punch lands in its cone and not beside, behind or beyond; aim assist turns a kick 40°; a drop-kick launches harder than a punch; blows land at 20 fps and across a 250 ms frame; a crowd closes in, keeps apart and punches; fodder don't pin the player against a wall; a defeat bursts into eleven pieces that freeze on the floor; a 330-piece burst holds the 300 moving cap; and the oldest frozen pieces go past 1,500. G3 adds: stepping onto a piece lying on the floor but not onto a waist-high block; no climbing a wall or the glass, even dashing; no way onto or over the glass from a pile just under the rim; climbing a loose pile that freezes, escaping to box 2 (empty, rebuilt at its rim, with the time bonus); a 25-fodder heap built and climbed at 30 fps, frozen pieces unmoved; the clock's penalty and a timeout on the last health ending the run; a defeat ending it; and the demo's stairs run reaching box 3. G6 adds: grabbing a fodder in front (not one behind), carrying it overhead into a corner without it punching, and setting it down inside the box; a spin-throw through a line of three that hurts each once, defeats the weakened one (burst and score) and hurts the thrown fodder once; frozen debris thawing when grabbed and freezing again after its throw; and aim assist bending a throw 20° toward a group of three, past a lone fodder ahead and a bigger group 45° off. G8 adds: a run from the start overlay (frozen until Enter) through a kill to the game-over screen with its score, box and a new best kept in the store, then a fresh run from zero that doesn't beat it, and the best read back by the next visit; a best from the 2.5D game's record carried over and beaten only by a higher score; Esc freezing the player, the enemies and the clock (with no punch saved up) until Esc again; and sound off until M on the pause screen, the choice kept both ways. Both death scenarios now go through the game-over screen. The browser harness checks the start overlay, a click starting the game, and Esc pausing it. G7 adds: a blast hurting and launching fodder nearby (more the nearer) but not one 4 m off, nor the player 4.5 m off; the player hurt and knocked back a metre from one; a chain going off a moment after the first blast, not the bomb 6 m on; a defeated bomb-head dropping its head as a lit bomb that blows without deleting debris; a blast in a 550-piece frozen heap thawing at most the moving cap, nearest first, deleting none; a cooked bomb thrown into a group stopping at the first fodder and its blast hurting all three, not the thrower; a grabbed bomb-head thrown to its defeat dropping its lit bomb where it broke; box 1 sending no bomb-heads and `Waves.bombs` (the demo knob) making every entry one; and a blast on the player's last health ending the run on the game-over screen.
- **Size:** `game_bg.wasm` is 19.1 MB, or 6.02 MB gzipped, against the 2.5D game's 6.6 MB gzipped (G1: 4.85 MB; G2: 5.85 MB, adding fighting and `bevy_ui` with text for the HUD; G3: +0.17 MB for the controller and the run). After G6 (grab and throw), `build.sh` reports 19,567 KB, or 6,032 KB gzipped. G8 (screens, `bevy_audio` with WAV and the embedded effects) brings it to 19,994 KB, or 6,147 KB gzipped (+0.11 MB). G7 (bomb-heads, with the embedded `boom.wav`) brings it to 20,052 KB, or 6,166 KB gzipped.

## Game release switch

Visitors can't find the game until it ships. `GAME_RELEASE=1 bash scripts/build.sh` turns on its entry points:

- a **Play** item in the nav and a small **Play the game** call to action under the tagline (a `data-collide` hero element, so the sparks flow around it), both linking to `/play/`;
- an easter egg: the Konami code (↑ ↑ ↓ ↓ ← → ← → B A) on the homepage opens `/play/`;
- touch devices (`(hover: none) and (pointer: coarse)`) that reach `/play/` are sent to `/play/watch/`, a looping gameplay clip with a "Play on a computer" note, before any game JS or wasm is fetched.

The switch is off by default, and deploys never set it. Off, the homepage is byte-for-byte the pre-switch page. In `site/`, switch-only content sits between `<!-- GAME_RELEASE -->` and `<!-- /GAME_RELEASE -->` markers (`/* … */` in CSS and JS), on their own lines or inline. The build deletes the marked content when the switch is off and only the markers when it is on. The switch needs the game, so the build refuses `GAME_RELEASE=1` with `SKIP_GAME=1`.

The clip (`site/play/watch/clip.mp4` plus `poster.jpg`) is six seconds of the real game: the scripted demo fight, recorded by `tests/e2e/record-clip.mjs` in its slow-motion mode. The page runs at `slow=0.2`; the recorder screenshots as fast as it can and times each frame back to game speed, then encodes 640×360 H.264, muted, at about 26 KB. The recorder's frame-exact mode (`?capture=1`) still serves the hero.

```sh
SKIP_GAME=1 bash scripts/build.sh && cd tests/e2e && npm ci
SLOW=0.2 CROP=480:270:240:262 PAGE='/play/?demo=1&slow=0.2&palette=netrunner' node record-clip.mjs   # → site/play/watch/clip.mp4 + poster.jpg (needs ffmpeg and a game build)
```

## Browser tests

Playwright smoke checks in `tests/e2e/` run against the built `dist/`: the homepage renders with no console errors, the no-WebGL fallback keeps readable text, reduced motion shows a still frame, `/play` loads cleanly (skipped when the build has no game), and the release switch behaves (see above).

```sh
SKIP_GAME=1 bash scripts/build.sh       # or a full build to cover /play
cd tests/e2e && npm ci && npx playwright test
```

Locally it uses the installed Chrome (`PW_CHANNEL=` to use a Playwright-managed browser) with SwiftShader WebGL, so no GPU is needed; `DIST=<dir>` tests another build. Set `GAME_RELEASE=1` when testing a switch-on build: the switch-off checks (no Play link, no clip page, an inert easter egg) run otherwise. Set `GAME=game3d` when testing a 3D build: the 2.5D game's scripted climb and reduced-motion checks are skipped until the 3D port brings them over (G3, G9); the load check runs on both, and the start-overlay check (G8) runs on the 3D build only. CI's `browser` job runs on `main` and `rc/` branches (not on PRs): it tests the full switch-on build and the hero-only `dist` that main deploys.
