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

- Enemies arrive constantly, 6-10 on screen in the first minute, through **tall black doorways that open in the
  walls**.
- Debris covers the whole floor within ~1 minute and builds into a deep layer of thousands of pieces. Pieces keep
  a pale tint of their figure's colour.
- Each box has a 3:30 clock. Box 1 is pale green with a tiled floor and thick black edges on a sky-blue
  background; box 2 grey checker; box 3 orange checker. Between boxes, a **bonus stage**: a tall shaft to climb, and a
  dark-blue arena.
- HUD: a row of green health pips at the top centre, score bottom left, clock bottom right; "Enemies Defeated: N"
  at the end of a box.
- Controls (the opening card): arrow keys move, double-tap to dash, Space attacks, Control or Alt grabs.
  "Goal: Get Out of the Box!"

## Where our 3D build is furthest off

1. The camera: fixed, square-on and far back (`camera.rs`, Q48), against a moving, yawed, close camera.
2. The player has no attack animation at all; Rumble Box's punch is a fast, readable arm extension.
3. Rigid sine-wave limb swings against floating parts with big, snappy offsets.
4. Pace: a few enemies at a time against a constant stream through wall doorways.
