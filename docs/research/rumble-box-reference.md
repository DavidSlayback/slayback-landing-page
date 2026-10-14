# Rumble Box: what the gameplay video shows

Source: DigiPen's official gameplay video, linked from https://games.digipen.edu/games/rumble-box
(https://youtu.be/2sdb2prWhTY, 5:52, 1280x720, 30 fps). Studied 2026-10-14 from contact sheets every 8 s and
30 fps crops of the first fights (0:08-0:28). Figures marked ~ are estimates read off frames, not measurements.
This replaces the screenshot-only reading in `game-research.md` section 1 wherever they disagree.

## Camera

- **It moves.** The yaw changes between frames half a second apart, swinging up to ~45° either way as the player
  moves. The distance barely changes: the box fills the frame throughout. It never holds the square-on framing we
  built.
- **Default framing:** yawed ~25-35° off the box's axis, so a back corner sits in view; pitch ~35-40°; close enough
  that the near wall falls off the bottom of the screen. The box fills the width of the frame.
- **In a fight** (0:19-0:28, full frames): pitch ~30-35°, a little lower than roaming; fighters are 1/7 to 1/5 of
  the frame's height. (An earlier reading of "low and close" came from 2x crops and was wrong.)
- **Intro:** each box opens with the camera at floor level among debris, swooping up and out to the default view
  (~2.5 s), while the player's parts fly together to assemble it.

## The player and the figures

- **Floating parts, no joints.** Head (orange ball), torso and shoulders (~4-5 blue cubes), arms (one blue cube and
  a big red ball fist each), legs (two dark cubes each). The parts have visible gaps between them.
- **Fodder:** magenta cube torso, peach ball head, brown feet. Several kinds at once; dark/grey ones with red ball
  fists (heavier), blue ones.
- Figures are big relative to the box: ~1/8 of the floor's width tall.

## Attacks (30 fps frame counts, 0:20.4)

- **Punch:** the arm's parts spread out along a straight line toward the target: the fist reaches full extension in
  ~2-3 frames (~80 ms), holds ~1-2 frames, and pulls back in ~2-3 frames. About 0.2 s per punch, thrown in quick
  strings. The torso twists into it, and the body leans.
- **Hit:** the victim's parts jolt; a kill blows the figure apart, its parts flying up a metre or more and
  tumbling, with a "100" popup and a combo multiplier (`+200 x2`).
- **Walk:** legs step with large alternating offsets; the body bobs.

## The box and the run

- Enemies arrive constantly through **tall black doorways that open in the walls**, often two at once. The crowd
  ramps over box 1 from one or two to 6-8 (counts below), and box 2 starts busy.
- Debris covers the whole floor within ~45 s of box 1 and builds into a deep layer of many hundreds of pieces. Pieces
  keep a pale tint of their figure's colour.
- Box 1 is pale green with a tiled floor and thick black edges on a sky-blue background, on a 3:30 clock; box 2
  ("Stage Two") is orange checker on a 6:00 clock. Between them come two **bonus stages**: a tall shaft to climb, then
  "Frenzy!" in a grey checker box ("How many enemies can you defeat in 30 seconds?"). After box 2: the shaft again
  and a dark-blue arena. The video ends there; it never shows a box 3.

### Pace, counted (2026-10-14)

Full 1280x720 frames once a second, 0:16-1:44 and four 8 s windows of box 2. "On screen" counts live enemies in
the frame (the camera shows nearly the whole box), not the player; it misses one hidden behind a wall now and then.
Kills per 10 s are inferred from the score (100 a fodder, 250 a heavy, times the combo count), so they are ~.

| Video | In the box | On screen | Kills per 10 s | Notes |
| --- | --- | --- | --- | --- |
| 0:17-0:20 | box 1, 0-3 s | 0, then 1 | - | First door opens ~2 s after the clock starts |
| 0:20-0:30 | 3-13 s | 1-5, mostly 3-4 | ~6 (+900) | Two doors open together at 0:21 |
| 0:30-0:40 | 13-23 s | 2-5, mostly 4 | ~6 (+1,130) | Heavies (dark, red fists) from ~14 s, two doors together at 0:31 |
| 0:40-0:50 | 23-33 s | 2-4 | ~6 (+1,270) | Floor mostly covered |
| 0:50-1:00 | 33-43 s | 3-6 | ~4 (+600) | Two doors together at 0:51 |
| 1:00-1:04 | 43-47 s | 6-8 | - | The peak |
| 1:04-1:20 | 47-63 s | 1-5 | ~6 | The player is up on the pile; a bomb-head at 1:14 |
| 1:20-1:36 | 63-79 s | 2-5 | ~10 (+1,900) | Escaped at 1:36 with 2:15 left (75 s in box 1) |
| 2:01-2:31 | Frenzy, 30 s | - | ~12 | "Enemies Defeated": 35 in 30 s (10, 9, 6, 3, 6, 1 per 5 s) |
| 2:52-2:56 | box 2, 3-7 s | 4-7 | - | Busy from the start; a bomb-head |
| 3:20-3:24 | 31-35 s | 4-5 | - | |
| 3:50-3:54 | 61-65 s | 4-5 | - | Combos to x5-x6 |
| 4:20-4:24 | 91-95 s | 4-6 | - | Escaped at ~4:52 (~125 s in box 2) |

So box 1 holds 3-5 for its first half-minute and peaks at 6-8 about 45 s in; box 2 holds 4-7 throughout. Entries
roughly match kills, about 4-10 per 10 s (the Frenzy stage, with enemies to spare, shows a player can kill ~12 per
10 s, and 2 a second in a burst). An idle wait between entries is under ~2 s, and the doors open in parallel.
- HUD: a row of green health pips at the top centre, score bottom left, clock bottom right; "Enemies Defeated: N"
  at the end of a box.
- Controls (the opening card): arrow keys move, double-tap to dash, Space attacks, Control or Alt grabs.
  "Goal: Get Out of the Box!"

## Where our 3D build is furthest off

1. The camera: fixed, square-on and far back (`camera.rs`, Q48), against a moving, yawed, close camera.
2. The player has no attack animation at all; Rumble Box's punch is a fast, readable arm extension.
3. Rigid sine-wave limb swings against floating parts with big, snappy offsets.
4. Pace: a few enemies at a time against a constant stream through wall doorways, ramping to 6-8.
