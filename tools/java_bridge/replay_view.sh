#!/bin/bash
# Watches a ReplayMod recording (.mcpr) with Horizon's replay viewer (`--replay`), headless, in
# the scratch folder S (its own homes: never real data), then runs each argument as a step
# (V: horizon-ctl on the viewer) and writes view-N.png (in S and OUT) after each.
#   S=/tmp/hz-bridge.x tools/java_bridge/replay_view.sh FILE.mcpr 'sleep 5' '$V command "speed 2"'
# Without a FILE: the newest replay_recordings/**/*.mcpr in S.
set -e
R=$(cd "$(dirname "$0")/../.." && pwd)
: "${S:?S: the scratch folder}" "${OUT:=/tmp}"
FILE=${1:-}
if [ -z "$FILE" ] || [ ! -f "$FILE" ]; then
  FILE=$(ls -t "$S"/replay_recordings/*/*.mcpr "$S"/replay_recordings/*.mcpr 2>/dev/null | head -1)
else
  shift
fi
[ -f "$FILE" ] || { echo "no recording in $S/replay_recordings" >&2; exit 1; }
VS=$S/viewer; mkdir -p "$VS/home" "$VS/lh" "$VS/data" "$VS/conf"
"$R/tools/java_bridge/fps_cap.sh" "$VS"
if [ -z "$DISPLAY" ]; then
  pgrep -x Xvfb >/dev/null || (nohup Xvfb :77 -screen 0 1280x1024x24 >/tmp/xvfb.log 2>&1 &)
  export DISPLAY=:77; sleep 1
fi
V="$R/target/fast/horizon-ctl --socket $VS/ctl.sock"
(cd "$VS" && HORIZON_HOME=$VS/home HORIZON_LAUNCHER_HOME=$VS/lh XDG_DATA_HOME=$VS/data XDG_CONFIG_HOME=$VS/conf \
  HORIZON_LAUNCHER_NO_SELF_UPDATE=1 HORIZON_NO_AUDIO=1 \
  $V launch --offscreen --wait --game "$R/target/fast/horizon-client" \
  --resources "$R/.local/horizon-client/26.3" --replay "$FILE" > "$VS/ctl.out" 2>&1)
echo "viewer: $FILE (log $VS/home/logs/latest.log)"
n=0; for step in "$@"; do eval "$step"; sleep 1; n=$((n+1)); $V screenshot "$VS/view-$n.png" >/dev/null; cp "$VS/view-$n.png" "$OUT/view-$n.png"; done
$V quit >/dev/null 2>&1 || true
