# Game research: Rumble Box, pile physics, Avian, Bevy wasm

Researched 2026-10-12 for the escape-the-box game (Bevy 0.19.1 + avian2d 0.7.0, wasm32, WebGL2).
Labels: **[verified]** = read in a primary source or reproduced locally; **[secondary]** = review, wiki or
store copy; **[unverified]** = could not confirm. The developer site `phackett.com/rumblebox/` returned
HTTP 521 and the Wayback Machine was offline on 2026-10-12, so the original readme and in-game help were not read.

## 1. Rumble Box (2005/2006)

**Provenance**
- DigiPen senior project (GAM 450) by Joseph Bourrie and Patrick Hackett. Released 2005-05-01. Awards:
  2006 Slamdance "Best Physics" (winner) and 2006 IGF finalist, Innovation in Game Design. [verified]
  ([DigiPen showcase](https://www.digipen.edu/showcase/student-games/rumble-box); [IGF 2006 finalists](https://igf.com/article/2006-igf-main-competition-finalists-announced))
- Two-person team over ~14 months. Hackett built the physics engine, animations, scripts and art. Bourrie
  built graphics, UI, combat, AI and color schemes. [verified]
  ([GameDev.net interview, 2006-02-26](https://gamedev.net/tutorials/industry/interviews/insert-coin-r2293))

**Core loop**
- The design premise, in the developers' words: "challenge the notion that enemies in video games disappear when
  defeated… if the characters were made of simple objects, we would be able to pile up hundreds of them." [verified] (interview)
- **There is no jump.** "The original concept contained more platforming elements such as a jump key, but when we
  discovered that the boxes tended to pile more terrain-like, without big columns of blocks, we removed the jump and
  focused on the combat mechanics." Players climb the pile by walking up its slope. [verified] (interview)
- **Spawning:** "enemies enter the level through doors that appear in the walls." [secondary]
  ([gHacks, 2007-01-12](https://www.ghacks.net/2007/01/12/game-for-the-weekeend-rumble-box/))
  Level scripts (`scripts/*.rbs`) spawn enemies in waves: `fodder 1, ninja 4`, `waitfor <frames>`,
  `wait <n enemies left>`, `repeat`, `randbomb <n>`, `bombstorm`. Spawn keywords are `fodder`, `thug`, `ninja`,
  `wildman` and `bodder`. That `bodder` means a bomb-head fodder is a guess. [secondary]
  ([Gamer's Journal modding guide, 2006](https://thegamersjournal.com/fighting/pc/rumblebox/mods.php))
- **Persistence:** defeated enemies break apart into the cubes and spheres they were made of, and the pieces stay in
  the level. Color separates live fighters from debris. [verified] (interview; [Home of the Underdogs](https://homeoftheunderdogs.net/game.php?id=5259))
- **Escape (win):** you exit once the debris reaches the top edge of the box. [secondary]
  ([Rarst review, 2010-08-14](https://rarst.net/games/rumble-box))
  On harder levels, killing enemies in one corner builds a climbable ramp faster. [secondary]
  ([bit-tech, 2009-12-01](https://bit-tech.net/blogs/free-games-i-like-rumble-box/1/))
- **Lose:** bomb blasts "can result in game over if you are standing too close." [secondary] (gHacks)
  MobyGames describes a race against a time limit, and Rarst plays "against time." The health and lives model is **[unverified]**.
- **Performance cap:** piles lagged slow GPUs. The developer said in comments that the game "no longer tracks every
  fallen piece." `PHYConfig.dat` caps blocks and spheres, and spawning too many crashes the game.
  [secondary] ([JayIsGames review and comments, 2006-03-28](https://jayisgames.com/review/rumble-box.php); modding guide)

**Controls and moves** (sources disagree on the grab key; the in-game help screen could not be checked)
- Arrow keys move. Double-tap a direction to dash. Space attacks. Ctrl grabs. [secondary] (gHacks)
  JayIsGames gives Alt as the grab, which swings the enemy in a circle as you move.
- Attack while holding an enemy does a **spin throw**. Attack while dashing does a dash attack or drop-kick. [secondary] (gHacks, JayIsGames)
- MobyGames lists moves "from the simple punch to the group-clearing spin throw." Hackett called the spin kick the
  best moment of development. [secondary/verified] ([MobyGames](https://www.mobygames.com/game/32162/rumble-box/), which returned 403 and was read via search snippet; interview)

**Enemies** (MobyGames and gHacks agree)
| Type | Description | Source |
|---|---|---|
| Fodder | "Harmless punching bags with legs" | gHacks |
| Thug | "Large and powerful, but slow in both action and wit" | gHacks |
| Ninja | "The fastest and most ruthless adversary" | gHacks |
| Wildman | "Primitive pack hunters"; "monkey-like" | gHacks; MobyGames |
| Bomb-heads (modifier) | Bomb in place of a head; explodes shortly after defeat; can be thrown into groups | MobyGames; interview ("the first time I threw a bomb-head into a group of enemies"); a focus tester suggested it |

Rarst adds that some group enemies "sacrifice their allies" to get you. [secondary]

**Scoring and bonus stages**
- Score multipliers rise the faster you defeat enemies. [secondary] (gHacks) Combos earn bonus points. [secondary] (Rarst)
- End-of-level score combines completion time, kills and best kill streak, and that score picks the bonus stage. [secondary] (JayIsGames)
- MobyGames lists **nine** bonus stages; JayIsGames counts **five**, possibly a different build. **[conflict, unverified]**
  Examples include dodging a rain of bombs, killing enemies with bombs, and surviving an onslaught in darkness.
  [secondary] (MobyGames, bit-tech, HotU) Readers report that `C` opens bonus and challenge modes, and the developer
  hinted at "watch the credits to the end." [secondary] (JayIsGames)

**Levels and presentation**
- A reader reports three levels, after which the game restarts in Challenge mode. [secondary] (JayIsGames)
  The total level count is **[unverified]**.
- There are 13 background color schemes with no textures, and scripts interpolate the color scheme over a level (`InterpEvents`). [verified] (interview; modding guide)
- The game is rendered in 3D ("geometric 3D brawler") but played on a floor inside a box. Camera behavior is
  **[unverified]**; the interview mentions only a months-long camera bug.

**Why it was fun, per reviewers**
- "Smooth animations and clever use of colors to differentiate living and dead guys". [secondary] (HotU)
- "Really easy to get stuck into", with Lego-like style. [secondary] (bit-tech)
- "One of the most action packed and intensive freeware games", with a "balance of playing against enemies, against
  arena you need to fill and against time." The real goal is the high score. [secondary] (Rarst)
- Criticisms: it lacks depth, gets repetitive, and has no upgrades. One reviewer wished for an upgrade system, and one
  reader found movement unresponsive. [secondary] (Rarst; JayIsGames)
- The developers' own lesson: they ran 60+ focus testers, and many of the best features, including bomb-heads, came
  from them. [verified] (interview)

## 2. Designs worth borrowing from

| Game | Mechanic to borrow | Source |
|---|---|---|
| **Super Crate Box** (Vlambeer, 2010) | Single-screen box. Enemies that reach the fire pit at the bottom respawn at the top "red and faster." Score is crates, not kills, and one hit ends the run. This is a clean punish for letting enemies slip past. | [Giant Bomb](https://www.giantbomb.com/games/3030-32945/) [secondary] |
| **Life Goes On** (Infinite Monkeys, 2014) | Corpses are tools: stepping stones over spikes, switch weights, and frozen ice-block platforms. Each body has a deliberate use. | [Nintendo.com](https://nintendo.com/games/detail/life-goes-on-switch); [Diabolical Plots review](https://www.diabolicalplots.com/game-review-life-goes-on-done-to-death) [secondary] |
| **Mount Your Friends** (Stegersaurus, 2014) | Players climb a tower of previous climbers' ragdolls under a turn timer, and the bodies are the terrain. | [Steam 296470](https://store.steampowered.com/app/296470/); [Destructoid](https://www.destructoid.com/?p=153301) [secondary] |
| **Tricky Towers** (WeirdBeard, 2016) | Physics stacking judged on the "tallest stable tower," with spells to disrupt or protect it. Pile stability is itself the gameplay. | [Steam 437920](https://store.steampowered.com/app/437920/Tricky_Towers/) [secondary] |
| **Gang Beasts** (Boneloaf, 2017) | Two-hand grab, lift and throw, with ring-outs off the arena. Loose, floaty physics is part of the comedy. | [Steam 285900](https://store.steampowered.com/app/285900/Gang_Beasts/); [Dexerto controls guide](https://www.dexerto.com/guides/gang-beasts-control-guide-for-xbox-ps4-pc-1725828) [secondary] |

## 3. avian2d performance (0.6/0.7)

**Releases.** avian2d 0.7.0 (2026-06-20) is "primarily just an update to Bevy 0.19". The 0.6.0 feature release
(2026-03-16) added a BVH broad phase, move-and-slide (KCC) and joint motors. Avian `main` (0.8.0-dev) still targets
Bevy 0.19, and Bevy 0.20.0 shipped 2026-10-08. [verified]
([v0.7.0](https://github.com/avianphysics/avian/releases/tag/v0.7.0), [v0.6.0](https://github.com/avianphysics/avian/releases/tag/v0.6.0), crates.io API)

**Sleeping and islands**
- Before 0.4, touching bodies could not sleep, so "piles never slept". Avian 0.4 (2025-10-13) added persistent
  simulation islands. An island sleeps only when all of its bodies can, and **waking any body wakes the whole
  island**. [verified] ([Avian 0.4 post](https://joonaa.dev/blog/09/avian-0-4))
- Defaults in 0.7.0: `SleepThreshold { linear: 0.15, angular: 0.15 }`, scaled by `PhysicsLengthUnit`, and
  `TimeToSleep = 0.5` s. `SleepingDisabled` opts a body out. [verified]
  ([SleepThreshold](https://docs.rs/avian2d/0.7.0/avian2d/dynamics/rigid_body/sleeping/struct.SleepThreshold.html), [TimeToSleep](https://docs.rs/avian2d/0.7.0/avian2d/dynamics/rigid_body/sleeping/struct.TimeToSleep.html))
- Open bugs: a sleeping body is not woken by a force (#901), and island bookkeeping panics in 0.7.0: "Head contact
  has no island" with ~50 bodies (#1025), and a stale island contact (#1076). PR #989 ("Decouple contacts and solver",
  merged 2026-07-19) may help but is unreleased: `main` is 33 commits ahead of v0.7.0 and there is no 0.7.1. [verified]
  ([#901](https://github.com/avianphysics/avian/issues/901), [#1025](https://github.com/avianphysics/avian/issues/1025), [#1076](https://github.com/avianphysics/avian/issues/1076), [#989](https://github.com/avianphysics/avian/pull/989))

**Solver settings that matter (0.7.0 defaults)**
- `SubstepCount` defaults to 6. More substeps make the simulation more accurate and cost more. The 0.4 post says
  4-12 substeps is common, and that 6 was not enough to keep a 3D stack of 10 cm cubes fully stable. [verified]
  ([SubstepCount](https://docs.rs/avian2d/0.7.0/avian2d/dynamics/solver/schedule/struct.SubstepCount.html); 0.4 post)
- `SolverConfig` defaults: `contact_damping_ratio 10.0`, `contact_frequency_factor 1.5`, `max_overlap_solve_speed 4.0`,
  `warm_start_coefficient 1.0`, `restitution_threshold 1.0`, `restitution_iterations 1`. [verified]
  ([SolverConfig](https://docs.rs/avian2d/0.7.0/avian2d/dynamics/solver/struct.SolverConfig.html))
- Tunneling: speculative collision is on by default, and `SweptCcd` is opt-in per body. Swept CCD is "primarily
  intended as a safety net" and costs more. Known artifacts are ghost collisions, missed contacts on thin, fast-spinning
  bodies, and energy loss on bounces. [verified] ([ccd module](https://docs.rs/avian2d/0.7.0/avian2d/dynamics/ccd/index.html))
- Wasm: 0.6's BVH tree optimization runs async, which broke wasm, so wasm uses a synchronous path (PR #936). [verified]
  ([#936](https://github.com/avianphysics/avian/pull/936))

**Benchmarks found. None is single-threaded wasm.**
- Bevy Ballpit (third party, **native**, `parallel` + SIMD on, one unspecified machine): Avian 0.5 2D spawned 9,562
  balls before the first frame under 50 fps. Avian 0.3 managed 4,140. [secondary] ([itch.io](https://molecularsadism.itch.io/bevy-ballpit))
- Avian 0.4 "Large Pyramid 2D", native i7-13700F: 12.29 ms/step on 1 thread and 8.92 ms on 6 threads. The body count
  is not stated. A separate `pyramid_2d` test with a 75-box base (~2,850 boxes, by arithmetic) was used for the solver
  benchmark. [verified] (0.4 post)
- **No browser or wasm figure for Avian was found. [unverified]** Our build already turns off Avian's `parallel` feature
  and compiles with `+simd128` (`.cargo/config.toml`). Avian's own `simd` feature is not enabled.

## 4. Bevy wasm dev iteration

- **No dynamic linking on wasm.** Reproduced locally (rustc 1.99.0): `cargo build --target wasm32-unknown-unknown`
  on a `dylib` crate fails with "target `wasm32-unknown-unknown` does not support these crate types". [verified]
- **No hot patching on wasm.** Bevy's `hotpatching` (subsecond via `dx serve`) is "Not supported in Wasm". [verified]
  ([Bevy 0.17 notes](https://bevy.org/news/bevy-0-17/)) The 0.19 notes do not mention it. ([Bevy 0.19 notes](https://bevy.org/news/bevy-0-19/))
- **Cranelift** "does not yet support Wasm builds". [verified] ([Bevy setup guide](https://bevy.org/learn/quick-start/getting-started/setup/))
- **Runners compared** [verified]:
  | Runner | Watch and reload | Notes |
  |---|---|---|
  | `bevy run web` (Bevy CLI v0.1.0-alpha.2, installed from git) | No watch command yet (issue #245 open) | Builds, runs wasm-bindgen and serves at 127.0.0.1:4000. Default profiles: `web` (inherits `dev`) and `web-release` (`opt-level="s"`, `strip="debuginfo"`). wasm-opt runs only for release builds. `bevy build web --bundle` produces a deployable bundle. |
  | `trunk serve` (0.21.14 stable; 0.22 is in RC) | Yes. Watches, rebuilds and auto-reloads the browser | Needs an `index.html`. |
  | `wasm-server-runner` (v1.0.0) as the cargo `runner` | Reloads the page when the server restarts | Pair it with `bacon` or `watchexec`. `cargo-watch` is archived and recommends those two instead. |
  Sources: [bevy_cli web docs](https://thebevyflock.github.io/bevy_cli/cli/web.html), [profiles.rs](https://github.com/TheBevyFlock/bevy_cli/blob/main/src/web/profiles.rs),
  [bevy_cli#245](https://github.com/TheBevyFlock/bevy_cli/issues/245), [trunk README](https://github.com/trunk-rs/trunk),
  [wasm-server-runner](https://github.com/jakobhellermann/wasm-server-runner), [cargo-watch](https://github.com/watchexec/cargo-watch).
- **Profiles that cut incremental time** (Bevy guide): `[profile.dev] opt-level = 1` plus
  `[profile.dev.package."*"] opt-level = 3`. Nightly `-Zshare-generics` and `-Zthreads=0` also help. The repo's
  `profile.game` (fat LTO, `codegen-units=1`, `opt-level="z"`) is a ship profile, and PLAN.md measures it at ~10 min cold. [verified]
- **Native Windows with the GNU host: blocked as-is.** Reproduced locally on rustc 1.99.0 `x86_64-pc-windows-gnu`. A
  `kind = "raw-dylib"` import (the mechanism `windows-sys`/`windows-link` use, which winit pulls in) fails with
  `error calling dlltool 'dlltool.exe': program not found`. Pointing `-Cdlltool` at rustup's bundled
  `self-contained/dlltool.exe` fails with `CreateProcess`, because rust-mingw ships no assembler. The installed
  `stable-x86_64-pc-windows-msvc` toolchain fails with no `link.exe`. [verified locally]
  Upstream issue [rust-lang/rust#103939](https://github.com/rust-lang/rust/issues/103939) is still open, and
  [getrandom#723](https://github.com/rust-random/getrandom/issues/723) reports the same failure. Known workarounds:
  put LLVM's `llvm-dlltool` on PATH renamed to `dlltool.exe`, install MSYS2 MinGW binutils, or install VS Build Tools
  and use MSVC. **[unverified here]**
  A third party reports that GNU `ld` can also emit a broken PE binary that LLD fixes.
  ([incan#1344](https://github.com/encero-systems/incan/issues/1344), [secondary])
  Even with a working toolchain, Bevy `dynamic_linking` on GNU fails with "export ordinal too large", reported through
  Bevy 0.16 ([bevy#4758](https://github.com/bevyengine/bevy/issues/4758)). That leaves a static native build plus LLD as the realistic native option.
- **Resolved 2026-10-12 (ticket A2): `scripts/native.sh`.** [verified locally] With `llvm-dlltool` and GNU ld 2.44, the
  game links but crashes at startup with `0xC0000005`. The cause is the unbound IAT slot for `kernel32!GetCurrentThreadId`:
  the fault address is in `.idata$6`, and no import descriptor lists the slot. A small `windows-sys` program doesn't
  reproduce it, so it depends on how the binary is composed. `-C link-arg=-fuse-ld=lld` does nothing, because the mingw
  gcc passes it to `ld` unchanged. What works is putting a copy of the toolchain's `rust-lld.exe`, named `ld.exe`, first
  on gcc's `-B` path. Called as `ld` with `-m i386pep`, LLD runs its MinGW driver; the PE header then reads linker version
  14.0 instead of 2.44. The exe then opens its window and initialises wgpu (Vulkan, RTX 4060). The cold dev build takes
  ~7.5 min.

## 5. Bevy 0.19 text and HUD features

- Bevy 0.19 switched its text engine from cosmic-text to **parley**. [verified] (0.19 notes)
- `bevy_text` is already in our build. The `2d_api` collection includes `common_api`, which includes `bevy_text`.
  `cargo tree -i bevy_text` on `-p game` shows it pulled in through `bevy_sprite` and `bevy_sprite_render`, along with
  parley 0.9, swash, harfrust, skrifa and fontique. `default_font` (an ASCII-only font, "+20kB") is also already on.
  **So `Text2d` HUD text needs no new features.** [verified]
  ([cargo_features.md @v0.19.1](https://github.com/bevyengine/bevy/blob/v0.19.1/docs/cargo_features.md), [Cargo.toml @v0.19.1](https://github.com/bevyengine/bevy/blob/v0.19.1/Cargo.toml))
- `bevy_ui` layout and nodes need `bevy_ui_render`. In `bevy_internal`, that feature enables `bevy_ui`, which enables
  `bevy_text` and `bevy_sprite`. The `ui` profile would also pull in `scene` and `picking`, which our manifest
  deliberately avoids. [verified]
- Size cost of adding `bevy_ui_render`, measured locally 2026-10-12. Method: `-p game --profile game`, a separate
  target dir, `--features bevy/bevy_ui_render` and no code changes, then wasm-bindgen and `wasm-opt -Oz` as in
  `scripts/build.sh`. [verified locally]
  | Build | wasm raw | gzip -9 | cold build |
  |---|---|---|---|
  | current features | 16,457,120 B | 5,099,001 B | 535 s |
  | + `bevy_ui_render` | 17,806,910 B (**+1.35 MB, +8%**) | 5,478,639 B (**+0.38 MB, +7%**) | 597 s |
  No upstream size figure exists beyond `default_font`'s 20 kB.

## 6. 2.5D rendering cost (B2 spike)

Measured locally 2026-10-12 (#11). The spike code lives on branch
[`spike/2.5d`](https://github.com/DavidSlayback/slayback-landing-page/tree/spike/2.5d) (`crates/game/examples/look.rs`,
raw results in `examples/look/results/`) and is not merged. The scene is the 1280×720 box as 3D walls and floor
(40 × 22.5 × 6 units), a 30° perspective camera tilted down at the whole box, and N figures. Each figure is 7 shared,
flat-shaded meshes: a box torso, a sphere head, box limbs and an emissive visor. It also has one directional light,
point lights, HDR, `Bloom::NATURAL`, `Tonemapping::AcesFitted` and emissive rim strips in the Netrunner palette.
The figures walk and bob. [verified locally]

**Size.** Method as in section 5: `--profile game`, wasm-bindgen 0.2.129, then `wasm-opt -Oz` with the flags in
`scripts/build.sh`. The game binary was rebuilt with no code changes, only the features.

| Build | wasm raw | gzip -9 |
|---|---|---|
| today's game (section 5 figure, not rebuilt) | 16,457,120 B | 5,099,001 B |
| + `bevy_pbr` | 19,281,790 B (**+2.82 MB, +17%**) | 5,933,668 B (**+0.83 MB, +16%**) |
| + `bevy_pbr` + `tonemapping_luts` + `zstd_rust` | 20,052,031 B (+0.77 MB more) | 6,501,216 B (**+0.57 MB more**) |
| `look` example alone (`bevy_pbr`, no Avian) | 16,779,860 B | 5,261,663 B |

- `bevy_pbr` is the only feature 3D needs. It pulls in `bevy_light` and `bevy_material`. HDR (`Hdr` on the camera),
  bloom and tonemapping are already in `bevy_core_pipeline` and `bevy_post_process`. The cost lands even with no
  3D code, because `DefaultPlugins` registers the PBR plugins.
- `tonemapping_luts` is needed only for TonyMcMapface, AgX and Blender Filmic. Without it, those modes log an error.
  ACES fitted and the Reinhard modes need no LUT.
- The building and linking were shared with the example, which added a wasm-only `web-sys` dev-dependency
  (`Window`, `Location`, `console`). Those are bindings, and anything unused is stripped. [inference]

**Frame time.** Headless Chrome 154 (`channel: 'chrome'`, Playwright 1.63), ANGLE D3D11 on this laptop (i7-13650HX).
Renderers (`UNMASKED_RENDERER_WEBGL`):
- **Intel UHD Graphics (0xA78B)**, the default adapter. This is the integrated-GPU target.
- **NVIDIA GeForce RTX 4060 Laptop GPU**, with `--force_high_performance_gpu`.

The vsync and frame-rate limits are off (`--disable-gpu-vsync --disable-frame-rate-limit`), so the frame interval is
the throughput. The first 180 frames are a warmup, then each run is 600 frames, and a run is recorded after a second
600-frame window. "Main" is the CPU time of the main schedule, measured from `First` to `Last` with
`bevy::platform::time::Instant`. CPU time to the end of the render world was within 0.5 ms of the interval: WebGL
blocks on the GPU inside the frame. No cargo build was running.
Defaults: shadows on (1 cascade), bloom on, MSAA 4×, 2 point lights, lit `StandardMaterial`.

| Variant (Intel UHD, 1280×720) | interval avg / p95 (ms) | main avg (ms) |
|---|---|---|
| empty box (no figures, point lights, shadows or bloom; MSAA off) | 5.0 / 5.5 | 0.37 |
| 20 figures, defaults | 10.5 / 11.9 | 0.51 |
| **30 figures, defaults** | **10.6 / 12.5** | 0.51 |
| 60 figures, defaults | 11.2 / 12.9 | 0.62 |
| 120 figures, defaults | 11.9 / 23.6 | 0.81 |
| 30, shadows off | 10.0 / 11.4 | 0.47 |
| 30, bloom off | 8.7 / 9.7 | 0.53 |
| 30, MSAA off | 10.0 / 11.3 | 0.53 |
| 30, unlit materials, shadows off | 9.9 / 10.9 | 0.50 |
| 30, no point lights | 8.6 / 9.9 | 0.50 |
| 30, 6 point lights | 14.7 / 16.6 | 0.59 |

| Variant (Intel UHD, **1920×1080**) | interval avg / p95 (ms) |
|---|---|
| empty box (as above) | 11.3 / 12.1 |
| 30, defaults | 22.4 / 24.5 |
| 30, no point lights | 17.6 / 19.3 |
| 30, no point lights, MSAA off | 16.4 / 18.3 |
| 30, no point lights, MSAA off, bloom off | 12.4 / 16.8 |
| 120, no point lights | 18.9 / 37.2 |

| Variant (RTX 4060, 1280×720) | interval avg / p95 (ms) |
|---|---|
| empty box | 1.5 / 1.9 |
| 30, defaults | 2.5 / 3.5 |
| 120, defaults | 3.6 / 4.5 |
| 30, 6 point lights | 3.1 / 3.7 |

With Chrome's default frame pacing (vsync flags left on; headless has no display), CPU per frame on the Intel GPU was
3.2 ms avg (p95 4.8) for 30 figures and 3.9 ms (p95 5.0) for 120. The interval stayed at 10.5 and 11.5 ms, so the
remaining time is GPU work.

**Reading.** [inference from the tables]
- On the integrated GPU the look is bound by fill and post-processing, not by figure count. Going from 20 to 120
  figures (840 meshes, batched through shared meshes and materials) adds ~1.4 ms. Pixels cost far more: at 1080p
  the empty HDR box alone takes 11.3 ms.
- Each point light costs ~1 ms at 720p. Bloom costs ~2 ms at 720p and ~4 ms at 1080p. MSAA 4× and the
  one-cascade directional shadow cost ~0.6 ms each. Unlit materials saved nothing measurable.
- The 120-figure p95 spikes (23.6 ms, and 37.2 ms at 1080p) suggest occasional stalls. 30–60 figures stayed smooth.

**Decision for D1 (the look).**
- **Features:** today's set + `bevy_pbr` only. No `tonemapping_luts`/`ktx2`/`zstd` (+0.57 MB gz), no
  `bevy_anti_alias`, no `bevy_gltf`/`scene`. Tonemap with `Tonemapping::AcesFitted`.
- **Lighting:** one directional key light with a single-cascade shadow map, low global ambient, and neon from
  emissive materials plus bloom. No point lights by default. At most one short-lived point light for explosions.
  Materials are lit `StandardMaterial` on flat-normal meshes (`with_duplicated_vertices().with_computed_flat_normals()`).
- **Quality:** bloom and MSAA 4× on at full quality. Automatic scaling drops bloom first, then MSAA, then shadows.
- **Resolution:** cap the backbuffer at about 1280×720 physical pixels and let CSS upscale the canvas. At 1080p the
  full look misses 60 fps on the integrated GPU (22 ms). D1 must verify how to cap it in Bevy (window resolution and
  scale factor vs `fit_canvas_to_parent`). [unverified]
- **Budget D1 must stay within:**
  - `game_bg.wasm` ≤ **6.0 MB gzipped** (5.93 MB measured with `bevy_pbr` and no new code). With `bevy_ui_render`
    added later, ≤ ~6.4 MB.
  - Render cost ≤ **11 ms per frame on the Intel UHD at the capped resolution** (measured 10.6 ms with 30
    figures), which leaves ~5 ms of a 16.7 ms frame for physics, particles and game logic.
  - Rendering-related main-schedule CPU ≤ 1 ms with 30–60 figures.

## Implications for the design (inference, not sourced fact)

- **Pile and debris budget:** with islands, one big connected pile wakes as a unit whenever anything touches it, and
  Rumble Box itself stopped tracking every piece. Plan for debris to **settle and then freeze**: switch it to
  `RigidBody::Static`, or bake it into a few merged static colliders such as a polyline heightfield. Freeze after it
  sleeps or after N seconds, and keep only a bounded set of awake debris (for example a few hundred) as dynamic bodies.
  Confirm the cap with a browser stress spike: N boxes, substeps 4 vs 6, time per frame in Chrome. No wasm benchmark exists to lean on.
- **Escape rule:** Rumble Box had no jump, and its piles formed ramps rather than columns. Either keep the player
  jumpless and climb ramps with Avian 0.6+ move-and-slide, or add a jump and accept column-stacking exploits. The escape
  test can be "the pile height under the player reaches the rim" rather than tracking individual boxes.
- **Enemy roster:** the original's axes were punching bag, slow tank, fast striker, pack swarm, plus a bomb modifier
  that also serves as a crowd-clear tool. Ship Fodder first, then a Thug, then a Ninja. Treat bomb-heads as a
  modifier on any type, not a separate class.
- **Progression and depth:** reviewers praised feel and score-chasing and faulted repetition. Cheap levers are score
  multipliers for kill speed and streaks, short alternate-goal bonus stages, and box variants (size and color scheme).
  Super Crate Box's "escapees come back faster" is a cheap pressure mechanic.
- **Controls:** the original used about three buttons (attack, grab, dash by double-tap) plus contextual combos (spin
  throw, dash attack). That supports keeping the J/K/mouse scheme small and adding grab as the third verb.
- **Dev loop:** wasm-only rules out dylib and hot patching, so the gains are (a) a `web` or dev profile without fat
  LTO for iteration, keeping `game` for deploys only, (b) `trunk serve` or `wasm-server-runner` + `bacon` for
  auto-reload, and (c) keeping rules in `game-core` (no Bevy, tests run natively today). Native Bevy runs need one of
  the toolchain fixes in section 4 first.
- **HUD:** use `Text2d` on the camera, or a screen-space sprite layer, at no new size cost. Adopt `bevy_ui` (+0.38 MB
  gzipped) only if menus or layout need it. PLAN.md's "no HUD text because ui/text features aren't enabled" is
  half-wrong: text is already compiled in.
- **Avian pinning:** stay on 0.7.0 with Bevy 0.19.1. Watch for 0.7.1 or 0.8 carrying #989 if island panics show up in
  pile-heavy scenes.
