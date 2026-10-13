# slayback-landing-page

David Slayback's landing page. The hero is welding-spark physics in Rust → WebAssembly, flowing around the page's text; a Rumble Box–style game is next. Plan and decisions: [`docs/PLAN.md`](docs/PLAN.md).

## Layout

| Path | What |
|---|---|
| `crates/sim` | Pure Rust 2D particle physics (grains and sparks, uniform grid, text SDF obstacles, forcefield). Native tests. |
| `crates/hero-wasm` | C-ABI WASM exports of `sim` for the browser (no wasm-bindgen; no imports). |
| `crates/game-core` | Engine-free game rules (movement, later combat and waves). Native tests. |
| `crates/game` | The game at `/play`: Bevy 0.19 + avian2d 0.7, a thin shell over `game-core`. Web-only build. |
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
