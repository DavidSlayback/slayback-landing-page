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

# Follow-up spikes S1 (#59) and S2 (#60), branch `spike/3d-measure`, 2026-10-13 21:00–22:15

Same harness, build (opt-3) and Chrome flags as above (Chrome 155 this time, same Intel UHD 0xA78B adapter).
Another agent was compiling on the machine, so `bench/measure.sh` gates every variant: it waits for 60 s with no
`cargo`/`rustc` process, watches for one during the run (a hit sends the row to `results/*.invalid.jsonl` and retries),
and runs the empty-box baseline (`cubes=0`, hull outlines, MSAA 4×) first and after every 4 variants, waiting when
its settled average exceeds 5.5 ms. All rows in `results/s1-outlines.jsonl`, `s1-passcost.jsonl` and
`s2-frozen.jsonl` ran with no compile seen. **Baselines:** 4.40–4.75 ms settled avg (11 runs; the clean 16:20 run
was 4.3 ms). One baseline at 21:13 read 7.2 ms and was rejected (it is the first row of `s1-outlines.jsonl`); the
first S1 attempt overlapped a compile and is kept only as `s1-outlines.invalid.jsonl`.

```sh
python -m http.server -d web 8094 &
bash bench/measure.sh results/s1-outlines.jsonl "bench=1&burst=1&cubes=500&outline=0&edge=2&es=0.008" ...
```

## S1: edge-detection outlines under WebGL2 — **go**

**Technique** (`src/outline.rs`, `src/outline.wgsl`): one fullscreen pass in `Core3dSystems::PostProcess` (before
tonemapping) that reads the camera's own depth texture. Reverse-Z NDC depth is affine in screen space across any
plane, so its second difference is ~0 inside a flat face and large at creases (slope change) and silhouettes (depth
jump). The pass takes the largest relative second difference over 4 directions and darkens by `smoothstep`:

- creases: sampled 1 px out, threshold 0.0008 → **~2 px lines**;
- silhouettes: edges above 0.008, sampled 2 px out → **~4 px lines** (`edge=2&es=0.008`, 17 depth reads per pixel).

Depth alone finds every edge a normal prepass would on flat-shaded cubes, so no prepass is needed.

**What WebGL2 forces** (Bevy 0.19.1):
- Bevy's depth prepass texture is useless here. The prepass copies depth with `copy_texture_to_texture`, which fails
  on WebGL2 (`bevy_core_pipeline` `prepass/node.rs`, bevy#9710). The main depth texture works once the camera sets
  `Camera3d::depth_texture_usages` to `RENDER_ATTACHMENT | TEXTURE_BINDING`.
- **MSAA must be off.** GLSL ES 3.00 cannot sample multisampled textures.
- Depth must be read through a NEAREST sampler (`textureSampleLevel`). Plain `textureLoad` returns 0 everywhere,
  because wgpu's GL backend leaves a texture read without a sampler on the texture's own LINEAR filter. That makes a
  `Depth32Float` texture incomplete.
- The depth texture binds as `Float { filterable: false }` rather than `Depth`, so no shadow sampler is needed.

**Cost** (interval avg / p95 ms; `>16.7` = frames over 16.7 ms in a 10 s window):

| config | 0 settled | 500 settled | 500 burst | 1000 settled | 1000 burst |
|---|---|---|---|---|---|
| no outlines, MSAA 4× (`outline=0`) | 4.6 / 5.1 | 5.9 / 6.8 | 5.9 / 7.5 | 6.4 / 8.8 | 9.4 / 16.2 (36 >16.7) |
| no outlines, no MSAA (`msaa=1`) | 3.2 / 4.1 | | | 5.0 / 8.2 | 7.3 / 13.8 |
| hull outlines, MSAA 4× (old default) | 4.4–4.8 / 5.1–5.6 | 6.8 / 8.6 | 7.1 / 8.4 | 6.4 / 9.6 | 9.8 / 14.5 |
| **edge outlines, no MSAA** (`edge=2&es=0.008`) | **6.0 / 6.6** | **6.9 / 8.2** | **6.9 / 8.0** | **7.3 / 8.7** | **8.9 / 15.2** (6) |
| edge + FXAA (`fxaa=1`) | 7.0 / 7.6 | 8.6 / 9.6 (22) | 8.4 / 9.4 (19) | 9.0 / 13.1 (36) | 8.9 / 14.2 (11) |

The 1000-cube burst column is physics-bound (8.3–9.2 ms per step in every config), so it says little about outlines.

**Cost of the pass by itself** (`s1-passcost.jsonl`, 0 cubes, no MSAA = 3.3 ms):
- an empty fullscreen pass with 1 depth read: +1.0 ms;
- 9 reads (`edge=1`): +1.6 ms;
- 17 reads (`edge=2&es`): +2.1 ms (+2.8 ms in the matrix run).

Turning MSAA off pays most of it back. Against today's MSAA look with no outlines, edge outlines add +1.4 / +1.0 /
+0.9 ms at 0 / 500 / 1000 cubes (settled). Against the hull outlines they add +0.1 ms at 500 and +0.9 ms at 1000.

**Verdict:** go. Use edge outlines with MSAA off and no FXAA, 2 px silhouettes plus 1 px creases. Expect about +1 ms
over the hull build.
- **FXAA: no.** It is a second fullscreen pass (+1–1.7 ms) and puts 20–36 frames per 10 s over 16.7 ms at 500–1000
  cubes. The bold lines hide most of the aliasing anyway.
- **Optional saving:** reading only the 4 axis neighbours (9 reads instead of 17) would save ~0.5 ms.
- **Blocking:** nothing.

**Screenshots** (1280×720, real GPU):
- `results/s1-edge-0.png`, `s1-edge-500.png`, `s1-edge-1000.png`, `s1-edge-1000-burst.png`;
- `s1-none-500.png` and `s1-hull-500.png` for comparison;
- `s1-crops.png`: none / hull / `edge=1` / chosen, the same region at 2×.

## S2: frozen-debris cap — **1,800 pieces (1,500 frozen + 300 moving)**

`freeze=1.5`: a piece whose |v| < 0.2 and |ω| < 0.5 for 1.5 s becomes `RigidBody::Static`. The heap (`cubes=N`) is
dropped in batches with at most 300 pieces falling at a time. Once the heap is frozen, `movers=300` never-frozen
pieces are added. Every burst teleports all 300 movers into a 7 × 7 column 4–8.5 units above a random point of the
heap and flings them out with spin, so the burst window is 300 pieces tumbling onto the heap the whole time (288–294
awake on average). Edge outlines were on (the S1 look), so these are the shipping render costs.

| frozen + moving | settled avg / p95 (>16.7) | **burst avg / p95 (>16.7)** | phys ms/step settled / burst | re-settle, awake after 5 s quiet |
|---|---|---|---|---|
| 500 + 300 | 7.1 / 8.6 (1) | 7.1 / 9.1 (1) | 1.4 / 3.5 | 4.5 s, 14 movers |
| 1000 + 300 | 7.7 / 8.8 (2) | 7.7 / 10.7 (2) | 2.0 / 4.2 | 7.3 s, 22 |
| **1500 + 300** | **8.3 / 10.2 (20)** | **8.2 / 11.6 (7)** | 2.5 / 4.4 | 9.7 s, 1 |
| 2000 + 300 | 8.7 / 16.3 (55) | 8.7 / 12.7 (34) | 2.8 / 4.6 | 4.6 s, 29 |
| 3000 + 300 | 9.6 / 18.9 (70) | 9.6 / 17.4 (60) | 2.6 / 4.4 | 21.7 s, 130 |
| 3000 + 0 | 9.3 / 18.3 (68) | (nothing to burst) | 0.66 | 0 |

Unfrozen for comparison (box3d run above): 1000 sleeping dynamic cubes cost 2.8–3.0 ms per step settled and
6.3–9.2 bursting, and 2000 never settled.

- **Freezing fixes the physics.**
  - A static piece costs ~0.1 µs per step: 3000 frozen pieces cost 0.66 ms per step (0.32 empty), against ~2.5 µs per sleeping
    dynamic body.
  - With 300 pieces bursting, a step stays at 3.5–4.6 ms from 500 to 3000 frozen pieces. Two steps per frame fit easily.
- **Rendering sets the cap, not physics.**
  - Each extra 1000 pieces costs ~1 ms of average frame time.
  - From 2000 frozen pieces the p95 jumps to 16–19 ms with nothing moving (the `3000 + 0` row matches `3000 + 300`).
    So the cost is drawing and extracting thousands of entities, not avian.
- **Cap:** 1500 frozen + 300 moving is the largest measured total that holds 60 fps bursting with margin (p95 11.6 ms,
  ~5 ms headroom for the opt-z build's ~1 ms and the game's own systems). 2000 + 300 has no margin.
  - Settled, 1.7% of frames ran 17–18 ms.
  - If the full game eats more than ~3 ms, fall back to 1000 + 300.
  - To raise the cap, merge frozen debris into a few static meshes (fewer draws and less extraction); the physics
    allows ~3000.
- **Frozen heaps stay quiet.** Frozen pieces cannot wake, and the heap's physics is flat at ~0.1 µs per piece. Only the
  300 never-frozen movers re-wake: 1–130 were awake 5 s after re-settling. In the game, movers freeze too, so this
  goes away. Avian wakes a body's island when its `RigidBody` changes (`wake_on_replace_rigid_body`), so each freeze
  briefly wakes the neighbours. Those neighbours are at rest and freeze in turn.
- Screenshots: `results/s2-heap-1500-burst.png` (1500 frozen, 291 movers in the air), `results/s2-heap-3000.png`.
