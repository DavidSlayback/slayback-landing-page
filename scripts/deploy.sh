#!/usr/bin/env bash
# Deploys from this machine (CI can't: decision Q44's account minutes are spent). Production ships the current
# checkout, hero only, release switch off; `rc` ships a preview of the full game with the switch on at
# https://rc-<n>.slayback.pages.dev.
#   bash scripts/deploy.sh          # production -> https://slayback.pages.dev
#   bash scripts/deploy.sh rc 2     # preview    -> https://rc-2.slayback.pages.dev
# With no CI behind it, this runs the browser harness (tests/e2e) on exactly the build it is about to ship and
# refuses to deploy on a failure. Needs Cloudflare credentials for wrangler: `npx wrangler@4 login` once on this
# machine, or CLOUDFLARE_API_TOKEN and CLOUDFLARE_ACCOUNT_ID in .env.local (git-ignored).
set -euo pipefail
cd "$(dirname "$0")/.."
if [ "${1:-}" = rc ]; then
  branch="rc-${2:?usage: deploy.sh rc <n>}"
  GAME_RELEASE=1 bash scripts/build.sh
  harness_env=(GAME_RELEASE=1 REQUIRE_GAME=1)
else
  branch=main
  SKIP_GAME=1 bash scripts/build.sh
  harness_env=()
fi
(
  cd tests/e2e
  [ -d node_modules ] || npm ci --silent
  # A port of its own, so the harness can't reuse some other server's build.
  env DIST=../../dist PORT=4179 "${harness_env[@]}" npx playwright test --reporter=line
)
npx --yes wrangler@4 pages deploy dist --project-name=slayback --branch="$branch" --commit-dirty=true
