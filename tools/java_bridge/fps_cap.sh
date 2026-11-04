#!/bin/bash
# Caps a test game's frame rate: maxFps in DIR/options.txt (the game folder's), MAX_FPS (default
# 30). An offscreen window renders as fast as it can otherwise: 99% of a real GPU (coil whine) for
# screenshots that need a few frames. An options.txt that sets maxFps already (OPTS) keeps it.
# Usage: tools/java_bridge/fps_cap.sh DIR
R=$(cd "$(dirname "$0")/../.." && pwd)
f=$1/options.txt
grep -q '^maxFps:' "$f" 2>/dev/null && exit 0
# A new options.txt names this version's options, so the game updates none of them.
[ -f "$f" ] || sed -n 's/^pub const DATA_VERSION: i32 = \([0-9]*\);/version:\1/p' \
  "$R/crates/horizon-core/src/version.rs" > "$f"
echo "maxFps:${MAX_FPS:-30}" >> "$f"
