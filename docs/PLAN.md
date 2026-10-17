# Portfolio plan

```text
As of: Thu Oct 15 2026 CDT
In flight: branch hero-sparks (from main), previewed at https://rc-5.slayback.pages.dev (rc-3, rc-4 mirror it): the homepage sparks vary a little in size and shape (David, 2026-10-17: "add some slight variance to the size and shape of the sparks"). Merged 2026-10-17 (David: "Merge"): PR #83 (look-flat: Q74-Q79, the homepage lettering and light, touch controls, the architecture refactors, the tagline, sound) as 88d10cf, with CI blocked by the account's billing limit, so the local hooks were the gate (README rule); production redeployed from main (homepage only, the game still behind its switch).
Architecture (David, 2026-10-16, after /improve-codebase-architecture; picked "1, 2, 5, 3. Maybe 4 and 6 after"): deepening refactors on look-flat, one commit each, every scenario green, behaviour identical except the held-enemy fix. (1) Blow: game-core combat::Blow + land() + impact::Feel; game3d Strike message, one fighter::land system; Hit carries Feel, not Move; land() skips held fighters unless the blow reaches them (blasts), fixing a mid-grab punch hitting the held enemy. DONE. (2) Carry: grab.rs -> carry.rs, sole writer of Carry { target, phase: Hold | Whirl { t, heading } } on the player (enum CarryPhase), Carried and Thrown; it aims the throw; player.rs only stands still while whirling; Facing::snap for the held enemy. DONE. (5) Frame: Frame::{Decide, Act, Resolve, Show} chained once in lib.rs; Decide/Act/Resolve run only while Playing, Show always; Time<Virtual> stays the hit-stop/physics clock; cross-module .after() edges go (the only ones left: debris, run and score .after(Combat), Combat being the first part of Resolve). DONE. (3) Look: pose.rs gains look(&mut Motion, &FigureState, dt) -> Look (Motion moved into pose; Look carries a Tint, not a paint); figure::animate only gathers and writes, and is the rig's only writer (the squash countdown moved from impact to comfort). DONE. (4) Kinds: game-core enemy::Style holds each kind's name, capsule radius and height too (game3d's radius constants, size() and debris::solid::height went); an enemy's figure strides full at its own walk speed (was half the run for every kind: thugs now step slower, ninjas faster, the one visible change). DONE. (6) Box: run::InBox marks everything a box owns, from the enemy/piece/lit_bomb bundles, and the box clears With<InBox> (was a hand-kept Or<Brain, Piece, LitBomb> list); debris::pile_under is the one "pile under a point" (run's escape test and the demo's placement). DONE. All six review candidates done. Terms in GLOSSARY.md.
Next: David plays rc-5, on a computer and on a phone. Then the touch playtest question (Q78): which inputs get used, whether a phone needs a slower game. When touch passes, delete site/play/watch and record-clip.mjs, or keep the clip as the no-WebGL fallback.
Waiting on: David: rc-5; playtesters (#29), gamepad (#26), C2 content (#13).
Re-arm: nothing scheduled. wrangler logged in (OAuth). ubuntu-latest moves to Ubuntu 26 on 2026-10-19. CI out of minutes until Nov 1.
Verified: game-core 114 tests, game3d 64 headless scenarios + 18 render unit tests, clippy both feature sets; rc-3 harness 14 passed, 2 skipped (shaders.spec.js included). Frames of all four stages compared against the video's full frames.
```

Status: 2026-10-12. Phase 0 and game slice 1 are merged to `main`. **The plan for everything left is complete:**
- grilling decisions in `docs/planning/game-decisions.md`
- research in `docs/research/game-research.md`
- the spec in `docs/planning/spec.md` (issue #5)
- 24 tickets (#6–#29) with dependencies

The homepage launches first; the game ships behind a switch after a cohort playtest. The parked slice-2 combat prototype (draft PR #4) is superseded by ticket D2 (#17). The hero spikes live on branch `spike/hero` (draft PR #1, do not merge). The sections below record the earlier phases and decisions; the spec and tickets supersede the Phase 1 outline.

## Look and impact (2026-10-13)

David, after the camera and look pass: **"Make the hits have more impact. Take inspiration from rumble box for the visuals. We may need to go to 3d."**

- **Rumble Box, from its official screenshots** (research §1): an open-topped 3D box seen from above at ~45°. Fighters move across the floor in two dimensions, and debris fills the floor as a heap. Figures are several cubes per limb plus a sphere head, in bold flat colours with black outlines. Comic "BOOM!" bursts, star-badge combo counts and floating score numbers. Debris takes the arena's colour, so the dead read apart from the living.
- **Two tracks:**
  1. **Impact pass, in the current 2.5D game.** Screen shake (off with reduced motion), a burst at the point of impact, squash and stretch on the victim, longer hit-stop on finishing blows, floating score numbers, harder-flying debris. Each carries over to 3D.
  2. **3D feasibility spike** on branch `spike/3d` (do not merge). An avian3d box under a 3/4 camera, the player moving across the floor, cube-built enemies that burst into tumbling cubes, and a pile of 200 to 1,000 cubes. It measures fps on the integrated GPU and the wasm size against avian2d.
- **Done:** impact pass #57 (on rc-1). The 3D spike passed: 60 fps at 500 resting cubes on the integrated GPU with bloom off, and no wasm size cost (research §7).
- **Decided: go 3D.** David: "We should properly plan 3d first." Grilling round 5 (Q46–Q64) settled it; spec #58 (`docs/planning/spec-3d.md`); tickets #59–#70.

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
| Previews (David, 2026-10-15) | **"We really should just have one site, and probably it should be the latest one, with everything included."** Unmerged work stacks on one branch with one PR, and one preview (rc-5) serves all of it; no preview per branch. | rc-1 stays the 2.5D archive. |
| Hero lettering (David, 2026-10-15) | **"I'd like my lettering to not have the color scheme, but instead be lit up by the light sources of the sparks. Just a bit above barely readable unless lit."** The name and lines take no scheme colour: dim neutral ink (`INK` 0.09 since the brighter sparks below; just legible on the dark) plus the sparks' light map, so each letter takes the colour of the current passing it, brightest on the edges that face it. Schemes still pick the sparks and the font. | `?ink=` and `?lit=` tune it on the live page. |
| Tagline (David, 2026-10-16) | **"I don't really like my tagline. Make it something a bit more interesting."** Chose, from four: "Reinforcement learner. Agent wrangler. Recovering gradient descender." (was "RL/ML enthusiast and recent LLM convert"). Balanced over two lines (`text-wrap: balance`); the meta description says it plainly. | |
| Game sound (David, 2026-10-16: "when I turn the sound on I still don't hear anything") | The game made sound all along (with it on, the demo fight hands the browser samples peaking at 0.58), but the audio backend (cpal) creates its AudioContext while the game loads and asks to resume it only then, before any interaction, so a visitor's browser keeps it suspended (the autoplay policy). The play page keeps each AudioContext the game makes and resumes it on every click, key or tap. Test browsers don't suspend audio, so the harness suspends it itself, then checks a key wakes it, and checks the demo fight is audible with sound on. David, after the fix: "Works." | |
| Spark variance (David, 2026-10-17) | **"add some slight variance to the size and shape of the sparks."** The sim gives each spark a grain (0..1, drawn at birth, kept for life, in its output's spare eighth float); the spark shader varies its width (x0.75-1.3) and streak length (x0.6-1.45) by it, independently, so short stubby sparks mix with long thin ones. | |
| Hero light (David, 2026-10-15) | **"more light and/or more particles on the main page. More radiance from each spark, probably make them faster or make them last longer. Also, I think I'd like their source to be a bit more consistent than now. Still random, but maybe from top and/or bottom so that I can rely on semi-consistent lighting. The letters can be a bit darker by default too."** Torches sit two above the screen and two below (sim `top_bottom`), each still jumping along its edge; 18k sparks (9k on small screens, from 14k and 7k); launched faster (180–480 px/s) with less drag (1.1), living 3.5–8 s; each spark's light half again as wide, more of it as background glow; ink 0.09. | Physics about 7 ms a step for 18k on this desktop (4.8 ms at 14k, 8.6 ms at 20k). |
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
