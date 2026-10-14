# Game planning: decision log

Grilling rounds for the remaining project (game + launch). Each row is a settled decision with its source; open questions are listed at the end. This log feeds `docs/planning/game-plan.md` (the execution plan) and the GitHub issues.

## Round 1 (2026-10-12)

| # | Decision | Answer | Notes |
|---|---|---|---|
| Q1 | Scope of this planning pass | **Everything left**: the game, plus deployment, real site content, and the game's entry point | Hero and stack are not reopened. |
| Q2 | What the game is for | **Portfolio showpiece**: 3–5 minutes, played once, remembered | Feel and polish over content volume. |
| Q3 | How close to Rumble Box | **Its core loop with our own identity** | Pile-to-escape loop; the hero's world: neon, schemes, physics FX. |
| Q4 | Content size and structure | **Endless boxes**, escalating difficulty, high score | Not the 3-box recommendation. Progression is a difficulty curve plus score. |
| Q5 | Devices and controls | **Desktop only + gamepad** | Touch devices get a fallback (no touch controls). Keyboard + mouse already settled. |
| Q6 | Art direction | **A distinct look, but with particles** | Read together with Q9 as flat-shaded low-poly 2.5D plus spark particles. Confirm in round 2. |
| Q7 | Audio | **Sound effects only, off by default** | No music. |
| Q8 | Done / date | **No date: done when it meets the acceptance criteria** | Tickets ranked so any prefix is shippable. |
| Q9 | 3D | **2.5D now, with engine-free rules** | 3D rendering over avian2d physics; `game-core` stays engine-free. |
| Q10 | Ticket tracker | **GitHub Issues** in `slayback-landing-page` | Labels, a milestone, and blocking edges noted per issue. |

## Round 2 (2026-10-12)

Informed by `docs/research/game-research.md`.

| # | Decision | Answer | Notes |
|---|---|---|---|
| Q11 | What "distinct look" means | **Flat-shaded low-poly figures built from primitives** (box torso, sphere head, box limbs) + spark particles | Characters visibly break into their own parts. |
| Q12 | Camera | **Fixed side view, slight perspective tilt, whole box visible**, plus small hit shake | |
| Q13 | Debris | **The enemy's own parts** become solid rigid bodies that persist; spark burst on defeat; capped per enemy | Settle-then-freeze to static (Avian island panics #1025/#1076; no wasm benchmark). Grains are FX only. |
| Q14 | Lives / health | **One life per run**; health carries between boxes with a partial refill on escape; death → score screen → instant restart | |
| Q15 | High score | **Local now** (`localStorage`), **serializable for a global board later** | |
| Q16 | Sound-effect source | **Generated retro effects** (jsfxr-style, ours) | Off by default (Q7). Measure the audio feature's size. |
| Q17 | Deployment | **CI deploys**: `wrangler pages deploy dist/` with an API token secret | Needs David's Cloudflare token. |
| Q18 | Domain | **`*.pages.dev` now, buy a domain later** | Purchase is David's call. |
| Q19 | Landing content | **Now:** projects list, GitHub, LinkedIn, email as text, résumé link. **Later:** a fuller "about" | Content from David. |
| Q20 | Game entry | **"Play" nav item + hero CTA, plus a secret easter-egg entry** | The CTA becomes a spark obstacle. |
| Q21 | Touch devices | **Looping recorded gameplay clip** + "Play on a computer" | Reuses the frame-exact recorder. |
| Q22 | Dev loop | **Fast wasm dev profile (no LTO) + `trunk serve`, plus native runs** via a light toolchain fix (`llvm-dlltool` or MSYS2 binutils), static linking + LLD | Verify with a throwaway build first. |
| Q23 | Jump | **No jump; climb ramps of debris; dash instead** | Changes slice-1 controls. |
| Q24 | Escape | **Exit opens when the pile height under the player reaches the rim** | |
| Q25 | Verbs | **Punch, kick, grab + spin-throw, dash** (dash + attack = drop-kick) | |
| Q26 | Roster | **Fodder, Thug, Ninja + bomb-head modifier** on any type | Wildman later. |
| Q27 | Pressure | **Per-box timer** that shrinks each box; time left → score; timeout costs health | |

## Corrections

- `docs/PLAN.md` said HUD text needs new features. It doesn't: `bevy_text` and `default_font` are already enabled. Adding `bevy_ui_render` costs +0.38 MB gzipped (5.10 → 5.48 MB, measured).

## Round 3 (2026-10-12)

| # | Decision | Answer | Notes |
|---|---|---|---|
| Q28 | Feel reference | **Crisp and punchy** (Hollow Knight / Celeste) | Short hit-stop and snappy knockback everywhere, including kicks and throws. |
| Q29 | Difficulty target | **A first-timer dies around box 3–4** (≈3–5 min) | |
| Q30 | Bonus stages | **None in v1**; filed as the first follow-up | |
| Q31 | Color scheme in game | **Rotate through the three schemes box by box**, starting from the site's scheme | |
| Q32 | Bomb-heads vs. the pile | **Hurt nearby enemies and the player; scatter debris but never delete it** | The pile never loses progress outright. |
| Q33 | What can be grabbed | **Live enemies, loose debris parts, and stunned or lit bomb-heads** | Frozen debris thaws when grabbed. |
| Q34 | Screens / UI | **Minimal**: start overlay, Esc pause with sound toggle, HUD (health, timer, score, box), game-over screen with score and best | Built with `bevy_ui` (+0.38 MB gz; measured +0.41 MB in D5, total 6.42 MB gz). |
| Q35 | Accessibility | **Reduced-motion support** (no shake, softer flashes), pause on tab blur, controls on the start overlay, **plus colorblind-safe enemy shapes** | Key remapping deferred. |
| Q36 | Analytics | **Cloudflare Web Analytics** (cookieless, page views only) | |
| Q37 | Performance target | **60 fps on an integrated-GPU laptop**, with automatic quality scaling | Physics budget comes from a browser stress spike. |
| Q38 | Playtest gate | **v1 is done only after a cohort playtest** (3–5 people on the deployed build) | Their notes become the final tuning tickets. |
| Q39 | Merge authority | **Claude merges a ticket's PR once CI is green and the acceptance check passes**, then reports | Deploys need David's go-ahead. |
| Q41 | When CI runs (David, 2026-10-13) | **"I don't really want feature branches to have to build until complete and ready to merge."** PRs open as drafts while in progress; CI runs only for non-draft PRs into main (marking ready triggers it), on main pushes, or by hand | Stacked PRs don't build until retargeted to main; docs-only PRs skip CI; newer runs cancel older ones. |
| Q42 | How much CI (David, 2026-10-13) | **"Feature branch CI/CD should be ruthlessly pared down. We should optimize the shit out of CI."** PRs get one `check` job (tests incl. headless game, clippy, hero build + budget); the wasm game build, browser harness and deploys run only on main and `rc/` branches. No PR preview deploys. | Wasm-only compile errors and browser regressions surface after merge, on main. |
| Q43 | Asking permission (David, 2026-10-13) | **"Can you stop fucking asking me for permission."** Standing authorization: merge, deploy (production and previews, including the rc/ game preview) and run the backlog without asking first, then report. | Only things the agent physically can't do (logins, payments) go to David, as a short action list, not a question. |
| Q44 | Stacked PRs and CI (David, 2026-10-13) | **"I actually do want stacked PRs. What I don't want is to run CI/CD on every single damn one. We should be running CI only when they're ready to merge and only when we're going to go merge them."** Stacks stay drafts through restacks (`gh stack`); a PR is marked ready only to merge it now. Superseded runs cancel, on main too, so merging a stack builds and deploys once. | Spend is tracked with david-brain's `ci-minutes` (account-wide: 2,123 of 2,000 free minutes in October 2026). |
| Q45 | Local checks (David, 2026-10-13) | **"There should definitely be pre-commits. We should have a standard set of pre-commits for any Rust project."** `.pre-commit-config.yaml` starts from david-brain's Rust template: hygiene, rustfmt and clippy on commit; tests, the wasm render lint and the hero budget on push. | Since merges no longer wait on CI, the hooks are the gate. The code was reformatted to 120 columns once. |

## Round 4 (2026-10-12)

| # | Decision | Answer |
|---|---|---|
| Q40 | Launch sequencing | **Homepage first** (hero, content, analytics) as soon as the token and content arrive; the Play link stays hidden until the game passes the playtest gate |

**Accepted assumptions** (proposed, not objected to; cheap to change during tuning):
- **Spawning:** enemies enter through telegraphed wall doors.
- **Thug:** ~2× fodder health, slow and wide. Overhead smash; punches can't interrupt it, kicks can.
- **Ninja:** low health and fast. Dashes in, strikes, retreats, and avoids standing in front of the player.
- **Bomb-head:** a modifier from box 2 on. A 1.5 s blinking fuse starts on defeat; it can be grabbed and thrown while lit.
- **Score:** kills × a streak multiplier, plus a time-left bonus on escape. The best score is stored with its date and box reached.
- **Difficulty:** per box, more enemies per wave, new types (thugs from box 2, ninjas from box 3), a shorter timer, and a slightly taller box. Data-driven.
- **Existing code:** slices 1–2 evolve (jump → dash). PR #4 is reworked under the new tickets, not merged as-is.
- **Ticket order:** dev loop → spikes (physics stress test, 2.5D cost) → launch-site tickets → game slices in shippable-prefix order → playtest gate.

## Status

The 2.5D decision tree (rounds 1–4) is complete. Round 5 reopens the game's form for 3D.

## Round 5: 3D (2026-10-13)

David, after the look and impact passes: "Take inspiration from rumble box for the visuals. We may need to go to 3d", then "We should properly plan 3d first." Evidence: research §1 (Rumble Box's camera and look, verified from its official screenshots) and §7 (the full-3D spike: 60 fps at 500 resting cubes on the integrated GPU, bloom off, no wasm size cost). Answers are David's letter picks for the options as asked.

| # | Decision | Answer | Notes |
|---|---|---|---|
| Q46 | 3D versus the playtest | **A: 3D is v1.** The cohort playtest waits until 3D matches the current game's feature set. | Supersedes the 2.5D path to the playtest (Q38 still gates release). |
| Q47 | Code structure | **A: a new `game3d` crate beside `game`, sharing `game-core`.** rc-1 keeps the 2.5D game; `game` is deleted once 3D matches it. | Each 3D slice lands on `main` and deploys to rc-2. |
| Q48 | Camera | **C: a fixed 3/4 view of the whole box now;** gentle follow and zoom later only if boxes outgrow one screen. | |
| Q49 | Movement, facing, aim | **B: 8-direction movement, facing the last move direction, blows hit a cone in front, plus light aim assist** toward the nearest enemy in the cone. | No twin-stick. |
| Q50 | Look | **C: hybrid.** Flat-shaded saturated cubes with outlines and comic impact bursts, coloured from the site's three schemes: arena and debris in the scheme's muted tone, live fighters in its vivid accents. | Bloom is out (research §7). Outlines need an edge-detection pass, as an early spike ticket. |
| Q51 | Escape rule | **A: escape when the pile under the player reaches the rim;** the rim rises each box, the floor stays the same size. | |
| Q52 | Debris budget | **A: settled debris freezes after ~1.5 s; at most 300 moving pieces; frozen pieces kept up to a measured cap (target ~1,500).** | Measured in S2 (#60): **1,500 frozen + 300 moving**; fallback 1,000 + 300 if the game costs more than ~3 ms per frame (research §7). |
| Q53 | Testing | **A: game-core rules tests, plus the 26 headless scenarios ported to 3D, plus the browser harness on rc-2.** | |
| Q54 | 3D v1 scope | **A: everything the game has today:** three enemy types, grab and spin-throw, bomb-heads, doors, sound, gamepad, comfort, score/HUD/screens, demo mode. | Tickets still land the core loop first. |
| Q55 | Phones | **A: unchanged.** Phones get the clip page and never download the game. | |
| Q56 | Box shape | **B: a wide rectangle, about 3:2.** | Fills a 16:9 screen without much more floor to pile on. |
| Q57 | Front wall | **A: solid, drawn as a see-through glass outline.** | Keeps the action and the rim's height visible. |
| Q58 | Doors | **A: in the back wall and both side walls,** none in the front. | The 2.5D warning pulse carries over. |
| Q59 | Jump | **A: still no jump.** Climb by walking up the pile, with an automatic step-up of about half a cube. | Step height is tuned in the controller ticket. |
| Q60 | Finding yourself | **A: a bright ring on the floor under the player,** in the scheme's main accent. | David chose the ring alone, over the recommended ring plus unique colour and outline. |
| Q61 | Figures | **A: several cubes per limb, Rumble Box style:** fodder ~11 parts, thugs ~14, ninjas ~9. | |
| Q62 | Throws | **A: flat along the facing, with aim assist toward the biggest enemy group within ~30°.** | Same assist rule as Q49. |
| Q63 | Enemy movement | **A: steering:** seek the player, separate from each other, avoid walls. Ninjas circle to flank; thugs plough straight in. | No pathfinding. |
| Q64 | Retiring 2.5D | **A: delete `game`, and `/play` serves 3D with no fallback.** rc-1 stays up as an archive of the 2.5D game. | |

## Round 6: feel pass (2026-10-14)

David, after playing rc-2 with G1-G10: "this plays like absolute shit. It feels nothing like Rumble Box. The main character doesn't even have a punch animation. The camera angle is wrong. The animations that are there are really stilted. Did you even look at how that game plays?" Evidence: DigiPen's gameplay video, studied frame by frame (`docs/research/rumble-box-reference.md`).

| # | Decision | Answer | Notes |
|---|---|---|---|
| Q65 | Camera | **Match Rumble Box:** a camera yawed off the box's axis that swings round as the player moves, the box filling the frame. | Reverses Q48. The question described a camera that "pushes in low during fights", from 2x crops of the video; full frames show only a slight drop, so the build follows the full frames. Movement becomes camera-relative, so the arrow keys stay "up the screen". |
| Q66 | Look | **Keep the schemes, but thick black edges and saturated figures.** | Amends Q50: outlines bolder and black; fighters' colours no longer dimmed or washed out. |
| Q67 | Order and review | **Camera and look, then the player's attack animations, then loose-part figures and kill bursts, then the pace.** Each step is played by David on a preview before it merges. | Proposed by Claude; David answered Q65 and Q66 without changing it. The playtest (#29) waits for his sign-off on feel. |
| Q68 | Standing still | **Lethal, by design.** David: "Standing still should be lethal." | Three fodder kill an idle player in about 12 s; leave it (or make it faster as the pace rises). |
| Q68a | Pace (feel pass 4) | **Rumble Box's pace, counted from the video:** box 1's crowd ramps from 3 to 7 over 40 s (Rumble Box: 3-5, then 6-8 about 45 s in); later boxes start at 5-7 and peak at 8-10. Doors are called every 1 s in box 1 (0.6 s at the fastest), up to two at once (three from box 3), each still warning for 0.5 s. | Replaces the first guesses made for Q29 (3 enemies in box 1, one more a box, one door every 2 s); follows Q68, never softer. An idle player now dies in about 10 s in box 1. The 1,500 frozen cap holds a full clock of kills at one a second (about the busiest the video shows); a box played past a timeout can exceed it, and then the oldest pieces go as before (Q52). Rumble Box also sends heavies in box 1; ours keep thugs from box 2 (Q26). Counts in `docs/research/rumble-box-reference.md`. |
| Q69 | Next review | **David looks again only once the attack animations (step 2) and the loose-part figures and pace (steps 3-4) are in.** David: "I don't even want to look until you fix 1 and 2." | rc-3 stays the preview. |
| Q70 | Player attacks | **More range and fluidity.** David, after rc-3: "The main character's attacks need more range and fluidity. I think movement speed is roughly ok." | Punch, kick and drop-kick reach +35% (game-core rule); the arm stretches further, the body lunges, the fist arcs, the torso counter-twists first. Gameplay timing unchanged. |
| Q71 | Figures | **More space between the blocks.** David: "More space between the blocks in each character." | Parts drawn smaller and spread sideways, player and enemies alike. |
| Q72 | First-hit freeze | **Fix the lag spike on the first attack.** David: "I notice a huge lag spike the first time I try to attack anyone." | Measured: a 3,642 ms frame at the first punch (WebGL builds each new render pipeline on first draw). Fixed by drawing those materials once while the page loads (`camera::Warm`): 67 ms. |
