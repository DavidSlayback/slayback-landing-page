#!/usr/bin/env bash
# Native (desktop) game builds on a Windows GNU-host toolchain: `bash scripts/native.sh` runs the game in a
# window; any other arguments go to cargo instead (e.g. `bash scripts/native.sh test -p game3d`).
#
# Bevy's Windows dependencies import DLL functions with `raw-dylib`, so rustc needs a `dlltool` on PATH. The
# toolchain's bundled GNU dlltool needs a GNU assembler we don't have, and llvm-dlltool's import libraries are
# mislinked by GNU ld (an IAT slot is left unbound and the exe crashes at startup). So this links with the
# toolchain's own LLD and uses llvm-dlltool, both copied out of the rustup sysroot into target/native-tools/.
# Nothing is installed globally; it needs only `rustup component add llvm-tools`. Wasm and CI builds are unaffected.
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

case "$(rustc -vV | sed -n 's/^host: //p')" in
  *-pc-windows-gnu) ;;
  *) [ $# -eq 0 ] && set -- run -p game3d; exec cargo "$@" ;;   # nothing to set up on other hosts
esac

bin="$(rustc --print sysroot)/lib/rustlib/x86_64-pc-windows-gnu/bin"
tools="${CARGO_TARGET_DIR:-target}/native-tools"
if [ ! -x "$tools/ld.exe" ] || [ ! -x "$tools/dlltool.exe" ]; then
  [ -f "$bin/llvm-ar.exe" ] || { echo "missing llvm-ar: run 'rustup component add llvm-tools'" >&2; exit 1; }
  mkdir -p "$tools"
  # Both are multi-call binaries that pick their mode from their own file name: rust-lld named `ld` runs its
  # MinGW driver (gcc passes -m i386pep), and llvm-ar named `dlltool` behaves as llvm-dlltool.
  cp "$bin/rust-lld.exe" "$tools/ld.exe"
  cp "$bin/llvm-ar.exe" "$tools/dlltool.exe"
  cp "$bin"/*.dll "$tools/"
fi
export PATH="$(cd "$tools" && pwd):$PATH"   # POSIX form: a C:/ path would split on its colon
# gcc's -B puts our `ld` ahead of the bundled GNU ld. Target-scoped, so wasm builds keep their own flags.
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C link-arg=-B$(cd "$tools" && pwd -W)/"

[ $# -eq 0 ] && set -- run -p game3d
exec cargo "$@"
