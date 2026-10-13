# box3d: full-3D feasibility spike (throwaway, branch `spike/3d`)

Question: can the game move from 2.5D (avian2d on a side-view plane) to full 3D like Rumble Box, meaning
a 3/4 view of an open box, cube-built figures that burst into tumbling cubes, and hundreds of cubes piling up?
Can Bevy 0.19.1 + avian3d 0.7.0 run that in the browser (WebGL2, single-threaded wasm) at 60 fps on the
integrated GPU?

`src/main.rs` builds a 20 x 20 box with 3-unit walls, a 42° perspective camera ~50° down, one directional light (no
shadow map), cool ambient light, MSAA 4×, ACES tonemapping and no bloom. Every primitive gets an inverted-hull outline
(a second, front-culled mesh). The player is a dynamic capsule made of 10 cubes and a sphere head. WASD or the arrow
keys move it; Space punches. Enemies drop in over the walls and walk to the player. A punched enemy bursts into its
11 parts. Query parameters are listed at the top of `main.rs`.

## Run

```sh
cd spikes/box3d
cargo build --release --target wasm32-unknown-unknown   # perf build: opt-level 3, no LTO (~8 min cold)
bash bench/ship.sh release                              # wasm-bindgen into web/
python -m http.server -d web 8093                       # http://127.0.0.1:8093/index.html?cubes=300&demo=1
NODE_PATH=../../tests/e2e/node_modules node bench/bench.cjs http://127.0.0.1:8093 gpu results/igpu.jsonl "bench=1&burst=1&cubes=500"
python -X utf8 bench/table.py results/igpu.jsonl
cargo build --profile game --target wasm32-unknown-unknown && bash bench/ship.sh game   # size build
```

## Method

Same harness as the B2 spike (`docs/research/game-research.md` section 6). Headless Chrome 154 (`channel: 'chrome'`)
ran on ANGLE D3D11 with the **Intel UHD Graphics (0xA78B)** adapter at a 1280×720 viewport, which is also the
backbuffer size. `gpu` mode turns vsync and the frame-rate limit off, so the frame interval measures throughput. Timing
is in-app with `bevy::platform::time::Instant`:

- interval: First to First;
- cpu: First to the end of the render world;
- phys: the time between avian's `PhysicsSystems::First` and `::Last` within each frame.

Each `?cubes=N` page drops N cubes of edge 0.3–0.6, waits until at most 2% are awake, waits 1 s more, then measures
two 10 s windows:

- **settled:** nothing touches the pile.
- **burst:** every 2.5 s, every cube within 4 units of a random point is thrown up and outward with spin.

After that the page waits for the pile to settle again, then watches it for 5 s with nothing touching it.
Avian ran at its defaults: 64 Hz fixed step, 6 substeps, sleeping on. No other build was running during the runs.
Raw output is in `results/*.jsonl`.

## Results (Intel UHD, 1280×720, vsync off, opt-3 build, 2026-10-13 16:20–16:50)

| cubes | phase | interval avg / p95 (ms) | frames >16.7 ms | physics ms/step | awake avg |
|---|---|---|---|---|---|
| 0 | settled | 4.3 / 5.3 | 2 / 2305 | 0.34 | 0 |
| 0 | burst | 4.6 / 5.4 | 3 / 2193 | 0.29 | 0 |
| 200 | settled | 5.6 / 6.7 | 0 / 1799 | 0.59 | 0 |
| 200 | burst | 5.5 / 6.8 | 0 / 1811 | 0.85 | 25 |
| **500** | settled | **5.4 / 8.0** (run 2: 6.9 / 12.1) | 0–2 / ~1450–1840 | 1.3–1.9 | 0 |
| **500** | burst | **6.8 / 10.0** (run 2: 6.8 / 8.5) | 1–2 / ~1475 | 2.0–2.2 | 108–113 |
| 1000 | settled | 7.9 / 10.1 (run 2: 8.0 / 10.0) | 2–6 / ~1260 | 2.8–3.0 | 0 |
| 1000 | burst | 7.8 / 13.9 (run 2: 7.9 / 14.9) | 5–11 / ~1275 | 6.3–7.1 | 412–509 |
| 2000 | (never settles) | 304 / 363 | all | 18.7 | ~1850 |
| 1000, outlines off | settled / burst | 6.5 / 8.1, 6.4 / 12.5 | 0, 1 | 2.7, 6.1 | 0, 449 |
| 500, bloom on (HDR + `Bloom::NATURAL`) | settled / burst | 9.4 / 17.7, 9.3 / 17.3 | 61, 58 | 1.4, 2.4 | 0, 177 |
| 500 + 8 enemies, auto-punching player | play | 7.7 / 10.2 | 4 / 1296 | 2.8 | 151 |

With Chrome's default frame pacing (`gpu-vsync`; headless still runs at ~130 fps), CPU time per frame at 500 cubes was
3.4 / 4.9 ms while bursting. At 1000 cubes it was 6.0 / 10.8 ms.

**Shipping build (opt-z).** All the runs above used the opt-3 perf build. Later in the session, the iGPU got about
2× slower for every build: the empty box went from 4.3 ms to 8.3–9.3 ms, and the power plan was unchanged with the
laptop on AC. The likely cause is the desktop Chrome, which opened new processes at that time. So the opt-z build was
compared only against an opt-3 control run in the same degraded state, at 500 cubes settled. Opt-z took 14.1 / 27.1 ms;
opt-3 took 13.0 / 25.1 ms. Opt-z costs about +1 ms per frame (main schedule +0.9 ms) and physics per step about
+0.4–0.8 ms. Raw data: `results/igpu-optz.jsonl` and `results/igpu-control.jsonl`.

**Wasm size** (`game` profile: opt-z, fat LTO, `wasm-opt -Oz`, the same steps as `scripts/build.sh`):

| build | raw | gzip -9 |
|---|---|---|
| box3d spike (Bevy 3D + avian3d; no audio or gamepads; little game code) | 20,048,095 B | 6,149,715 B |
| 2.5D game, `dist/play/game_bg.wasm` as of 2026-10-13 16:00 | 21,529,926 B | 6,590,395 B |
| box3d perf build (opt-3, no LTO, no wasm-opt; used for the timings) | 52,865,461 B | 11,119,419 B |

The engine side of 3D costs no size: avian3d + parry3d is in the same range as avian2d, and `bevy_pbr` is already in
the game. A 3D port would land near today's ~6.6 MB gzipped. [inference: the spike has none of the game's code,
audio or gamepad support]

## Findings

- **500 resting cubes at 60 fps: pass**, with margin. The worst 60 Hz frame holds two physics steps, about
  5 ms of rendering plus 2 × 2.2 ms of physics, so it stays near 10 ms.
- **1000 cubes is the edge.** Bursting, a step costs 6–7 ms, so a frame with two steps lands at ~18 ms. At 2000 cubes
  a step costs ~19 ms, longer than the 15.6 ms fixed step, and Bevy's fixed loop falls into a death spiral (300 ms
  frames, and the pile never settles).
- **Sleeping is not free.** With every cube asleep, a physics step still costs ~2.5 µs per body: 3 ms at 1000 cubes
  against 0.34 ms empty. The cause is not profiled; the candidates are per-step upkeep of the broad phase, the collider
  tree or transforms.
- **Big connected piles don't stay asleep.** At 500 cubes, the pile re-slept within 3–5 s of a burst and stayed
  asleep. At 1000 cubes, the floor is covered and the pile is one connected island. A local burst woke 400–700 cubes,
  the pile took 14–30 s to re-sleep (or never did), and in two of four runs a fully re-slept pile **woke again by
  itself** with nothing touching it (170–470 awake). That matches the islands risk noted in game-research.md. The
  2.5D game already has the fix: freeze settled debris to `RigidBody::Static` and cap the dynamic count.
- **Rendering is cheap.** 1000 outlined cubes is 2000 draws of shared meshes, and frames stayed under ~8 ms when
  settled. Outlines cost ~1.4 ms at 1000 cubes. Bloom costs ~4 ms at 500 cubes and pushes p95 to ~17.5 ms. In 3D,
  the bloom budget from B2 doesn't fit next to the physics.
- **Outlines:** an inverted hull with a fixed world thickness (0.05 units per side) comes out at only ~1–2 px from this
  camera. It reads at ~0.1 units, but the hull's back faces show as an offset "shadow" rather than an even line. A real
  port wants a post-process edge pass or hand-tuned hulls.
- **No tunnelling seen**, with 6 substeps and flings up to ~8 u/s against a 1-unit floor and 0.5-unit walls.
- **Feel** was not judged from a headless browser.
