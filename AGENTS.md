# Agent notes

- Resume from the block at the top of `docs/PLAN.md`. Decisions, in David's words, are in `docs/planning/game-decisions.md`; the README maps the crates, scripts, hooks and CI.
- On the Windows GNU-host toolchain, anything that links the game (`run`, `test`) goes through `bash scripts/native.sh <cargo args>`: plain cargo links an exe that crashes at startup.
