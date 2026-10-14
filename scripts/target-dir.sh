# Sourced by native.sh and build.sh, from the repo root. In a git worktree with no CARGO_TARGET_DIR set, builds share
# the main checkout's target directory: a worktree's cold `target/` costs a full Bevy build (~15 min) and, on the
# Windows GNU host, failed the pre-push hook ("dlltool.exe: program not found"). Set CARGO_TARGET_DIR yourself to
# opt out (e.g. a parallel agent that should not contend for the main checkout's build lock).
if [ -z "${CARGO_TARGET_DIR:-}" ]; then
  _common="$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null || true)"
  _main="${_common%/.git}"
  _here="$(git rev-parse --path-format=absolute --show-toplevel 2>/dev/null || true)"
  if [ -n "$_common" ] && [ "$_main" != "$_common" ] && [ "$_main" != "$_here" ] && [ -d "$_main/target" ]; then
    export CARGO_TARGET_DIR="$_main/target"
  fi
  unset _common _main _here
fi
