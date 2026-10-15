#!/usr/bin/env bash
# Deploys from this machine (CI can't: decision Q44's account minutes are spent). Production ships the current
# checkout, hero only, release switch off; `rc` ships a preview of a full game with the switch on at
# https://rc-<n>.slayback.pages.dev.
#   bash scripts/deploy.sh                 # production -> https://slayback.pages.dev
#   bash scripts/deploy.sh rc 2            # preview    -> https://rc-2.slayback.pages.dev
# rc-1 is the frozen archive of the retired 2.5D game (Q64): it is never redeployed, so `rc 1` is refused.
# With no CI behind it, this runs the browser harness (tests/e2e) on exactly the build it is about to ship and
# refuses to deploy on a failure. Needs Cloudflare credentials for wrangler: `npx wrangler@4 login` once on this
# machine, or CLOUDFLARE_API_TOKEN and CLOUDFLARE_ACCOUNT_ID in .env.local (git-ignored).
set -euo pipefail
cd "$(dirname "$0")/.."
if [ "${1:-}" = rc ]; then
  n="${2:?usage: deploy.sh rc <n>}"
  if [ "$n" = 1 ]; then
    echo "rc-1 is the frozen archive of the 2.5D game (Q64), which is no longer in this repo; deploy to another rc" >&2
    exit 1
  fi
  branch="rc-$n"
  GAME_RELEASE=1 bash scripts/build.sh
  harness_env=(GAME_RELEASE=1 REQUIRE_GAME=1)
else
  branch=main
  SKIP_GAME=1 bash scripts/build.sh
  harness_env=()
fi
harness() (
  cd tests/e2e
  [ -d node_modules ] || npm ci --silent
  # A port of its own, so the harness can't reuse some other server's build.
  env DIST=../../dist PORT=4179 "${harness_env[@]}" npx playwright test --reporter=line
)
# One retry: under heavy machine load the scripted climb has timed out (a 36 s run took 2.6 min) on a build that then
# passed twice. A real failure fails both runs.
if ! harness; then
  echo "harness failed; retrying once in case that was machine load" >&2
  harness
fi
npx --yes wrangler@4 pages deploy dist --project-name=slayback --branch="$branch" --commit-dirty=true
