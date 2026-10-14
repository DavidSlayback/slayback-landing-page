# Spec: *Escape the Box* in 3D (v1)

Sources: `docs/planning/game-decisions.md` (round 5, Q46–Q64), `docs/research/game-research.md` (§1 Rumble Box's verified camera and look; §7 the full-3D spike), `docs/PLAN.md` ("Look and impact"). It supersedes the 2.5D game parts of spec #5. The homepage, release-switch and playtest-gate parts of #5 still stand.

## Problem Statement

The 2.5D game works, but it does not look or feel like the game it is inspired by. A side view can't produce Rumble Box's signature: a heap of cubes spreading across the floor of a box seen from above, with fighters exploding into blocks that tumble in 3D. David's verdict after the look and impact passes: "Take inspiration from rumble box for the visuals. We may need to go to 3d." The open question was whether a browser on an integrated GPU can carry that. The full-3D spike answered it: 60 fps at 500 resting cubes, with no wasm size cost.

## Solution

Rebuild the game's shell in 3D: an open-topped box seen from above at a 3/4 angle, under a fixed camera.
- **Fighting:** the player moves in eight directions over the floor and fights waves of cube-built enemies with punch, kick, dash-kick, grab and spin-throw. Light aim assist makes blows and throws find their target despite the 3/4 view's depth.
- **Debris:** defeated enemies burst into their cubes. These tumble, then freeze into a growing heap.
- **Escape:** you climb the heap by walking up it, without jumping, and escape when the pile under you reaches the rim.
- **Look:** a hybrid of Rumble Box's forms (flat-shaded saturated cubes, outlines, comic impact bursts) with the site's three colour schemes. The arena and debris are muted; live fighters are vivid.

Everything the 2.5D game has today ships in 3D v1 before the cohort playtest: three enemy types, bomb-heads, doors, sound, gamepad, comfort, score, screens and demo mode. The 3D game is built beside the 2.5D one and replaces it when it reaches parity.

## User Stories

**Players: the arena and moving**
1. As a player, I want to see the whole box at once from above, so that I can read where enemies, doors and the pile are.
2. As a player, I want the box to fill my widescreen, so that fighters are large enough to read.
3. As a player, I want the front wall drawn as see-through glass, so that it never hides the fight behind it.
4. As a player, I want to move in eight directions with the keyboard or a stick, so that moving across the floor feels natural.
5. As a player, I want my fighter to face the way I last moved, so that I always know where my blows will go.
6. As a player, I want a bright ring on the floor under my fighter, so that I never lose myself in a crowd.
7. As a player, I want to walk up the pile without jumping, stepping up onto debris automatically, so that climbing is about building the right heap.
8. As a player, I want the pile under me to carry me out when it reaches the rim, so that escaping is the visible result of my fighting.

**Players: fighting**
9. As a player, I want punch, kick and dash-kick to hit a cone in front of me, so that blows land where I expect.
10. As a player, I want a swing to snap toward the nearest enemy in that cone, so that the 3/4 view's depth never makes me whiff on an enemy that looked lined up.
11. As a player, I want every hit to shake the view, flash a comic star burst and squash the enemy I struck, so that hits feel heavy.
12. As a player, I want kills to freeze longer and explode the enemy into its cubes, so that every kill lands as a beat.
13. As a player, I want each kill's points to float up from where it happened, with my streak, so that I feel the score building.
14. As a player, I want to grab an enemy and spin-throw them flat into a crowd, aimed toward the biggest group, so that I can bowl enemies over.
15. As a player, I want thrown bodies to hurt whatever they hit, so that throwing is a crowd-control tool.
16. As a player, I want bomb-heads that I can throw into groups before they blow, so that I can set up big chain reactions.
17. As a player, I want doors to warn me before enemies enter, so that I can position myself.

**Players: enemies**
18. As a player, I want fodder that head straight for me but keep apart from each other, so that crowds spread out and look alive.
19. As a player, I want thugs that plough straight in and shrug off light blows, so that they feel heavy and must be read.
20. As a player, I want ninjas that circle to flank me, so that they keep me moving.
21. As a player, I want enemies to telegraph their attacks, so that I can react.
22. As a player, I want every enemy built from cubes in its own vivid colour, so that I can tell types apart at a glance, colour-blind safe through shape.

**Players: the run**
23. As a player, I want each box's rim higher and its timer shorter, so that the run escalates.
24. As a player, I want the colour scheme to change each box, starting from the site's, so that progress is visible.
25. As a player, I want one life, a timer, a score with streak multipliers and a best score kept locally, so that I chase a better run.
26. As a player, I want a start screen with the controls, pause on Esc, and a game-over screen with my score and best, so that the run has a clear frame.
27. As a player, I want sound for hits, kills, blasts and doors that I can mute, so that the fight has weight without forcing audio on me.
28. As a player with a gamepad, I want every verb on a standard gamepad with nothing to set up, so that I can play from the couch.
29. As a player who prefers reduced motion, I want no screen shake or squash, softer flashes and fewer sparks, so that the game is comfortable.
30. As a player on a weaker machine, I want the game to shed effects rather than drop frames, so that it stays playable.
31. As a player who switches tabs, I want the game to pause, so that I don't lose a run.

**Visitors and David**
32. As a phone visitor, I want the clip page instead of a game download, so that I see the game without a broken experience.
33. As a visitor with the release switch off, I want no trace of the game, so that the homepage stays clean until the playtest passes.
34. As David, I want each 3D slice playable on a preview (rc-2) while the 2.5D game stays on rc-1, so that I can follow progress and compare.
35. As David, I want a demo mode with URL knobs (scripted fight, slow motion, debris, enemy type, bombs), so that I can record clips and show the game.
36. As David, I want the 3D game to load no larger than the 2.5D one (~6.6 MB gzipped), so that first play stays quick.
37. As David, I want 60 fps on an integrated-GPU laptop at the debris cap, so that the showpiece runs on cohort-mates' machines.
38. As a cohort playtester, I want the 3D game to be the version I test, so that my notes shape what ships.
39. As a maintainer, I want one game crate after the switch, so that there is one codebase to keep working.

## Implementation Decisions

- **Structure:** a new `game3d` crate beside the 2.5D `game`, both on the shared engine-free `game-core`.
  - **Shared rules stay:** health, score, run (box specs, timer, scheme rotation), enemy kinds and styles, doors, bomb timing, quality scaling, combat timing, hit-stop including the finisher freeze.
  - **Rules that change:** geometry-bound rules move to 3D-aware forms. Hit reach is a cone around a yaw, debris shapes are 3D, and the escape test measures the pile height under the player.
  - **Lifecycle:** `game` is deleted at the switch-over, and `/play` serves `game3d` with no fallback. rc-1 remains as the 2.5D archive.
- **Engine:** Bevy 0.19.1 with avian3d 0.7.0, pinned as a pair.
  - Rendering stays WebGL2: one directional light (Bevy's WebGL2 limit), no shadow map, no bloom. Bloom doesn't fit next to the physics at the debris cap.
  - Comic effects are emissive geometry.
- **Camera:** fixed, framing the whole box at a 3/4 angle. A box ~3:2 in floor shape.
- **Walls:** the front wall collides at full height and renders as glass. Doors are in the back and side walls only.
- **Player controller:** kinematic move-and-slide over the floor and pile, with an automatic step-up of about half a cube and no jump.
  - Facing is the last move direction (8-way input, smooth yaw).
  - Attacks test a cone in front. Aim assist turns a swing toward the nearest enemy inside the cone; throws aim toward the biggest group within about 30°.
- **Enemies:** steering behaviours (seek, separation, wall avoidance). Ninjas flank by circling; thugs keep straight lines. Same brains, timings and telegraphs as today.
- **Figures:** cube-built, Rumble Box style, roughly 11 parts for fodder, 14 for thugs and 9 for ninjas. On defeat the parts burst out with spin and become debris.
- **Debris budget:** at most 300 moving pieces at once. Settled debris freezes to static after about 1.5 s, with a total cap set by measurement (target about 1,500). The freeze is mandatory in 3D: touching cubes share one sleep island, and an unfrozen pile wakes as a whole (research §7).
- **Look:** flat-shaded cubes with edge-detection outlines.
  - The scheme's muted tone colours the arena and debris; its vivid accents colour live fighters.
  - The player is marked by a floor ring in the main accent.
  - The impact pass carries over: trauma shake, star bursts, squash, finisher freeze, floating score numbers.
- **Build and deploy:** a build switch selects the 3D game for `/play`. `scripts/deploy.sh rc 2` ships the 3D preview, gated by the browser harness. The release switch, the clip page and phone detection are unchanged.
- **Process:** no CI (out of Actions minutes). Merges are gated by the local pre-commit hooks, and each merge deploys rc-2 from the machine.

## Testing Decisions

- **Good tests** exercise behaviour through public seams (a rule's inputs and outputs, a scripted scenario's observable outcome), never internal system order or private state.
- **Rules:** `game-core` stays the home for pure rules, tested once. The new 3D-aware rules (cone reach, aim-assist target choice, throw-group choice, pile height under a point, debris freeze and cap) get unit tests there.
- **Scenarios:** the 26 headless scenarios are ported to `game3d`, each ticket bringing its own. They step the game without a renderer and with scripted input, and cover climbing and escape, crowds not pinning the player, slow frames not whiffing blows, and piles staying stable at 30 fps.
- **Browser harness:** the existing Playwright smoke tests run against rc-2 builds through `deploy.sh`: the game loads with no console errors, and a scripted run climbs out to box 2.
- **Performance gates:** frame time at the debris cap on the Intel UHD. Measured with the B2/box3d harness: headless Chrome on the real GPU, vsync off.
- **Prior art:** `crates/game/tests/scenarios.rs`, the `game-core` module tests, `tests/e2e/smoke.spec.js`, and the box3d spike's bench scripts on branch `spike/3d`.

## Out of Scope

- A following or zooming camera (Q48 defers it until boxes outgrow one screen).
- Jumping, twin-stick aiming, touch controls on phones.
- Pathfinding or flow fields for enemies.
- New enemy types (the wildman), upgrades, bonus stages.
- Bloom and shadow maps.
- Keeping the 2.5D game as a selectable mode.

## Further Notes

- Tickets are tracer-bullet slices that each leave rc-2 deployable. Two measurement spikes (outlines, frozen-debris cap) gate the look and the escape slices.
- Risks, from research §7: pile sleep islands waking whole heaps (mitigated by freezing); spontaneous re-waking of settled piles (freezing again); the cost of outlines; climbing feel on irregular piles.
