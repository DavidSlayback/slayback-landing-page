#!/usr/bin/env bash
# Deploys from this machine (when CI can't): production from the current checkout, hero only, release switch off;
# and with `rc` a preview of the full game with the switch on, at https://rc-<n>.slayback.pages.dev.
#   bash scripts/deploy.sh          # production -> https://slayback.pages.dev
#   bash scripts/deploy.sh rc 2     # preview    -> https://rc-2.slayback.pages.dev
# Needs Cloudflare credentials for wrangler: `npx wrangler@4 login` once on this machine, or CLOUDFLARE_API_TOKEN and
# CLOUDFLARE_ACCOUNT_ID in .env.local (git-ignored).
set -euo pipefail
cd "$(dirname "$0")/.."
if [ "${1:-}" = rc ]; then
  GAME_RELEASE=1 bash scripts/build.sh
  branch="rc-${2:?usage: deploy.sh rc <n>}"
else
  SKIP_GAME=1 bash scripts/build.sh
  branch=main
fi
npx --yes wrangler@4 pages deploy dist --project-name=slayback --branch="$branch" --commit-dirty=true
