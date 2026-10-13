# Portfolio plan

```text
As of: Tue Oct 13 2026 03:01 CDT (AFK backlog run paused: everything left needs David)
In flight: nothing running. Stacked open PRs, all held because CI cannot start (GitHub Actions billing): #48 D10 sound -> #49 D11 gamepad -> #50 D12 comfort -> #51 E1 prep. Local checks green on each (60 rules, 26 scenarios, 3 input tests, browser smoke 5/5).
Next: once CI runs: merge #48, retarget #49 to main and merge, then #50, then #51 (each after green CI). Then E1's preview: push an rc/ branch from main (needs David's go-ahead) and run the harness against that preview.
Waiting on: David: (1) CI: fix the Actions billing/spending limit, make the repo public, or approve a self-hosted runner; (2) go-ahead for the rc/ preview deploy (the unreleased game at a public preview URL); (3) a 1-minute gamepad check (#26); (4) C3 #14 analytics, C2 #13 content, E2 #29 playtest. Defaults: hold.
Re-arm: nothing scheduled. ubuntu-latest moves to Ubuntu 26 from 2026-10-19 (CI apt packages may need checking).
Verified: main 33cff19 (through D9) CI green. Branch e1-rc 332dd7d: native tests/clippy clean; dist build 6.42 MB gz; harness 5/5 incl. scripted run to box 2.
```

Status: 2026-10-12. Phase 0 and game slice 1 are merged to `main`. **The plan for everything left is complete:**
- grilling decisions in `docs/planning/game-decisions.md`
- research in `docs/research/game-research.md`
- the spec in `docs/planning/spec.md` (issue #5)
- 24 tickets (#6–#29) with dependencies

The homepage launches first; the game ships behind a switch after a cohort playtest. The parked slice-2 combat prototype (draft PR #4) is superseded by ticket D2 (#17). The hero spikes live on branch `spike/hero` (draft PR #1, do not merge). The sections below record the earlier phases and decisions; the spec and tickets supersede the Phase 1 outline.

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
