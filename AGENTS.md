# Agent notes

- Resume from the block at the top of `docs/PLAN.md`. Domain words (Blow, Strike, Feel, ...) are defined in `GLOSSARY.md`. Decisions, in David's words, are in `docs/planning/game-decisions.md`; the README maps the crates, scripts, hooks and CI.
- On the Windows GNU-host toolchain, anything that links the game (`run`, `test`) goes through `bash scripts/native.sh <cargo args>`: plain cargo links an exe that crashes at startup.
- Game feel or visuals (camera, animation, look, pace): compare against `docs/research/rumble-box-reference.md`, reading the reference video's full frames, never crops. Done means David has played the rc preview (Q67); tests and screenshots alone don't close it.
- A new material or mesh layout drawn mid-game freezes WebGL while its shader links: draw it once at load (`camera::Warm`). `tests/e2e/shaders.spec.js` fails if anything links after the warm-up.
- To look at a page or measure the game (screenshots, frames, frame times, shader links) run `tests/e2e/tools/{shot,capture,frametime,trace-shaders}.mjs` on a `dist/` build; each file's header lists its flags.
