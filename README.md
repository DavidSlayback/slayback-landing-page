# slayback-landing-page

David Slayback's landing page. The hero is welding-spark physics in Rust → WebAssembly, flowing around the page's text; a Rumble Box–style game is next. Plan and decisions: [`docs/PLAN.md`](docs/PLAN.md).

## Layout

| Path | What |
|---|---|
| `crates/sim` | Pure Rust 2D particle physics (grains and sparks, uniform grid, text SDF obstacles, forcefield). Native tests. |
| `crates/hero-wasm` | C-ABI WASM exports of `sim` for the browser (no wasm-bindgen; no imports). |
| `crates/game-core` | Engine-free game rules (movement, later combat and waves). Native tests. |
| `crates/game` | The game at `/play`: Bevy 0.19 + avian2d 0.7, a thin shell over `game-core`. The library's `plugin` is the gameplay (physics stays 2D); `look::plugin` dresses it as 2.5D lit primitives with bloom in the page's colour scheme; `main.rs` adds window and renderer. `tests/scenarios.rs` steps it headless with scripted input (`cargo test -p game`, Linux CI job `game-tests`). |
| `site/` | The static page: `index.html` and `hero.js` (WebGL2 renderer + WASM glue); `site/play/` hosts the game and `site/play/watch/` its touch-device clip page. |
| `tests/e2e` | Playwright browser smoke checks against the built `dist/` (see Browser tests), plus the clip recorder. |
| `scripts/build.sh` | Builds `dist/`: site, `hero.wasm`, and the game (`SKIP_GAME=1` to skip the slow Bevy build; `GAME_RELEASE=1` for the release switch). |

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

There is no jump: you climb by walking up debris.

**The box.**
- **Enemies:** fodder come in through the side-wall doors, which glow for 0.5 s first.
- **Debris:** a defeated enemy breaks into its six parts, which freeze 3 s later (or on sleep) and become solid terrain. Fighters step up ledges of up to 24 units, so piles are climbable. The budget (B1) is at most 1000 parts; the oldest go first.

**The run** (`game-core::run`) is endless. These numbers are first guesses for the playtest (Q29).
- **Escape:** pile debris under yourself until you stand at the rim, which loads the next box.
- **Each new box:** about 45 units taller, a shorter timer, one more enemy at a time, and the next colour scheme.
- **Timer:** running out costs 25 health and restarts the clock.
- **Health:** escaping refills 30%; dying restarts at box 1.

**Demo mode.** `/play/?demo=1` plays a scripted fight by synthetic key presses: punch, kick, drop-kick and spin-throw. Add `&slow=0.25` for slow motion (for recording a clip) or `&debris=1000` to load the box with debris. Natively, run `DEMO=1 SLOW=0.25 bash scripts/native.sh`.

**The look** follows the B2 spike's budget (`docs/research/game-research.md`, section 6): `bevy_pbr`, one directional light with a single shadow cascade, emissive neon plus bloom, and ACES tonemapping.
- **Resolution:** `site/play/` caps the canvas at a 16:9 stage of 1280×720 CSS px and scales it up with a CSS transform; the game ignores the display's pixel ratio. The backbuffer therefore never exceeds 1280×720.
- **Measured** (headless Chrome, Intel UHD, vsync off): about 8 ms per frame, or 10 ms with a full 1000-part pile. `game_bg.wasm` is about 6.0 MB gzipped.
- **Colour scheme:** the game starts in the visitor's hero colour scheme (`slayback-scheme` in localStorage), and `?palette=` overrides it. It then rotates the scheme box by box.

### Score and screens

A run starts from an overlay that lists the controls. Esc pauses, and so does the page losing focus. The HUD shows health, box, timer and score.
- **Score** (`game-core::score`): each kill is worth 100 × a streak multiplier. Kills within 2.5 s of each other step it up (×1.5, ×2, up to ×4). Escaping adds 10 points per second left × the box number.
- **Game over:** shows the score, the box reached, and the best run. The best is stored as `v1;score;box;date` in localStorage (`slayback-best`), or natively in the temp directory.
- **Size:** the UI (`bevy_ui` + text) added 0.41 MB gzipped. `game_bg.wasm` is 6.42 MB gz (Q34 estimated +0.38 MB).

## Game release switch

Visitors can't find the game until it ships. `GAME_RELEASE=1 bash scripts/build.sh` turns on its entry points:

- a **Play** item in the nav and a small **Play the game** call to action under the tagline (a `data-collide` hero element, so the sparks flow around it), both linking to `/play/`;
- an easter egg: the Konami code (↑ ↑ ↓ ↓ ← → ← → B A) on the homepage opens `/play/`;
- touch devices (`(hover: none) and (pointer: coarse)`) that reach `/play/` are sent to `/play/watch/`, a looping gameplay clip with a "Play on a computer" note, before any game JS or wasm is fetched.

The switch is off by default, and deploys never set it. Off, the homepage is byte-for-byte the pre-switch page. In `site/`, switch-only content sits between `<!-- GAME_RELEASE -->` and `<!-- /GAME_RELEASE -->` markers (`/* … */` in CSS and JS), on their own lines or inline. The build deletes the marked content when the switch is off and only the markers when it is on. The switch needs the game, so the build refuses `GAME_RELEASE=1` with `SKIP_GAME=1`.

The clip (`site/play/watch/clip.mp4` plus `poster.jpg`) comes from the frame-exact recorder `tests/e2e/record-clip.mjs`. It loads a page with `?capture=1`, steps its `window.__capture.frame(dt)` hook one fixed timestep per frame, screenshots each frame and encodes them with ffmpeg (640×360 H.264, 5 s, muted). **The committed clip is a placeholder recorded from the hero**, because the game has no capture hook yet. The release ticket re-records it from gameplay with `PAGE=/play/` once the game has that hook.

```sh
SKIP_GAME=1 bash scripts/build.sh && cd tests/e2e && npm ci
node record-clip.mjs                    # → site/play/watch/clip.mp4 + poster.jpg (needs ffmpeg; FRAMES=150, PAGE=/?…)
```

## Browser tests

Playwright smoke checks in `tests/e2e/` run against the built `dist/`: the homepage renders with no console errors, the no-WebGL fallback keeps readable text, reduced motion shows a still frame, `/play` loads cleanly (skipped when the build has no game), and the release switch behaves (see above).

```sh
SKIP_GAME=1 bash scripts/build.sh       # or a full build to cover /play
cd tests/e2e && npm ci && npx playwright test
```

Locally it uses the installed Chrome (`PW_CHANNEL=` to use a Playwright-managed browser) with SwiftShader WebGL, so no GPU is needed; `DIST=<dir>` tests another build. Set `GAME_RELEASE=1` when testing a switch-on build: the switch-off checks (no Play link, no clip page, an inert easter egg) run otherwise. CI's `browser` job tests both: the `game` job's full switch-on build (test-only, never deployed) and the `build` job's switch-off `dist`, which is what deploys.
