#!/usr/bin/env bash
# Checks that this machine can build, test and deploy the site, and prints the fix for each thing missing. Run it
# once on a new machine; deploy.sh runs it first, so a missing tool fails there instead of halfway through a deploy.
#   bash scripts/doctor.sh              # everything a full (rc) build and deploy needs
#   SKIP_GAME=1 bash scripts/doctor.sh  # what a hero-only (production) build needs: no wasm-bindgen
# Not checked: Cloudflare credentials (see deploy.sh), which need the network.
set -uo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
missing=0
check() {   # check <what> <fix> <command...>
  local what="$1" fix="$2"; shift 2
  if "$@" >/dev/null 2>&1; then echo "ok       $what"; else echo "MISSING  $what: $fix"; missing=1; fi
}

check "cargo" "install Rust with rustup (https://rustup.rs)" command -v cargo
check "wasm32-unknown-unknown target" "rustup target add wasm32-unknown-unknown" \
  bash -c 'rustup target list --installed | grep -qx wasm32-unknown-unknown'
check "wasm-opt (Binaryen)" "cargo install wasm-opt --locked" command -v wasm-opt
if [ "${SKIP_GAME:-0}" != 1 ]; then
  # The CLI must match the wasm-bindgen crate in Cargo.lock exactly, or the glue it writes doesn't fit the module.
  want=$(grep -A1 '^name = "wasm-bindgen"$' Cargo.lock | sed -n 's/^version = "\(.*\)"/\1/p')
  check "wasm-bindgen $want" "cargo install wasm-bindgen-cli --version $want --locked" \
    bash -c "wasm-bindgen --version | grep -qx 'wasm-bindgen $want'"
fi
check "pre-commit hooks" "uv tool install pre-commit && pre-commit install -t pre-commit -t pre-push" \
  test -f "$(git rev-parse --git-path hooks/pre-push)"
check "node" "install Node.js 20 or later" command -v node
# The harness runs in the installed Chrome unless PW_CHANNEL says otherwise; launching it is the only sure test.
check "Chrome for the harness" "install Google Chrome, or set PW_CHANNEL= to use a Playwright-managed browser" \
  bash -c 'cd tests/e2e && [ -d node_modules ] || npm ci --silent && node -e "
    const { chromium } = require(\"@playwright/test\"); const channel = process.env.PW_CHANNEL ?? \"chrome\";
    chromium.launch(channel ? { channel } : {}).then(b => b.close(), () => process.exit(1));"'
exit "$missing"
