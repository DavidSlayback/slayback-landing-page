# Portfolio plan

Status: 2026-10-12. Hero look is being chosen; nothing production-grade exists yet. Repo, Rust toolchain and preview pages are set up.

## Settled

| Decision | Choice | Why |
|---|---|---|
| Engine | Rust → WASM, `wgpu` (WebGPU, WebGL2 fallback), hand-rolled loop | Small bundle for a landing page; Bevy-sized WASM would delay first paint. |
| Page content | Static semantic HTML; canvas behind it, `aria-hidden` | Links, SEO, screen readers, and a no-WebGL fallback stay intact. |
| Hosting | Private GitHub repo `DavidSlayback/slayback-landing-page` (local checkout: `repos/portfolio`) → Cloudflare Pages | Serves `application/wasm`; `_headers` can add COOP/COEP if WASM threads are ever needed. |
| Game (v1) | Rumble Box–style physics brawler on a lazy-loaded route | Shares the sim core with the hero; loads only when someone clicks Play. |
| Motion hygiene | `prefers-reduced-motion`, a persisted motion toggle, a static fallback on WebGL context loss | Matches the reference's polish; keeps it from reading as a gimmick. |

Reference studied: canwetakethis.online ("letterfall"): ~46 KB vanilla JS, a custom SI-unit rigid-body solver, glyph-SDF collisions, letters on springs to their DOM positions, and WebGL2 instanced rendering.

## In progress: hero spike

The spike lives on branch `spike/hero` (draft PR #1, do not merge). `spikes/hero/hero-prototype.html` is a throwaway WebGL2 page. It answers only "which look?"; the shader math is what carries over to WGSL.

- `?variant=particles` (A): about 147k particles spring-bound to pixels of the name. The cursor repels them, a click sends a shockwave, Space scatters them.
- `?variant=flow` (B): about 262k particles following curl noise that flows around the name's SDF (Bridson's ramp). The cursor makes a vortex; `t` cycles the text style.
- `?n=<side>` sets the particle count to side², and `?warm=<frames>` pre-runs the sim (useful for screenshots).
- Colour schemes, chosen with `?palette=`, the bar's second control, or `p`. The particle math is the same for both; the look comes from colours, a post-processing pass, and the font:
  - `netrunner` (Cyberpunk 2077, green/blue): Chakra Petch; a cyan → mint → acid-green palette; scanlines, chromatic aberration, a neon halo, random slice-tear glitches, and a square grid lit by the particles.
  - `renaissance` (Deus Ex: Human Revolution): Rajdhani with wide tracking; a luminance → black/umber/gold gradient map at 85% (the "piss filter"); film grain; bloom; a hex grid lit by the particles.
  - Screenshots: `spikes/hero/{netrunner,renaissance}-{particles,flow}.png`.
- Motion clips: `spikes/hero/motion/<variant>-<palette>.{gif,mp4}` (gitignored).
  - To regenerate, run `spikes/hero/capture/record.mjs` and then `encode.sh`.
  - Recording uses `?capture=1`, which advances the sim exactly 1/30 s per frame while Playwright scripts the cursor, so playback timing doesn't depend on render speed.

Verified: both variants render with no console errors in headless Chrome using SwiftShader (software renderer, so fps is meaningless there). Not yet measured on a real GPU.

Known spike issues: in B, text placed over the flow (the tagline) is hard to read. Under reduced motion, the page shows a settled still and animates only after a click.

Published previews (private Claude artifacts; share them from each page's Share menu):
- Live lab: https://claude.ai/artifact/S1DT4sha9mGCRC8znsC2Wh. This is the spike with its document wrapper stripped; republish it after spike changes.
- Clip reel: https://claude.ai/artifact/LVFFQgJU4BrekxMQt5qwag. The four MP4s, with timelines of the scripted events.

## Environment

- Rust 1.99 stable, installed via rustup with the **GNU host** (`x86_64-pc-windows-gnu`, using the bundled linker, so no Visual Studio Build Tools), plus the `wasm32-unknown-unknown` target, clippy and rustfmt. Verified that a Rust 2024 cdylib builds to wasm32 and runs under Node. If a crate with C dependencies ever needs MSVC, run `rustup toolchain install stable-msvc`.
- Pushes go over HTTPS with `gh` as the credential helper; there is no GitHub SSH key on this machine.

## Open (needs David)

1. Pick A, B, or a mix (e.g. B's flow with A's particles forming the name on load), and pick a colour scheme. The schemes could also be a user-facing theme toggle.
2. Connect Cloudflare Pages to the repo (needs your Cloudflare login).
3. Game shape. Recommendation: low-poly 3D with a fixed isometric camera, `rapier3d` for physics, and enemies built from primitives drawn with instancing. The pile-of-bodies mechanic doubles as the performance showcase (thousands of live rigid bodies).

## Next

1. Hero choice → capture the verdict here and on PR #1, then close the PR.
2. Install `wasm-bindgen-cli`, pinned to the `wasm-bindgen` crate version, and scaffold a Cargo workspace: `crates/sim` (pure and native-testable), `crates/web` (wasm-bindgen + wgpu), `site/` (static HTML/CSS).
3. Port the chosen hero to wgpu: compute-shader update on WebGPU, CPU/WASM update on the WebGL2 fallback.
4. Deploy the hero-only page to Cloudflare Pages, then start the game.
