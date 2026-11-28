#!/usr/bin/env bash
# First step after the checkout on a self-hosted runner (agents#48). The
# checkout has just cleaned the workspace, as on GitHub's runners. The build
# folder and the corpus audio live outside it, one set per runner, so builds
# stay warm without the Actions cache: `target` is a link to this runner's
# build folder. TG_CACHE comes from the runner's .env (tg-runner.sh install).
# A build folder past TG_TARGET_MAX_GB (default 60) starts again empty.
set -eu
cache="${TG_CACHE:-$HOME/tg-cache/${RUNNER_NAME:-runner}}"
max="${TG_TARGET_MAX_GB:-60}"
mkdir -p "$cache/target"
used="$(du -s --block-size=1G "$cache/target" | cut -f1)"
echo "build folder $cache/target: ${used} GB (limit ${max})"
if [ "$used" -gt "$max" ]; then
  echo "over the limit: starting it again empty"
  rm -rf "$cache/target"
  mkdir -p "$cache/target"
fi
rm -rf target
ln -s "$cache/target" target
echo "TG_CACHE=$cache" >> "$GITHUB_ENV"
