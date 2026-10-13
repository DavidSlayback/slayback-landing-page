# slayback-landing-page

David Slayback's landing page. The hero is welding-spark physics in Rust → WebAssembly, flowing around the page's text; a Rumble Box–style game is next. Plan and decisions: [`docs/PLAN.md`](docs/PLAN.md).

## Layout

| Path | What |
|---|---|
| `crates/sim` | Pure Rust 2D particle physics (grains and sparks, uniform grid, text SDF obstacles, forcefield). Native tests. |
| `crates/hero-wasm` | C-ABI WASM exports of `sim` for the browser (no wasm-bindgen; no imports). |
| `crates/game-core` | Engine-free game rules (movement, later combat and waves). Native tests. |
| `crates/game` | The game at `/play`: Bevy 0.19 + avian2d 0.7, a thin shell over `game-core`. The library's `plugin` is the gameplay (physics stays 2D); `look::plugin` dresses it as 2.5D lit primitives with bloom in the page's colour scheme; `main.rs` adds window and renderer. `tests/scenarios.rs` steps it headless with scripted input. The window, renderer and UI sit behind the default `render` feature, so the scenarios build without them: `cargo test -p game --no-default-features` (CI's `check` job). |
| `site/` | The static page: `index.html` and `hero.js` (WebGL2 renderer + WASM glue); `site/play/` hosts the game and `site/play/watch/` its touch-device clip page. |
| `tests/e2e` | Playwright browser smoke checks against the built `dist/` (see Browser tests), plus the clip recorder. |
| `scripts/build.sh` | Builds `dist/`: site, `hero.wasm`, and the game (`SKIP_GAME=1` to skip the slow Bevy build; `GAME_RELEASE=1` for the release switch). |

## CI

Pull requests get one cheap `check` job: tests (including the headless game), clippy, and the hero build with its size budget. It runs only for non-draft PRs into `main`. PRs, stacked ones included, stay drafts while iterating; mark one ready only to merge it. The wasm game build, browser harness and deploys run on `main` (production, hero only) and on `rc/…` branches (a preview with the game). A newer run cancels an older one, so merging a stack builds once. See decisions Q41, Q42 and Q44.

Locally, pre-commit hooks are the gate (Q45). Install them once per clone: `uv tool install pre-commit && pre-commit install -t pre-commit -t pre-push`. Commits run hygiene checks, rustfmt and clippy; pushes add the tests, the wasm lint of the renderer modules and the hero size budget.

Deploying from this machine (when CI can't): `bash scripts/deploy.sh` ships production, hero only. `bash scripts/deploy.sh rc <n>` ships a preview of the full game at `https://rc-<n>.slayback.pages.dev`. Both need wrangler logged in, or credentials in `.env.local`.

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
```

Uses the fast `dev` profile: no LTO, no debug info, our crates at `-O1`, dependencies at `-O2`. Measured on this machine: the first build takes ~6 min. After that, an edit reaches a reloaded tab in **~14 s** (game crate) to **~16 s** (`game-core`). The size-optimised `game` profile (~10 min cold) is used only by `scripts/build.sh`, CI and deploys. `trunk` 0.21.14 comes from its prebuilt release; it fetches a matching `wasm-bindgen` if needed.

For feel-tuning, the game also runs natively in a desktop window:

```sh
bash scripts/native.sh                  # cargo run -p game; other args go to cargo: native.sh test -p game
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

Gamepads work too, with nothing to set up: left stick or d-pad to run, A or the right trigger to dash, X punch, Y kick, B grab, Start to pause or start (`crates/game/src/input.rs` maps every device to the same actions; +0.03 MB gzipped).

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

Locally it uses the installed Chrome (`PW_CHANNEL=` to use a Playwright-managed browser) with SwiftShader WebGL, so no GPU is needed; `DIST=<dir>` tests another build. Set `GAME_RELEASE=1` when testing a switch-on build: the switch-off checks (no Play link, no clip page, an inert easter egg) run otherwise. CI's `browser` job runs on `main` and `rc/` branches (not on PRs): it tests the full switch-on build and the hero-only `dist` that main deploys.
