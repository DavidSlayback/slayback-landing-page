# Portfolio plan

```text
As of: Mon Oct 12 13:40 CDT 2026
In flight: nothing running. Slice 2 (combat) is parked as a draft PR on branch game-combat; main has Phase 0 + slice 1 (PRs #2, #3 merged).
Next: David plans the game with his own workflows (design + tech plan) BEFORE any more game code; see "Handoff: game planning" below.
Waiting on: David: game design decisions; Cloudflare Pages login/token; real projects/contact content (default: placeholders stay, nothing deployed).
Re-arm: nothing. No monitors, servers, or scheduled checks were left running.
Verified: main at 4086b8a, CI green; game-combat: cargo test (28) + clippy clean; dist build checked in headless Chrome (fight script, 0 console errors).
```

Status: 2026-10-12. Phase 0 and game slice 1 are merged to `main`. Slice 2 (combat) is a draft PR, parked so the game can be planned properly. The spikes live on branch `spike/hero` (draft PR #1, do not merge).

## Handoff: game planning

- **Why paused:** the game has been specified by one line ("Rumble Box, escape the box, 2D"), so design choices were being made ad hoc during slices. Each game change also costs a ~10-minute full-LTO build. David asked to plan with his own workflows before slice 3.
- **The plan needs to settle:**
  - The core loop: box size, how the pile and exit work, win/lose, progression across boxes.
  - The feel spec: move list, timings, hit-stop and knockback targets.
  - The enemy roster and difficulty ramp.
  - Debris and pile rules, including the physics budget for Avian bodies in single-threaded WASM.
  - Grab and throw (deferred from slice 2).
  - Art and audio direction: how the hero's spark look carries into the game.
  - The tech iteration loop: a fast dev wasm profile without LTO, possibly native runs, headless Bevy system tests, and a scripted playtest harness.
  - Re-cut slices with acceptance criteria.
- **Built so far:**
  - `crates/game-core` holds the engine-free rules: movement, combat (`Move`, `Swing`, hitboxes, knockback, `Health`, `HitStop`) and `FodderBrain`, with 13 tests.
  - `crates/game` is the Bevy 0.19.1 + avian2d 0.7 shell: arena, player (J/Z or left-click punch, K/X or right-click kick), fodder spawner and AI, health bars, white hit-flash, strike boxes, and hit-stop that pauses `Time<Virtual>`.
  - Collision layers: fighters pass through each other and stand on the world.
- **Known issues:**
  - Fodder bunch up on top of each other; they need a separation behavior.
  - There is no HUD text, because Bevy's `ui`/`text` features aren't enabled.
  - A defeated player just respawns; there's no game over.
  - Defeated enemies despawn instead of becoming debris.
- **Build facts:**
  - The web build is 16 MB raw, 5 MB gzipped. Cold `--profile game` builds take ~10 min; `cargo clippy -p game --target wasm32-unknown-unknown --profile game` takes about 1 s incremental.
  - `wasm-bindgen` 0.2.129 is the prebuilt binary in `~/.cargo/bin`, because `cargo install` needs `dlltool` on the GNU host. Binaryen `wasm-opt` v133 is installed too.
- **Verification scripts:** Playwright scripts live in the session scratchpad, not the repo. They should be recreated as a repo playtest harness during planning.
- **Shareable pages** (private Claude artifacts; share from each page's Share menu):
  - Homepage: https://claude.ai/artifact/V5hEXRMs7BKttghCKz4Fym. Production hero, no placeholders.
  - Hero Lab (prototype): https://claude.ai/artifact/S1DT4sha9mGCRC8znsC2Wh.
- **Boundaries:** don't merge PRs or deploy without David's go-ahead. Don't commit `spikes/` to `main`.
- **Refresh:**
  - `git -C C:/Users/david/repos/portfolio fetch && git log --oneline -5 origin/main`
  - `gh pr list -R DavidSlayback/slayback-landing-page`
  - `gh run list -R DavidSlayback/slayback-landing-page --limit 3`

## Settled

| Decision | Choice | Why |
|---|---|---|
| Hero | Spike round 4 *Currents* (`spikes/hero4` on `spike/hero`) | Chosen by David after four rounds (details below). |
| Hero stack | `crates/sim` (Rust → WASM, ~60 KB, plain C ABI) + a small WebGL2 renderer in JS | Fast first paint on the landing page. The renderer works and is verified; porting it to Rust/wgpu would add MBs of WASM for no visible gain. Revisit if David wants all-Rust. |
| Game stack | Bevy **0.19.1** + **avian2d 0.7** (2D now; avian3d and Bevy 3D later) | David wants 3D eventually. Bevy + Avian makes 2D → 3D a swap of physics crate and renderables, not a rewrite. Avian 0.7 targets Bevy 0.19 ([crates.io](https://crates.io/crates/avian2d)); Bevy 0.20 shipped 2026-10-08 and isn't supported by Avian yet, so pin 0.19 and upgrade both together. |
| Shared core | `crates/sim` serves both: the hero via its C ABI, and the game as a Rust dependency for spark and debris effects | One physics core with small variants; tested once. |
| Game design (v1) | 2D side view; *escape the box* loop; keyboard + mouse | Chosen by David. |
| Page content | Static semantic HTML; canvas behind it; hero text is real, transparent DOM drawn by WebGL | Links, SEO, selection and screen readers stay intact. |
| Schemes | Netrunner (lime), Synthwave, Human Revolution (gold): random per visit, switchable in the nav, saved | David likes all three. |
| Hosting | Private GitHub repo `DavidSlayback/slayback-landing-page` (local checkout `repos/portfolio`) → Cloudflare Pages | Serves `application/wasm`; `_headers` can add COOP/COEP later if threads are needed. |
| Motion hygiene | `prefers-reduced-motion` still frame; static fallback if WebGL or WASM is unavailable | Polish; never a blank page. |

## Phase 0: production foundation (on `main`)

Goal: the chosen hero running from production-quality code on `main`, built by CI, and deployable.

1. **Workspace.** A root `Cargo.toml` workspace.
   - `crates/sim`: the pure physics library ported from `spikes/hero4/sim`, with its tests and benches, and no FFI.
   - `crates/hero-wasm`: a `cdylib` holding only the C-ABI exports.
   - Shared `.cargo/config.toml` (wasm `simd128`).
2. **Site.**
   - `site/index.html`: markup and CSS from the hero template, with real copy.
   - `site/hero.js`: the renderer and WASM glue.
   - `site/hero.wasm`: copied in by the build, loaded with `instantiateStreaming` rather than inlined.
3. **Build.** `scripts/build.sh` runs the WASM release build and assembles `dist/`. Local preview uses any static server (WASM can't load from `file://`).
4. **CI.** A GitHub Actions workflow runs `cargo test`, clippy, the wasm build and `dist/` assembly on every push and PR.
5. **Deploy.** Cloudflare Pages from `dist/`. This needs David's Cloudflare login: either Git integration in the dashboard, or an API token for `wrangler pages deploy` from CI.

Acceptance:
- `cargo test --workspace` passes (the 15 sim tests).
- The `dist/` page renders the hero in headless Chrome with no console errors, and text stays readable.
- `hero.wasm` is ≤ 80 KB.
- CI is green on `main`.

## Phase 1: the game (`/play`, lazy-loaded)

Rumble Box's loop: defeated enemies stay in the arena as physical debris, and you pile them up to climb out ([MobyGames](https://www.mobygames.com/game/32162/rumble-box/), [bit-tech](https://bit-tech.net/blogs/free-games-i-like-rumble-box/1/)).

Each slice below is playable on its own, in order:
1. **Tracer bullet. Done (PR #3).** A Bevy app in `crates/game` builds to WASM and loads at `site/play/`. It shows a box arena where a player body runs and jumps (avian2d dynamic body with ground detection).
2. **Combat.** Punch, kick and grab hitboxes with knockback and hit-stop. One enemy type ("fodder") walks over and attacks. Health and defeat.
3. **Debris.** A defeated enemy breaks into its block parts as avian rigid bodies that persist. The pile is solid and climbable.
4. **Escape.** Wave spawner; an exit hatch at the top of the box; reaching it loads the next, harder box. Score and run timer.
5. **Look.** Neon blocks in the active scheme, spark FX from `crates/sim` on hits and shatters, Bevy bloom, a minimal HUD.
6. **Integration.** A "Play" link on the landing page loads `/play` only on click. Back-navigation returns to the hero.

Later, not v1: more enemy types (thugs, ninjas, bomb-heads), bonus stages, gamepad and touch, the 3D port.

Risks:
- **Bevy WASM size.** Measured after slice 1. With thin LTO, no stripping: 87 MB. With symbols stripped: 39 MB. With fat LTO and `opt-level = "z"`: 23 MB. With `wasm-opt -Oz`: 17.6 MB. With trimmed Bevy features: **16.1 MB raw, 4.98 MB gzipped**. Almost all of what's left is Bevy's renderer. Acceptable behind the Play click, with a loading note.
- **The GNU host toolchain** may hit issues with native Bevy builds on Windows. Fallback: `rustup toolchain install stable-msvc` (needs VS Build Tools), or iterate in WASM only.
- **Avian performance** with hundreds of debris bodies in single-threaded WASM. Measure in slice 3. If needed, fewer and larger chunks, or sleeping bodies.

## Hero spike history (on `spike/hero`)

- **Round 4: chosen.** Welding sparks from off-screen torches that relocate every 2.5–6.5 s. Sparks meander through tight curl currents with per-spark wander, then fade. Text is composited last as an opaque, haloed surface, so it's always readable. Holding the pointer raises a 40 px forcefield. 15 native tests; about 5 ms per physics step for 14k sparks.
- **Round 3:** granular grains on the first version of the Rust engine, and the fixed-substep fix that keeps piles stable at 30 fps.
- **Round 2:** shader-only neon variants (hypercube, sandfall, vortex).
- **Round 1:** particle-name and flow-field heroes.

Previews: the live Lab at https://claude.ai/artifact/S1DT4sha9mGCRC8znsC2Wh runs the chosen hero (private Claude artifact). The clip Reel is retired.

## Environment

- **Rust:** 1.99 stable via rustup with the **GNU host** (bundled linker, no VS Build Tools), plus the `wasm32-unknown-unknown` target, clippy and rustfmt.
- **Tools:** Node 24, ffmpeg, and Playwright (`playwright-core` against the installed Chrome) for headless checks.
- **Git:** pushes go over HTTPS with `gh` as the credential helper; there is no GitHub SSH key on this machine.

## Open (needs David)

1. **Cloudflare Pages:** connect the repo in the dashboard (Git integration), or create an API token for CI deploys.
2. **Real content** for the landing page: the project list (names, one-liners, links) and the GitHub/contact links. Placeholders until then.
