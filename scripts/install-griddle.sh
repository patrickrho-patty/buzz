#!/usr/bin/env bash
# Reinstall Griddle.app from the local build output over the existing
# /Applications/Griddle.app, in place — no DMG drag-install needed.
#
# Usage: just scripts/install-griddle.sh
#   Optional: scripts/install-griddle.sh /custom/install/path
set -euo pipefail
BUNDLE_DIR="$(cd "$(dirname "$0")/../desktop/src-tauri/target/release/bundle/macos" 2>/dev/null && pwd)"
DEST="${1:-/Applications}"
APP_SRC="$BUNDLE_DIR/Griddle.app"
APP_DST="$DEST/Griddle.app"

[[ -d "$APP_SRC" ]] || { echo "error: $APP_SRC not found — run: just desktop-dev / pnpm tauri build --bundles app" >&2; exit 1; }

if pgrep -f "Griddle.app/Contents/MacOS/buzz-desktop" >/dev/null 2>&1; then
  echo "stopping running Griddle..."
  pkill -x buzz-desktop || true
  sleep 1
fi

if [[ -d "$APP_DST" ]]; then
  rm -rf "$APP_DST"
  echo "removed existing $APP_DST"
fi

ditto "$APP_SRC" "$APP_DST"
echo "installed: $APP_DST"

# Re-sign with the stable Developer ID so macOS Keychain recognizes the SAME
# signing identity across reinstalls — no password prompt on every update.
# (Ad-hoc signing changes the signature each install, which orphans the
# keychain ACL entry and forces the macOS permission prompt.)
IDENT="${CREW_CODESIGN_IDENTITY:-Developer ID Application: Patty Co.,LTD (S37644C7R8)}"
if security find-identity -v -p codesigning 2>/dev/null | grep -q "Developer ID Application"; then
  codesign --force --deep --options runtime --sign "$IDENT" "$APP_DST" 2>/dev/null \
    && echo "Developer ID re-signed (stable keychain identity)" \
    || codesign --force --deep --sign - "$APP_DST" 2>/dev/null && [ $? -ne 0 ] || true
else
  codesign --force --deep --sign - "$APP_DST" 2>/dev/null && echo "ad-hoc re-signed (no Developer ID cert found)"
fi
