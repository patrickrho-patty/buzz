#!/usr/bin/env bash
# Weekly build-cache hygiene for the Crew repo.
#
# Invoked by the sh.crew.tidy launchd agent (see install command in
# CREW_RENAME_PLAN or the plist footer) but safe to run by hand:
#   scripts/crew-tidy.sh /Users/patrickrho/projects/griddle [days] [cap_gb]
#
# Prunes Rust build artifacts older than DAYS from the shared target cache,
# then fails loudly if the cache still exceeds CAP_GB (surfaced via launchd
# exit status + stderr in `launchctl` logs).
set -euo pipefail

REPO="${1:-/Users/patrickrho/projects/griddle}"
DAYS="${2:-21}"
CAP_GB="${3:-20}"
TARGET="$REPO/target"

if [[ ! -d "$TARGET" ]]; then
    echo "crew-tidy: no cache at $TARGET, nothing to do" >&2
    exit 0
fi

before=$(du -sg "$TARGET" 2>/dev/null | awk '{s+=$1} END {print s+0}')
find "$TARGET" -type f \( -atime +"$DAYS" -o -mtime +"$DAYS" \) -print0 2>/dev/null \
    | xargs -0 rm -f 2>/dev/null || true
find "$TARGET" -mindepth 1 -type d -empty -delete 2>/dev/null || true
after=$(du -sg "$TARGET" 2>/dev/null | awk '{s+=$1} END {print s+0}')
echo "crew-tidy: $TARGET ${before}GB -> ${after}GB (> ${DAYS}d old pruned)"

gb=$(du -sg "$TARGET" | cut -f1)
if (( gb > CAP_GB )); then
    echo "crew-tidy: ERROR $TARGET still ${gb}GB > cap ${CAP_GB}GB; run 'cargo clean' there" >&2
    exit 1
fi
