#!/usr/bin/env bash
# Reset Griddle desktop app state for a fresh onboarding experience.
#
# Wipes (in order):
#   1. Running app process (quit)
#   2. OS keychain identity   (griddle-desktop service)
#   3. App data dir           (~/Library/Application Support/xyz.patty.griddle.app)
#                             — localStorage: onboarding transactions, SSO stash,
#                               cached profiles, webview state
#   4. Legacy buzz-identifier data (~/Library/WebKit/xyz.block.buzz.app) from
#      pre-rebrand installs
#
# Keeps:
#   - Relay-side membership (re-admitted on next SSO login)
#   - Keycloak user + nostr attributes (login re-mints or recovers)
#   - Notarized DMG installs (only the /Applications copy is reset)
#
# Optional flags:
#   --launch   relaunch the app after reset
#
# Usage: scripts/reset-griddle-desktop.sh [--launch]

set -euo pipefail

LAUNCH=0
for arg in "$@"; do
  case "$arg" in
    --launch) LAUNCH=1 ;;
    *) echo "unknown flag: $arg" >&2; exit 1 ;;
  esac
done

APP_ID="xyz.patty.griddle.app"
APP_PATH="/Applications/Griddle.app"

echo "==> Quitting Griddle…"
pkill -x buzz-desktop 2>/dev/null || true
sleep 1

echo "==> Deleting keychain identity (griddle-desktop)…"
while security delete-generic-password -s griddle-desktop >/dev/null 2>&1; do :; done

echo "==> Wiping app data (localStorage, onboarding state, caches)…"
rm -rf "$HOME/Library/Application Support/$APP_ID"
rm -rf "$HOME/Library/Caches/$APP_ID" 2>/dev/null || true
rm -rf "$HOME/Library/WebKit/$APP_ID" 2>/dev/null || true

echo "==> Wiping legacy pre-rebrand data (xyz.block.buzz.app)…"
rm -rf "$HOME/Library/WebKit/xyz.block.buzz.app" 2>/dev/null || true
rm -rf "$HOME/Library/Application Support/xyz.block.buzz.app" 2>/dev/null || true

echo "==> Reset complete."
if [[ "$LAUNCH" -eq 1 ]]; then
  if [[ -d "$APP_PATH" ]]; then
    echo "==> Launching $APP_PATH..."
    open "$APP_PATH"
  else
    echo "    (app not found at $APP_PATH — skipping launch)" >&2
  fi
fi
