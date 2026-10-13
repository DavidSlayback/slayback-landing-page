# Spec: homepage launch and *Escape the Box* v1

Sources: `docs/planning/game-decisions.md` (grilling rounds 1–4, Q1–Q40, plus the accepted assumptions), `docs/research/game-research.md`, `docs/PLAN.md`.

## Problem Statement

David has a finished, shareable hero (welding-spark physics flowing around his name), but no public site. The landing page has no real content: the projects, contact and résumé are placeholders, nothing is deployed, and there are no analytics. The game exists only as a two-slice prototype (movement, punch and kick, one enemy type), built without a design, so it isn't yet something a visitor would remember. Iterating on it is also slow: each change costs a ~10-minute size-optimised build plus a scripted browser run.

## Solution

Launch the homepage first: deploy it to Cloudflare Pages with real content and cookieless analytics, keeping the game hidden. Then build *Escape the Box*, a 2.5D, Rumble Box–inspired brawler that is a 3–5 minute portfolio showpiece:
- **Look:** flat-shaded figures made of primitives, in a tilted side view, with the site's neon schemes and spark particles.
- **Loop:** you fight waves of enemies who enter through wall doors. Defeated enemies fall apart into their own solid parts, and you escape each box by climbing that pile until the height under you reaches the rim.
- **Run structure:** one life, endless boxes, each harder and shorter on time, ending with a score screen and a local best score.
- **Release gate:** it ships behind a "Play" link only after a cohort playtest.

Development first gets a fast iteration loop: a no-LTO dev build with auto-reload, plus native runs.

## User Stories

**Homepage visitors**
1. As a visitor, I want the homepage to load fast and show the spark hero immediately, so that my first impression is the work itself.
2. As a visitor, I want the name and every piece of text to stay readable however many sparks pass, so that I can actually read the page.
3. As a visitor, I want a real projects list (name, one-liner, link), so that I can see what David has built.
4. As a visitor, I want GitHub, LinkedIn, an email address shown as text, and a résumé link, so that I can follow up.
5. As a visitor without WebGL or WebAssembly, I want a plain readable page, so that I am never shown a blank hero.
6. As a visitor who prefers reduced motion, I want a still hero, so that the page doesn't animate at me.
7. As a visitor, I want to pick a colour scheme and have it remembered, so that the site looks the way I like on return.
8. As a cohort-mate, I want a public URL I can open without a Claude account, so that I can see the site David shares.

**David as site owner**
9. As David, I want each push to `main` to deploy automatically after CI passes, so that I never deploy by hand.
10. As David, I want cookieless page-view analytics with no consent banner, so that I can tell whether people visited.
11. As David, I want the site on a `*.pages.dev` address now and switchable to a custom domain later with DNS only, so that buying a domain doesn't block launch.
12. As David, I want the Play link hidden until the game passes its playtest, so that visitors never see an unfinished game.

**Reaching the game**
13. As a visitor, I want a visible "Play" link and a small call to action in the hero once the game ships, so that I can find it.
14. As a curious visitor, I want a hidden easter-egg way into the game, so that there's a reward for poking around.
15. As a phone or tablet visitor, I want a looping gameplay clip and a "Play on a computer" note instead of a broken game, so that I still see what it is.

**Players: starting and controls**
16. As a player, I want a "click to start" overlay that shows the controls, so that I know how to play before anything attacks me.
17. As a player, I want sound off by default with a one-key toggle, so that the page never surprises me with noise.
18. As a player, I want keyboard and mouse controls, so that I can play on any laptop.
19. As a player with a gamepad, I want it to just work, so that the game feels like a console game.
20. As a player, I want Esc to pause, and the game to pause itself when the tab loses focus, so that I never lose a run to an interruption.

**Players: movement and combat**
21. As a player, I want to run and dash (there is no jump), so that I move fast and climb by walking up debris ramps.
22. As a player, I want a quick punch, a slower, stronger kick, and a dash attack (drop-kick), so that I have distinct tools.
23. As a player, I want to grab a live enemy, a loose debris part, or a stunned or lit bomb-head, and spin-throw it, so that I can clear crowds and reshape the pile.
24. As a player, I want hits to feel crisp: short hit-stop, snappy knockback, a white flash, sparks and a sound, so that every blow lands.
25. As a player, I want enemies' attacks telegraphed before they land, so that getting hit feels fair.
26. As a player, I want enemies I'm not fighting to pass through each other and me, so that a crowd can't pin me against a wall.

**Players: enemies**
27. As a player, I want fodder that walk at me and punch, so that I have easy targets to build the pile with.
28. As a player, I want thugs that are slow, tough and wide, with a big overhead smash that kicks (not punches) can interrupt, so that some fights need the right tool.
29. As a player, I want ninjas that dash in, strike and retreat, and avoid standing in front of me, so that some enemies demand movement.
30. As a player, I want some enemies, from box 2 on, to carry bombs that start a blinking 1.5 s fuse on defeat, so that I can throw them into crowds or get caught in the blast.
31. As a player, I want bombs to hurt everyone nearby and scatter debris without deleting it, so that explosions reshape the pile but never erase my progress.
32. As a colour-blind player, I want each enemy type to have a distinct shape, not just a colour, so that I can tell them apart.
33. As a player, I want enemies to enter through doors that open in the side walls with a short warning, so that spawns are readable.

**Players: the box, the pile and escape**
34. As a player, I want defeated enemies to fall apart into their own solid parts, which stay where they land, so that I can see and climb what I built.
35. As a player, I want the pile to behave like solid ground I can walk up, so that climbing feels reliable.
36. As a player, I want an exit to open when the pile height under me reaches the rim, so that escaping depends on where I build the pile.
37. As a player, I want a per-box timer, with time left adding to my score and running out costing health, so that I feel pressure to build fast.
38. As a player, I want each new box to be a little taller, shorter on time, busier, and in the next colour scheme, so that progress is visible and the challenge grows.
39. As a player, I want health to carry between boxes with a partial refill on each escape, so that a good box helps the next.

**Players: score and run end**
40. As a player, I want a HUD showing health, timer, score and box number, so that I always know where I stand.
41. As a player, I want a multiplier that grows with fast consecutive kills, so that aggressive play scores more.
42. As a player, I want a game-over screen with my score, box reached and best score, and an instant restart, so that I'm tempted to go again.
43. As a player, I want my best score remembered on this device with its date and box, so that I have something to beat.
44. As a player, I want the game to hold 60 fps on an ordinary laptop, lowering particle and bloom quality on slower machines, so that it never feels sluggish.
45. As a player who prefers reduced motion, I want no screen shake and softer flashes, so that the game is comfortable.

**Development and quality**
46. As the developer, I want a fast dev build that auto-reloads the browser, so that changing a number and seeing it takes well under 90 seconds.
47. As the developer, I want to run the game natively on this Windows machine, so that feel tuning restarts in seconds.
48. As the developer, I want every gameplay rule in an engine-free crate with tests, so that rules stay correct and portable to 3D later.
49. As the developer, I want headless scenarios that run the real ECS and physics wiring without a window, so that debris, freezing, escape and bomb behaviour are checked in CI.
50. As the developer, I want a browser smoke harness for the homepage and `/play`, so that console errors and load regressions are caught before merge.
51. As the developer, I want a measured physics budget (debris cap, substeps, freeze policy) from a browser stress test, so that piles stay smooth and avoid Avian's island panics.
52. As the developer, I want the cost of 2.5D rendering measured before the visual slices, so that the size and frame-rate trade-off is known.
53. As the agent, I want to merge a ticket's PR once CI is green and its acceptance check passes, so that small changes don't wait on David. Deploys still wait for David.
54. As David, I want 3–5 cohort-mates to playtest the deployed build and their notes to become the final tuning tickets, so that v1 is shaped by real players.

## Implementation Decisions

**Homepage launch**
- **CI deploys:** GitHub Actions runs `wrangler pages deploy` on the built site after tests pass on `main`, using a Cloudflare API token stored as a repository secret. Pull requests get preview deploys if the token allows them.
- **Analytics:** Cloudflare Web Analytics (cookieless, page views only), enabled through the Cloudflare beacon or dashboard. No custom events.
- **Content:** the landing page renders David's real content (projects, links, email as text, résumé). Until he provides it, the shareable build omits placeholders rather than showing them, as the current homepage artifact does.
- **Game visibility:** the Play nav item, hero call to action and easter egg are behind a single build-time switch that stays off until the playtest gate passes. The touch-device clip page is generated by the existing frame-exact recorder.

**Game architecture**
- **Stack:** Bevy 0.19.1 + avian2d 0.7.0 stay pinned together. Physics stays 2D; rendering becomes 2.5D: Bevy 3D meshes, lights, a perspective camera with a slight tilt showing the whole box, and spark particles. Rendering is the only part that depends on 3D.
- **`game-core` (engine-free) owns all rules:**
  - movement: run, dash, no jump
  - combat: moves, swings, hitboxes, knockback, health and hit-stun, hit-stop
  - grab and throw rules
  - enemy brains: fodder, thug (armoured against punches), ninja (hit-and-run), and the bomb fuse
  - run state: one life, health carry-over and partial refill, per-box timer and timeout penalty
  - scoring: kills × streak multiplier, plus a time-left bonus
  - the difficulty curve: wave composition by box, timer length, box height, scheme rotation
  - the escape test: pile height under the player ≥ rim height
  - the best-score record (serialisable)
- **`game` (the Bevy shell)** maps those rules onto entities, physics bodies, input (keyboard, mouse, gamepad), rendering, audio and UI.
- **Fighters** are articulated primitive figures (head, torso and limbs as separate visual parts) on a single 2D physics body while alive. Fighters collide with the world and debris, not with each other (collision layers already exist).
- **Debris:** on defeat, a figure breaks into its parts as separate dynamic bodies (≤ 6 per enemy), which collide with everyone. After settling (sleeping, or N seconds), debris freezes to static. Grabbing a part or an explosion thaws it locally. The cap and freeze policy come from the physics stress spike. This avoids Avian's open island-bookkeeping panics (#1025, #1076) and bounds the cost of waking large piles.
- **Spawning:** enemies enter through doors that appear in the side walls with a ~0.5 s telegraph.
- **Bombs:** an explosion damages fighters in radius and applies an impulse to debris, thawing and scattering it without ever removing it.
- **UI:** built with `bevy_ui` (adds `bevy_ui_render`, measured at +0.38 MB gz). Start overlay with controls, Esc pause with a sound toggle, HUD, game-over screen. Pauses on tab blur. Honours `prefers-reduced-motion` (no shake, softer flashes).
- **Audio:** generated retro sound effects (our own jsfxr-style assets, so no licensing), muted by default. Measure the audio feature's size cost.
- **Performance:** 60 fps on an integrated-GPU laptop, with automatic quality scaling (particle count, bloom) when frame time exceeds budget.
- **Build profiles:**
  - a fast dev or web profile (no LTO, incremental, dependencies optimised) for iteration, served by `trunk serve` with auto-reload
  - the existing size-optimised `game` profile (fat LTO, `opt-level = "z"`, `wasm-opt -Oz`) for CI and deploys only
  - native runs, via a toolchain fix for the GNU host (`llvm-dlltool` on PATH, or MSYS2 binutils) with static linking and LLD
- **Scope of 3D-readiness:** rules stay engine-free; no other 3D concessions.

**Process**
- **Tickets** are GitHub issues with dependencies noted, ordered so that any prefix is shippable: dev loop → spikes (physics stress test, 2.5D cost) → homepage launch (blocked on David's token and content) → game slices → playtest gate.
- The parked combat prototype (PR #4) is reworked under the new tickets, not merged as is.
- **Merge authority:** the agent merges a ticket PR after green CI and a passing acceptance check, then reports. Deploys and anything public need David.

## Testing Decisions

- **What makes a good test:** it exercises public behaviour at the highest available seam and asserts outcomes a player or visitor would notice (a hit lands, an exit opens, a score rises, a page renders readable text). It never asserts internal system ordering or private state. Each rule gets at least one test that fails if the rule is removed. Tests that track entities across frames use stable identities.
- **Seam 1, the `game-core` public API (primary):** every gameplay rule lands here first, test-first, natively in milliseconds. Prior art: the existing movement, combat and fodder tests (13), written red-first.
- **Seam 2, headless Bevy scenarios (new):** the real app without a window, stepped frame by frame through scripted scenarios:
  - defeated enemy → ≤ 6 solid parts
  - debris freezes after settling
  - pile height under the player reaches the rim → exit opens
  - a bomb scatters but never deletes debris
  - thawing on grab

  These run in Linux CI, and locally once native builds work.
- **Seam 3, the browser harness (Playwright, promoted into the repo):**
  - homepage renders with no console errors and readable text
  - no-WebGL fallback
  - reduced-motion still frame
  - `/play` loads with no console errors
  - a scripted input run reaches box 2
  - touch devices get the clip

  It runs against the built site locally and in a CI job. Prior art: the scratchpad scripts used to verify the hero and slices 1–2.
- **Seam 4, the `sim` crate (hero physics):** the existing 15 tests are unchanged.
- **Not asserted:** shader output and pixel-exact visuals. Visual quality is judged in review screenshots and the playtest gate.
- **Performance claims:** frame time and size budgets are measured and recorded per relevant ticket (the stress spike, the 2.5D spike, and CI size reports), never asserted from estimates.

## Out of Scope

- Touch controls; key remapping; music; a global leaderboard (the score format is ready for one); bonus stages (the first follow-up after v1); the Wildman enemy; combos and juggles.
- A full 3D game: v1 is 2.5D rendering over 2D physics.
- Buying a custom domain (David's call; the site works on `*.pages.dev`).
- Game-level analytics events.
- Reopening the hero design or the overall stack.

## Further Notes

- **Inputs David owns, which block the homepage-launch tickets only:** the Cloudflare API token, and the landing-page content (projects, LinkedIn, email, résumé file).
- **Avian:** stay on 0.7.0 with Bevy 0.19.1. Watch for a release carrying PR #989 if island panics appear despite freezing. Bevy 0.20 is out, but Avian doesn't support it yet.
- **Native toolchain:** native Windows builds on the GNU host currently fail on `raw-dylib` (`dlltool` missing). Bevy's dynamic linking is broken on GNU, so native runs use static linking with LLD.
- **Rumble Box facts** used here (no jump, wall doors, escape at the rim, bomb-heads as a modifier, grab plus spin-throw) are cited in `docs/research/game-research.md`. The original's health model, camera and level count are unverified.
