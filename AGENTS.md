# Agent notes

- Resume from the block at the top of `docs/PLAN.md`. Decisions, in David's words, are in `docs/planning/game-decisions.md`; the README maps the crates, scripts, hooks and CI.
- On the Windows GNU-host toolchain, anything that links the game (`run`, `test`) goes through `bash scripts/native.sh <cargo args>`: plain cargo links an exe that crashes at startup.
- Game feel or visuals (camera, animation, look, pace): compare against `docs/research/rumble-box-reference.md`, reading the reference video's full frames, never crops. Done means David has played the rc preview (Q67); tests and screenshots alone don't close it.
- A new material or mesh layout drawn mid-game freezes WebGL while its shader links: draw it once at load (`camera::Warm`). `tests/e2e/shaders.spec.js` fails if anything links after the warm-up.
