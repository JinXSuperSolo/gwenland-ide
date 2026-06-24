#!/usr/bin/env bash
# Re-fire the tag-triggered release workflows for a version.
#
# The five .github/workflows/*.yml release builds only run on a pushed `vX.Y.Z`
# tag. When a release fails for a workflow reason (not a code reason), you fix it
# on main, then this script moves the tag to the fixed main tip and re-pushes it,
# which re-triggers all five workflows.
#
# Usage:   scripts/retag-release.sh v0.1.0
#
# Safety:  refuses to run unless the local tree is clean and the tag would point
# at the current origin/main tip (so you never tag stale or unmerged code).
set -euo pipefail

TAG="${1:-}"
if [[ -z "$TAG" ]]; then
  echo "usage: scripts/retag-release.sh vX.Y.Z" >&2
  exit 2
fi
if [[ ! "$TAG" =~ ^v[0-9]+\.[0-9]+\.[0-9]+ ]]; then
  echo "error: tag must look like vX.Y.Z (got '$TAG')" >&2
  exit 2
fi

echo "→ Fetching origin…"
git fetch origin --prune --tags --quiet

# Must be on a clean tree so we don't accidentally tag uncommitted work.
if [[ -n "$(git status --porcelain)" ]]; then
  echo "error: working tree is dirty — commit or stash first." >&2
  exit 1
fi

echo "→ Syncing local main to origin/main…"
git checkout main --quiet
git pull --ff-only origin main --quiet

MAIN_TIP="$(git rev-parse HEAD)"
echo "  main is at $MAIN_TIP"
echo "  $(git log --oneline -1)"

# Confirm the version in tauri.conf.json matches the tag (vX.Y.Z → X.Y.Z).
WANT="${TAG#v}"
HAVE="$(grep -m1 '"version"' frontend/tauri.conf.json | sed -E 's/.*"version": *"([^"]+)".*/\1/')"
if [[ "$WANT" != "$HAVE" ]]; then
  echo "error: tag $TAG implies version $WANT but frontend/tauri.conf.json says $HAVE." >&2
  echo "       bump the version (and Cargo.toml) on main first, or use the matching tag." >&2
  exit 1
fi

echo "→ Moving tag $TAG to $MAIN_TIP and re-pushing…"
git tag -d "$TAG" 2>/dev/null || true
git push origin ":refs/tags/$TAG" 2>/dev/null || true
git tag -a "$TAG" -m "$TAG"
git push origin "$TAG"

echo
echo "✓ Pushed $TAG → re-triggered the five Release workflows."
echo "  Watch: https://github.com/JinXSuperSolo/gwenland-ide/actions"
