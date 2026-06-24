#!/usr/bin/env bash
# Local CD: build the GwenLand IDE release installers via `cargo tauri build`
# and, optionally, publish them to a GitHub Release.
#
# The GitHub Actions CD path cannot run the full Tauri bundle for us, so the
# release build is done locally here. Produces the platform's native bundles
# (Windows MSI + NSIS; macOS .dmg/.app; Linux .deb/.AppImage per tauri.conf.json
# targets). Run from anywhere — paths resolve relative to the repo root.
#
# Usage:
#   scripts/release.sh                      # build for the version in tauri.conf.json
#   scripts/release.sh --version 0.1.0      # build a specific version (must match config)
#   scripts/release.sh --publish            # also tag + create a GitHub Release via gh
#   scripts/release.sh --skip-checks        # skip clean-tree / version-match guards
set -euo pipefail

# --- Locate repo root (this script lives in <root>/scripts) ----------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
FRONTEND_DIR="$REPO_ROOT/frontend"
CONF="$FRONTEND_DIR/tauri.conf.json"
BUDGET_BYTES=6815744  # 6.5 MB binary budget

VERSION=""
PUBLISH=0
SKIP_CHECKS=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --version) VERSION="$2"; shift 2 ;;
    --publish) PUBLISH=1; shift ;;
    --skip-checks) SKIP_CHECKS=1; shift ;;
    *) echo "Unknown arg: $1" >&2; exit 1 ;;
  esac
done

info() { printf '\033[36m==> %s\033[0m\n' "$1"; }
ok()   { printf '\033[32mOK  %s\033[0m\n' "$1"; }
fail() { printf '\033[31mERR %s\033[0m\n' "$1" >&2; exit 1; }

# --- 1. Toolchain ----------------------------------------------------------
info 'Checking toolchain'
for tool in cargo pnpm; do
  command -v "$tool" >/dev/null 2>&1 || fail "$tool not found on PATH"
done
cargo tauri --version >/dev/null 2>&1 || \
  fail 'cargo tauri not found. Install with: cargo install tauri-cli --version "^2"'
ok 'cargo, pnpm, cargo tauri present'

# --- 2. Version + tree guards ----------------------------------------------
# Read "version": "x.y.z" from tauri.conf.json without a JSON dependency.
CONF_VERSION="$(grep -m1 '"version"' "$CONF" | sed -E 's/.*"version"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/')"
[[ -n "$VERSION" ]] || VERSION="$CONF_VERSION"
info "Target version: $VERSION (tauri.conf.json: $CONF_VERSION)"

if [[ "$SKIP_CHECKS" -eq 0 ]]; then
  [[ "$VERSION" == "$CONF_VERSION" ]] || \
    fail "Version mismatch: --version $VERSION != tauri.conf.json $CONF_VERSION. Bump the config first."
  if [[ -n "$(git -C "$REPO_ROOT" status --porcelain)" ]]; then
    fail 'Working tree is not clean. Commit or stash before releasing.'
  fi
  ok 'Version matches and tree is clean'
else
  printf '\033[33mWARN skipping clean-tree / version guards (--skip-checks)\033[0m\n'
fi

# --- 3. Build the installers -----------------------------------------------
info 'Building release bundle (cargo tauri build) — this runs pnpm build first'
( cd "$FRONTEND_DIR" && cargo tauri build ) || fail 'cargo tauri build failed'

# --- 4. Binary budget check ------------------------------------------------
EXE="$REPO_ROOT/target/release/GwenLand-IDE"
if [[ -f "$EXE" ]]; then
  SIZE="$(stat -c%s "$EXE" 2>/dev/null || stat -f%z "$EXE")"
  SIZE_MB="$(awk "BEGIN{printf \"%.2f\", $SIZE/1048576}")"
  if [[ "$SIZE" -gt "$BUDGET_BYTES" ]]; then
    printf '\033[33mWARN GwenLand-IDE is %s MB, over the 6.5 MB budget\033[0m\n' "$SIZE_MB"
  else
    ok "GwenLand-IDE is $SIZE_MB MB (under the 6.5 MB budget)"
  fi
fi

# --- Collect produced bundles ----------------------------------------------
BUNDLE_DIR="$REPO_ROOT/target/release/bundle"
mapfile -t ARTIFACTS < <(find "$BUNDLE_DIR" -type f \
  \( -name '*.msi' -o -name '*-setup.exe' -o -name '*.dmg' -o -name '*.deb' -o -name '*.AppImage' -o -name '*.rpm' \) \
  2>/dev/null || true)
[[ "${#ARTIFACTS[@]}" -gt 0 ]] || fail "No installers found under $BUNDLE_DIR"

info 'Built installers:'
printf '    %s\n' "${ARTIFACTS[@]}"

# --- 5. Optional publish ---------------------------------------------------
if [[ "$PUBLISH" -eq 1 ]]; then
  command -v gh >/dev/null 2>&1 || \
    fail 'gh CLI not found — install GitHub CLI and `gh auth login`, or drop --publish and upload manually.'
  TAG="v$VERSION"
  info "Publishing GitHub Release $TAG"
  if [[ -z "$(git -C "$REPO_ROOT" tag --list "$TAG")" ]]; then
    git -C "$REPO_ROOT" tag -a "$TAG" -m "Release $TAG"
    git -C "$REPO_ROOT" push origin "$TAG"
    ok "Created and pushed tag $TAG"
  else
    printf '\033[33mWARN tag %s already exists; reusing it\033[0m\n' "$TAG"
  fi
  gh release create "$TAG" "${ARTIFACTS[@]}" \
    --title "GwenLand IDE $TAG" --generate-notes \
    --repo "$(git -C "$REPO_ROOT" remote get-url origin)" \
    || fail 'gh release create failed'
  ok "Published $TAG with ${#ARTIFACTS[@]} installer(s)"
else
  info 'Local build only. Re-run with --publish to tag + upload via gh, or attach the installers above manually.'
fi

ok 'Done.'
